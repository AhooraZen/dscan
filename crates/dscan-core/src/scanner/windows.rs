#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::Ordering;

#[cfg(windows)]
use crate::arena::{DirArena, LocalTopFiles};
#[cfg(windows)]
use crate::work_stealing::Worker;

#[cfg(windows)]
use super::buffer::DualBuffer;
#[cfg(windows)]
use super::state::GlobalState;
#[cfg(windows)]
use super::{record_file_ext, record_file_stat};

#[derive(Debug, Clone)]
pub struct WidePathStack {
    wide: Vec<u16>,
}

impl WidePathStack {
    pub fn new() -> Self {
        let mut wide = Vec::with_capacity(1024);
        wide.push(0);
        Self { wide }
    }

    #[cfg(windows)]
    pub fn set_root(&mut self, path: &std::path::Path) {
        use std::os::windows::ffi::OsStrExt;
        self.wide.clear();
        self.wide.extend(path.as_os_str().encode_wide());
        while self.wide.len() > 3
            && (self.wide.last() == Some(&(b'\\' as u16))
                || self.wide.last() == Some(&(b'/' as u16)))
        {
            self.wide.pop();
        }
        self.wide.push(0);
    }

    pub fn set_root_wide(&mut self, wide_root: &[u16]) {
        self.wide.clear();
        self.wide.extend_from_slice(wide_root);
        while self.wide.len() > 3
            && (self.wide.last() == Some(&(b'\\' as u16))
                || self.wide.last() == Some(&(b'/' as u16)))
        {
            self.wide.pop();
        }
        self.wide.push(0);
    }

    #[inline(always)]
    pub fn push_child(&mut self, child_name_wide: &[u16]) -> usize {
        let prev_len = self.wide.len().saturating_sub(1);
        if self.wide.last() == Some(&0) {
            self.wide.pop();
        }
        if !self.wide.ends_with(&[b'\\' as u16]) && !self.wide.ends_with(&[b'/' as u16]) {
            self.wide.push(b'\\' as u16);
        }
        self.wide.extend_from_slice(child_name_wide);
        self.wide.push(0);
        prev_len
    }

    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        self.wide.truncate(len);
        if self.wide.last() != Some(&0) {
            self.wide.push(0);
        }
    }

    #[inline(always)]
    pub fn as_null_terminated(&self) -> *const u16 {
        self.wide.as_ptr()
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u16] {
        if self.wide.len() > 1 {
            &self.wide[..self.wide.len() - 1]
        } else {
            &[]
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.wide.len().saturating_sub(1)
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(windows)]
    pub fn to_path_buf(&self) -> PathBuf {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        PathBuf::from(OsString::from_wide(self.as_slice()))
    }
}

impl Default for WidePathStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
pub fn scan_directory_tree_windows(
    current_node: u32,
    rel_depth: usize,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    dual_buffer: &mut DualBuffer,
    wide_path_stack: &mut WidePathStack,
    path_stack: &mut Vec<u8>,
    utf8_scratch: &mut Vec<u8>,
    local_arena: &mut DirArena,
    local_top_files: &mut LocalTopFiles,
    local_ext_stats: &mut std::collections::HashMap<String, (u64, u64)>,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    if state.cancel_requested.load(Ordering::Relaxed) {
        return;
    }

    use crate::simd::{is_dot_or_dotdot_utf16, transcode_utf16_to_utf8};
    use crate::sys::windows::*;

    let mut local_dir_size: u64 = 0;
    let mut sub_dirs_wide: Vec<Vec<u16>> = Vec::with_capacity(16);

    let root_dev = state.config.root_dev;
    let matcher = &state.config.matcher;

    let wide_ptr = wide_path_stack.as_null_terminated();
    // SAFETY: wide_ptr is a valid null-terminated UTF-16 wide string pointer.
    if let Some(h_dir) = unsafe { open_dir_from_wide_ptr(wide_ptr) } {
        if !state.config.cross_filesystems
            && root_dev != 0
            // SAFETY: h_dir is a valid open directory handle.
            && let Some(vol) = (unsafe { get_volume_serial_number(h_dir) })
            && vol != root_dev
        {
            // SAFETY: h_dir is a valid open handle.
            unsafe { close_handle(h_dir) };
            return;
        }

        let buf_slice = dual_buffer.current_mut().as_mut_slice();
        let buf_ptr = buf_slice.as_mut_ptr() as *mut std::ffi::c_void;
        let buf_len = buf_slice.len() as u32;
        let mut restart_scan = true;

        loop {
            // SAFETY: h_dir is a valid directory handle, buf_ptr is a valid aligned buffer pointer of buf_len bytes.
            let (status, info_bytes) =
                unsafe { sys_nt_query_directory_file_fast(h_dir, buf_ptr, buf_len, restart_scan) };

            if (status != STATUS_SUCCESS && status != STATUS_BUFFER_OVERFLOW) || info_bytes == 0 {
                break;
            }
            restart_scan = false;

            let mut offset = 0usize;
            let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);
            loop {
                if offset + fn_offset > info_bytes {
                    break;
                }

                // SAFETY: offset + fn_offset <= info_bytes; pointer is within buffer bounds.
                let entry_ptr =
                    unsafe { (buf_ptr as *const u8).add(offset) as *const FileIdBothDirInfo };
                // SAFETY: read_unaligned prevents unaligned memory faults from misaligned filesystem records.
                let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };

                let name_len_bytes = entry.file_name_length as usize;
                let name_len_wchars = name_len_bytes / std::mem::size_of::<u16>();

                if offset + fn_offset + name_len_bytes > info_bytes {
                    break;
                }

                // SAFETY: offset + fn_offset + name_len_bytes <= info_bytes.
                let file_name_ptr =
                    unsafe { (entry_ptr as *const u8).add(fn_offset) as *const u16 };
                // SAFETY: file_name_ptr is within buffer bounds verified above.
                let name_slice =
                    unsafe { std::slice::from_raw_parts(file_name_ptr, name_len_wchars) };

                if !is_dot_or_dotdot_utf16(name_slice) {
                    transcode_utf16_to_utf8(name_slice, utf8_scratch);

                    // Fast-path: bypass path_stack operations if relative rule matches (e.g. .git, target, node_modules)
                    if matcher.is_name_excluded(utf8_scratch.as_slice()) {
                        continue;
                    }

                    let orig_path_len = path_stack.len();
                    if !path_stack.ends_with(b"\\") && !path_stack.ends_with(b"/") {
                        path_stack.push(b'\\');
                    }
                    path_stack.extend_from_slice(utf8_scratch);

                    let excluded =
                        matcher.is_excluded(path_stack.as_slice(), utf8_scratch.as_slice());
                    path_stack.truncate(orig_path_len);

                    if !excluded {
                        let attrs = entry.file_attributes;
                        let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                        let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                        if is_dir {
                            if !is_reparse || state.config.follow_symlinks {
                                sub_dirs_wide.push(name_slice.to_vec());
                            }
                        } else {
                            let sz = if entry.allocation_size > 0 {
                                entry.allocation_size as u64
                            } else {
                                entry.end_of_file.max(0) as u64
                            };

                            local_dir_size += sz;
                            record_file_stat(local_files, local_bytes, sz, state);
                            local_top_files.push(
                                sz,
                                current_node,
                                utf8_scratch.as_slice(),
                                local_arena,
                            );
                            record_file_ext(
                                local_ext_stats,
                                utf8_scratch.as_slice(),
                                sz,
                                state.config.collect_ext_stats,
                            );
                        }
                    }
                }

                if entry.next_entry_offset == 0
                    || offset + (entry.next_entry_offset as usize) >= info_bytes
                {
                    break;
                }
                offset += entry.next_entry_offset as usize;
            }
        }

        // SAFETY: h_dir is a valid open handle.
        unsafe { close_handle(h_dir) };
    } else {
        // Fallback to std::fs::read_dir if CreateFileW fails (e.g., special permissions or device paths)
        let dir_path = wide_path_stack.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_bytes = name.to_string_lossy().as_bytes().to_vec();
                let orig_path_len = path_stack.len();
                if !path_stack.ends_with(b"\\") && !path_stack.ends_with(b"/") {
                    path_stack.push(b'\\');
                }
                path_stack.extend_from_slice(&name_bytes);
                let excluded = matcher.is_excluded(path_stack.as_slice(), &name_bytes);
                path_stack.truncate(orig_path_len);
                if excluded {
                    continue;
                }
                let meta_res = if state.config.follow_symlinks {
                    entry.metadata()
                } else {
                    std::fs::symlink_metadata(entry.path())
                };
                if let Ok(meta) = meta_res {
                    if meta.is_dir() {
                        use std::os::windows::ffi::OsStrExt;
                        let wide_name: Vec<u16> = name.encode_wide().collect();
                        sub_dirs_wide.push(wide_name);
                    } else {
                        let sz = meta.len();
                        local_dir_size += sz;
                        record_file_stat(local_files, local_bytes, sz, state);
                        local_top_files.push(sz, current_node, &name_bytes, local_arena);
                        record_file_ext(
                            local_ext_stats,
                            &name_bytes,
                            sz,
                            state.config.collect_ext_stats,
                        );
                    }
                }
            }
        }
    }

    local_arena.add_direct_bytes(current_node, local_dir_size);

    // Work-stealing: construct PathBuf ONLY when delegating to another worker
    if sub_dirs_wide.len() > 1 {
        let half = sub_dirs_wide.split_off(sub_dirs_wide.len() / 2);
        for sub_w in half {
            let prev_len = wide_path_stack.push_child(&sub_w);
            let full_path = wide_path_stack.to_path_buf();
            wide_path_stack.truncate(prev_len);
            worker.push(full_path);
        }
        state.cvar.notify_all();
    }

    // Local recursion: push/pop onto wide_path_stack and path_stack with zero allocations
    for sub_w in sub_dirs_wide {
        transcode_utf16_to_utf8(&sub_w, utf8_scratch);
        let (child_node, next_rel_depth) = if rel_depth < state.config.max_depth {
            let next_d = rel_depth + 1;
            let node = local_arena.add_node(current_node, next_d as u16, utf8_scratch.as_slice());
            (node, next_d)
        } else {
            (current_node, rel_depth + 1)
        };

        let orig_wide_len = wide_path_stack.push_child(&sub_w);
        let orig_path_len = path_stack.len();
        if !path_stack.ends_with(b"\\") && !path_stack.ends_with(b"/") {
            path_stack.push(b'\\');
        }
        path_stack.extend_from_slice(utf8_scratch);

        dual_buffer.swap();
        scan_directory_tree_windows(
            child_node,
            next_rel_depth,
            state,
            worker,
            dual_buffer,
            wide_path_stack,
            path_stack,
            utf8_scratch,
            local_arena,
            local_top_files,
            local_ext_stats,
            local_files,
            local_bytes,
        );
        dual_buffer.swap();

        path_stack.truncate(orig_path_len);
        wide_path_stack.truncate(orig_wide_len);
    }
}
