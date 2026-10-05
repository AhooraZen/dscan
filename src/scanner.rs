use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
#[cfg(unix)]
use std::ffi::CStr;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::CliOptions;
#[cfg(unix)]
use crate::sys::{
    AT_STATX_DONT_SYNC, AT_SYMLINK_NOFOLLOW, DT_DIR, DT_LNK, DT_REG, DT_UNKNOWN, LinuxDirent64,
    S_IFDIR, S_IFMT, S_IFREG, STATX_BLOCKS, STATX_TYPE, SYS_GETDENTS64, open_dir,
};
use crate::ui::{clear_spinner_line, render_spinner_line};
use crate::work_stealing::{Steal, Stealer, Worker, deque};

pub struct AlignedBuffer {
    ptr: *mut u8,
    layout: std::alloc::Layout,
    len: usize,
}

impl AlignedBuffer {
    pub fn new(size: usize, align: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
        // SAFETY: layout has non-zero size and valid power-of-two alignment.
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        AlignedBuffer {
            ptr,
            layout,
            len: size,
        }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: self.ptr points to an allocated buffer of self.len bytes.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        // SAFETY: self.ptr was allocated with self.layout by std::alloc::alloc.
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}

// SAFETY: AlignedBuffer owns its heap allocation and can be transferred across threads.
unsafe impl Send for AlignedBuffer {}

#[repr(align(64))]
pub struct CachePadded<T>(pub T);

impl<T: Default> Default for CachePadded<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct ScanConfig {
    pub excludes: Vec<Vec<u8>>,
    pub root_dev: u64,
    pub base_depth: usize,
    pub max_depth: usize,
    pub top_limit: usize,
    pub threads: usize,
    pub follow_symlinks: bool,
    pub cross_filesystems: bool,
}

pub struct GlobalState {
    pub config: ScanConfig,
    pub total_bytes: CachePadded<AtomicU64>,
    pub total_files: CachePadded<AtomicU64>,
    pub active_workers: CachePadded<AtomicUsize>,
    pub done: CachePadded<AtomicBool>,
    pub current_active: Mutex<String>,
    pub stealers: Vec<Stealer<PathBuf>>,
    pub cvar: Condvar,
    pub cvar_mutex: Mutex<()>,
}

impl GlobalState {
    pub fn all_stealers_empty(&self) -> bool {
        self.stealers.iter().all(|s| s.is_empty())
    }
}

pub struct ThreadLocalResult {
    pub dir_sizes: HashMap<PathBuf, u64>,
    pub top_files: BinaryHeap<Reverse<(u64, PathBuf)>>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub elapsed: Duration,
    pub total_bytes: u64,
    pub total_files: u64,
    pub top_dirs: Vec<(PathBuf, u64)>,
    pub top_files: Vec<(u64, PathBuf)>,
    pub max_dir_size: u64,
    pub max_file_size: u64,
}

#[inline(always)]
pub fn is_excluded_dir(path_bytes: &[u8], name_bytes: &[u8], excludes: &[Vec<u8>]) -> bool {
    for ex in excludes {
        if ex.is_empty() {
            continue;
        }

        let ex_slice = ex.as_slice();

        // Exact name match (e.g. ".git", "target")
        if name_bytes == ex_slice {
            return true;
        }

        // Absolute root prefix match (e.g. "/proc", "/sys", "/dev")
        if ex_slice.starts_with(b"/") {
            if path_bytes == ex_slice {
                return true;
            }
            if path_bytes.starts_with(ex_slice) && path_bytes.get(ex_slice.len()) == Some(&b'/') {
                return true;
            }
        } else {
            // Relative folder match across components
            if path_bytes
                .split(|&b| b == b'/')
                .any(|segment| segment == ex_slice)
            {
                return true;
            }
        }
    }
    false
}

#[inline(always)]
fn push_top_file(
    top_files: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
    limit: usize,
    sz: u64,
    build_path: impl FnOnce() -> PathBuf,
) {
    if top_files.len() < limit {
        top_files.push(Reverse((sz, build_path())));
    } else if let Some(Reverse((min_sz, _))) = top_files.peek()
        && sz > *min_sz
    {
        top_files.pop();
        top_files.push(Reverse((sz, build_path())));
    }
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn scan_directory_tree(
    dir_path: &Path,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    buffer: &mut AlignedBuffer,
    path_stack: &mut Vec<u8>,
    local_dirs: &mut HashMap<PathBuf, u64>,
    local_top_files: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    let mut local_dir_size: u64 = 0;
    let mut sub_dirs = Vec::new();

    let root_dev = state.config.root_dev;
    let top_limit = state.config.top_limit;
    let excludes = &state.config.excludes;

    let dir_bytes = dir_path.as_os_str().as_bytes();
    path_stack.clear();
    path_stack.extend_from_slice(dir_bytes);

    if let Some(fd) = open_dir(dir_path) {
        if !state.config.cross_filesystems {
            let mut stx = crate::sys::Statx::default();
            let empty_path = b"\0";
            let ret = crate::sys::sys_statx(
                fd,
                empty_path.as_ptr() as *const std::ffi::c_char,
                crate::sys::AT_EMPTY_PATH | crate::sys::AT_STATX_DONT_SYNC,
                crate::sys::STATX_TYPE,
                &mut stx,
            );
            if ret == 0 {
                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                if dev != root_dev {
                    unsafe { crate::sys::close(fd) };
                    return;
                }
            }
        }

        let buf_slice = buffer.as_mut_slice();
        loop {
            // SAFETY: fd is valid open directory descriptor, buffer is a valid aligned slice.
            let nread = unsafe {
                crate::sys::syscall(
                    SYS_GETDENTS64,
                    fd as i64,
                    buf_slice.as_mut_ptr() as i64,
                    buf_slice.len() as i64,
                )
            };

            if nread <= 0 {
                break;
            }

            let mut pos = 0usize;
            let nread = nread as usize;

            while pos < nread {
                if pos + std::mem::size_of::<LinuxDirent64>() > nread {
                    break;
                }

                let dirent_ptr = unsafe { buf_slice.as_ptr().add(pos) as *const LinuxDirent64 };
                // SAFETY: dirent_ptr is within buffer bounds; read_unaligned handles unaligned records safely.
                let dirent = unsafe { std::ptr::read_unaligned(dirent_ptr) };
                let reclen = dirent.d_reclen as usize;

                if reclen < 19 || pos + reclen > nread {
                    break;
                }

                let name_start = pos + 19;
                let name_record_slice = &buf_slice[name_start..pos + reclen];
                let name_cstr = match CStr::from_bytes_until_nul(name_record_slice) {
                    Ok(c) => c,
                    Err(_) => {
                        pos += reclen;
                        continue;
                    }
                };
                let name_bytes = name_cstr.to_bytes();

                pos += reclen;

                if name_bytes == b"." || name_bytes == b".." {
                    continue;
                }

                if dir_bytes == b"." {
                    path_stack.clear();
                } else if !path_stack.ends_with(b"/") {
                    path_stack.push(b'/');
                }
                path_stack.extend_from_slice(name_bytes);

                if is_excluded_dir(path_stack, name_bytes, excludes) {
                    path_stack.clear();
                    path_stack.extend_from_slice(dir_bytes);
                    continue;
                }

                let d_type = dirent.d_type;
                match d_type {
                    DT_DIR => {
                        // FAST PATH: Guaranteed directory. No stat syscall required.
                        let child_path = if path_stack.is_empty() {
                            PathBuf::from(".")
                        } else {
                            PathBuf::from(std::ffi::OsStr::from_bytes(path_stack))
                        };
                        sub_dirs.push(child_path);
                    }
                    DT_REG => {
                        // FAST PATH: Guaranteed regular file. Query only STATX_BLOCKS relative to dirfd.
                        let mut stx = crate::sys::Statx::default();
                        let res = crate::sys::sys_statx(
                            fd,
                            name_cstr.as_ptr(),
                            AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
                            STATX_BLOCKS,
                            &mut stx,
                        );
                        if res == 0 {
                            let sz = stx.stx_blocks * 512;
                            local_dir_size += sz;
                            *local_files += 1;
                            *local_bytes += sz;

                            if *local_files >= 1024 {
                                state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                                state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                                *local_bytes = 0;
                                *local_files = 0;
                            }

                            push_top_file(local_top_files, top_limit, sz, || {
                                if path_stack.is_empty() {
                                    PathBuf::from(".")
                                } else {
                                    PathBuf::from(std::ffi::OsStr::from_bytes(path_stack))
                                }
                            });
                        }
                    }
                    DT_UNKNOWN | DT_LNK => {
                        let flags = if state.config.follow_symlinks {
                            AT_STATX_DONT_SYNC
                        } else {
                            AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC
                        };
                        let mut stx = crate::sys::Statx::default();
                        let res = crate::sys::sys_statx(
                            fd,
                            name_cstr.as_ptr(),
                            flags,
                            STATX_TYPE | STATX_BLOCKS,
                            &mut stx,
                        );
                        if res == 0 {
                            let mode = stx.stx_mode;
                            let file_type = mode & S_IFMT;
                            if file_type == S_IFDIR {
                                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                                if state.config.cross_filesystems || dev == root_dev {
                                    let child_path = if path_stack.is_empty() {
                                        PathBuf::from(".")
                                    } else {
                                        PathBuf::from(std::ffi::OsStr::from_bytes(path_stack))
                                    };
                                    sub_dirs.push(child_path);
                                }
                            } else if file_type == S_IFREG {
                                let sz = stx.stx_blocks * 512;
                                local_dir_size += sz;
                                *local_files += 1;
                                *local_bytes += sz;

                                if *local_files >= 1024 {
                                    state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                                    state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                                    *local_bytes = 0;
                                    *local_files = 0;
                                }

                                push_top_file(local_top_files, top_limit, sz, || {
                                    if path_stack.is_empty() {
                                        PathBuf::from(".")
                                    } else {
                                        PathBuf::from(std::ffi::OsStr::from_bytes(path_stack))
                                    }
                                });
                            }
                        }
                    }
                    _ => {
                        // Sockets, FIFOs, character/block devices: skip
                    }
                }

                path_stack.clear();
                path_stack.extend_from_slice(dir_bytes);
            }
        }
        // SAFETY: fd was opened by open_dir.
        unsafe { crate::sys::close(fd) };
    } else if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let p_bytes = path.as_os_str().as_bytes();
            let name_bytes = entry.file_name().as_os_str().as_bytes().to_vec();

            if is_excluded_dir(p_bytes, &name_bytes, excludes) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    sub_dirs.push(path);
                } else if meta.dev() == root_dev {
                    let sz = meta.blocks() * 512;
                    local_dir_size += sz;
                    *local_files += 1;
                    *local_bytes += sz;

                    if *local_files >= 1024 {
                        state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                        state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                        *local_bytes = 0;
                        *local_files = 0;
                    }

                    push_top_file(local_top_files, top_limit, sz, || path);
                }
            }
        }
    }

    *local_dirs.entry(dir_path.to_path_buf()).or_insert(0) += local_dir_size;

    // Dynamic work-stealing offload to idle peers
    if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads {
        let half = sub_dirs.split_off(sub_dirs.len() / 2);
        for d in half {
            worker.push(d);
        }
        state.cvar.notify_all();
    }

    for sub in sub_dirs {
        scan_directory_tree(
            &sub,
            state,
            worker,
            buffer,
            path_stack,
            local_dirs,
            local_top_files,
            local_files,
            local_bytes,
        );
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn scan_directory_tree_windows(
    dir_path: &Path,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    buffer: &mut AlignedBuffer,
    local_dirs: &mut HashMap<PathBuf, u64>,
    local_top_files: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    use crate::sys::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut local_dir_size: u64 = 0;
    let mut sub_dirs = Vec::new();

    let root_dev = state.config.root_dev;
    let top_limit = state.config.top_limit;
    let excludes = &state.config.excludes;

    if let Some(h_dir) = open_dir(dir_path) {
        if !state.config.cross_filesystems
            // SAFETY: h_dir is a valid open directory handle.
            && let Some(vol) = (unsafe { get_volume_serial_number(h_dir) })
            && vol != root_dev
        {
            // SAFETY: h_dir is a valid open handle.
            unsafe { close_handle(h_dir) };
            return;
        }

        let buf_slice = buffer.as_mut_slice();
        let buf_ptr = buf_slice.as_mut_ptr() as *mut std::ffi::c_void;
        let buf_len = buf_slice.len() as u32;

        loop {
            // SAFETY: h_dir is a valid directory handle opened with FILE_FLAG_BACKUP_SEMANTICS,
            // buf_ptr is a valid aligned buffer pointer.
            let ret = unsafe {
                GetFileInformationByHandleEx(h_dir, FILE_ID_BOTH_DIRECTORY_INFO, buf_ptr, buf_len)
            };

            if ret == 0 {
                break;
            }

            let mut offset = 0usize;
            loop {
                if offset + std::mem::size_of::<FileIdBothDirInfo>() > buf_slice.len() {
                    break;
                }

                let entry_ptr =
                    unsafe { (buf_ptr as *const u8).add(offset) as *const FileIdBothDirInfo };
                // SAFETY: read_unaligned prevents unaligned memory faults from misaligned filesystem records.
                let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };

                let name_len_bytes = entry.file_name_length as usize;
                let name_len_wchars = name_len_bytes / std::mem::size_of::<u16>();

                let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);
                if offset + fn_offset + name_len_bytes > buf_slice.len() {
                    break;
                }

                let file_name_ptr =
                    unsafe { (entry_ptr as *const u8).add(fn_offset) as *const u16 };

                let name_slice =
                    unsafe { std::slice::from_raw_parts(file_name_ptr, name_len_wchars) };

                let is_dot = name_slice == [b'.' as u16];
                let is_dotdot = name_slice == [b'.' as u16, b'.' as u16];

                if !is_dot && !is_dotdot {
                    let os_name = OsString::from_wide(name_slice);
                    let child_path = dir_path.join(&os_name);
                    let child_path_str = child_path.to_string_lossy();

                    let mut excluded = false;
                    for exc in excludes {
                        let exc_str = String::from_utf8_lossy(exc);
                        if child_path_str.ends_with(exc_str.as_ref()) || os_name == exc_str.as_ref()
                        {
                            excluded = true;
                            break;
                        }
                    }

                    if !excluded {
                        let attrs = entry.file_attributes;
                        let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                        let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                        if is_dir {
                            if !is_reparse || state.config.follow_symlinks {
                                sub_dirs.push(child_path);
                            }
                        } else {
                            // Regular file: AllocationSize gives actual disk cluster allocation!
                            let sz = if entry.allocation_size > 0 {
                                entry.allocation_size as u64
                            } else {
                                entry.end_of_file.max(0) as u64
                            };

                            local_dir_size += sz;
                            *local_files += 1;
                            *local_bytes += sz;

                            if *local_files >= 1024 {
                                state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                                state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                                *local_bytes = 0;
                                *local_files = 0;
                            }

                            push_top_file(local_top_files, top_limit, sz, || child_path);
                        }
                    }
                }

                if entry.next_entry_offset == 0
                    || offset + (entry.next_entry_offset as usize) >= buf_slice.len()
                {
                    break;
                }
                offset += entry.next_entry_offset as usize;
            }
        }

        // SAFETY: h_dir is a valid open handle.
        unsafe { close_handle(h_dir) };
    } else if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let p_bytes = path.to_string_lossy().as_bytes().to_vec();
            let name_bytes = entry.file_name().to_string_lossy().as_bytes().to_vec();

            if is_excluded_dir(&p_bytes, &name_bytes, excludes) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    sub_dirs.push(path);
                } else {
                    let sz = meta.len();
                    local_dir_size += sz;
                    *local_files += 1;
                    *local_bytes += sz;

                    if *local_files >= 1024 {
                        state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                        state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                        *local_bytes = 0;
                        *local_files = 0;
                    }

                    push_top_file(local_top_files, top_limit, sz, || path);
                }
            }
        }
    }

    *local_dirs.entry(dir_path.to_path_buf()).or_insert(0) += local_dir_size;

    if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads {
        let half = sub_dirs.split_off(sub_dirs.len() / 2);
        for d in half {
            worker.push(d);
        }
        state.cvar.notify_all();
    }

    for sub in sub_dirs {
        scan_directory_tree_windows(
            &sub,
            state,
            worker,
            buffer,
            local_dirs,
            local_top_files,
            local_files,
            local_bytes,
        );
    }
}

fn worker_loop(
    thread_id: usize,
    worker: Worker<PathBuf>,
    state: Arc<GlobalState>,
) -> ThreadLocalResult {
    let mut local_dirs: HashMap<PathBuf, u64> = HashMap::with_capacity(4096);
    let mut local_top_files: BinaryHeap<Reverse<(u64, PathBuf)>> =
        BinaryHeap::with_capacity(state.config.top_limit + 10);
    let mut aligned_buffer = AlignedBuffer::new(512 * 1024, 4096);
    #[cfg(unix)]
    let mut path_stack = Vec::with_capacity(4096);
    let mut local_files: u64 = 0;
    let mut local_bytes: u64 = 0;

    let mut is_active = thread_id == 0;
    let num_stealers = state.stealers.len();

    loop {
        // 1. Try local worker deque (LIFO)
        let task = if let Some(t) = worker.pop() {
            Some(t)
        } else {
            // 2. Try stealing from peers (FIFO)
            let mut stolen = None;
            for offset in 1..num_stealers {
                let target_idx = (thread_id + offset) % num_stealers;
                match state.stealers[target_idx].steal() {
                    Steal::Success(t) => {
                        stolen = Some(t);
                        break;
                    }
                    Steal::Retry | Steal::Empty => continue,
                }
            }
            stolen
        };

        if let Some(current_dir) = task {
            if !is_active {
                state.active_workers.fetch_add(1, Ordering::SeqCst);
                is_active = true;
            }

            if let Ok(mut act) = state.current_active.try_lock() {
                *act = current_dir.to_string_lossy().to_string();
            }

            #[cfg(unix)]
            scan_directory_tree(
                &current_dir,
                &state,
                &worker,
                &mut aligned_buffer,
                &mut path_stack,
                &mut local_dirs,
                &mut local_top_files,
                &mut local_files,
                &mut local_bytes,
            );

            #[cfg(windows)]
            scan_directory_tree_windows(
                &current_dir,
                &state,
                &worker,
                &mut aligned_buffer,
                &mut local_dirs,
                &mut local_top_files,
                &mut local_files,
                &mut local_bytes,
            );

            // Flush remaining batched counts on directory completion
            if local_files > 0 {
                state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
                state.total_files.fetch_add(local_files, Ordering::Relaxed);
                local_bytes = 0;
                local_files = 0;
            }
        } else {
            // No task available
            if is_active {
                state.active_workers.fetch_sub(1, Ordering::SeqCst);
                is_active = false;
            }

            if state.done.load(Ordering::SeqCst) {
                break;
            }

            if state.active_workers.load(Ordering::SeqCst) == 0 && state.all_stealers_empty() {
                state.done.store(true, Ordering::SeqCst);
                state.cvar.notify_all();
                break;
            }

            // Wait for work notification or timeout
            let guard = state.cvar_mutex.lock().unwrap();
            if state.done.load(Ordering::SeqCst) {
                break;
            }
            if state.active_workers.load(Ordering::SeqCst) == 0 && state.all_stealers_empty() {
                state.done.store(true, Ordering::SeqCst);
                state.cvar.notify_all();
                break;
            }
            let _ = state.cvar.wait_timeout(guard, Duration::from_millis(1));
        }
    }

    ThreadLocalResult {
        dir_sizes: local_dirs,
        top_files: local_top_files,
    }
}

pub fn run_scan(options: &CliOptions) -> Result<ScanResult, std::io::Error> {
    let root = Path::new(&options.target_path);
    let root_meta = root.metadata()?;
    #[cfg(unix)]
    let root_dev = {
        use std::os::unix::fs::MetadataExt;
        root_meta.dev()
    };
    #[cfg(windows)]
    let root_dev = {
        let _ = &root_meta;
        if let Some(h) = crate::sys::open_dir(root) {
            // SAFETY: h is a valid open directory handle returned by open_dir.
            let dev = (unsafe { crate::sys::get_volume_serial_number(h) }).unwrap_or(0);
            // SAFETY: h is a valid open handle returned by open_dir.
            unsafe { crate::sys::close_handle(h) };
            dev
        } else {
            0
        }
    };
    let base_depth = root.components().count();

    let excludes_bytes: Vec<Vec<u8>> = options
        .excludes
        .iter()
        .map(|s| s.as_bytes().to_vec())
        .collect();

    let num_threads = options.threads.max(1);

    let mut workers = Vec::with_capacity(num_threads);
    let mut stealers = Vec::with_capacity(num_threads);

    for _ in 0..num_threads {
        let (w, s) = deque::<PathBuf>();
        workers.push(w);
        stealers.push(s);
    }

    // Push initial root directory to worker 0
    workers[0].push(root.to_path_buf());

    let state = Arc::new(GlobalState {
        config: ScanConfig {
            excludes: excludes_bytes,
            root_dev,
            base_depth,
            max_depth: options.max_depth,
            top_limit: options.top_limit,
            threads: num_threads,
            follow_symlinks: options.follow_symlinks,
            cross_filesystems: options.cross_filesystems,
        },
        stealers,
        active_workers: CachePadded(AtomicUsize::new(1)),
        total_bytes: CachePadded(AtomicU64::new(0)),
        total_files: CachePadded(AtomicU64::new(0)),
        done: CachePadded(AtomicBool::new(false)),
        current_active: Mutex::new(String::new()),
        cvar: Condvar::new(),
        cvar_mutex: Mutex::new(()),
    });

    let start_time = Instant::now();
    let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Live TUI progress thread
    let rep_state = Arc::clone(&state);
    let rep_handle = thread::spawn(move || {
        let mut idx = 0;
        while !rep_state.done.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(60));
            idx = (idx + 1) % spinners.len();

            let bytes = rep_state.total_bytes.load(Ordering::Relaxed);
            let files = rep_state.total_files.load(Ordering::Relaxed);
            let active = rep_state.active_workers.load(Ordering::Relaxed);
            let cur = rep_state.current_active.lock().unwrap().clone();

            render_spinner_line(spinners[idx], bytes, files, active, &cur);
        }
    });

    let mut handles = Vec::with_capacity(num_threads);
    for (i, w) in workers.into_iter().enumerate() {
        let state_clone = Arc::clone(&state);
        handles.push(thread::spawn(move || worker_loop(i, w, state_clone)));
    }

    let mut all_results = Vec::with_capacity(num_threads);
    for h in handles {
        if let Ok(res) = h.join() {
            all_results.push(res);
        }
    }

    state.done.store(true, Ordering::SeqCst);
    state.cvar.notify_all();
    let _ = rep_handle.join();

    let elapsed = start_time.elapsed();
    let total_bytes = state.total_bytes.load(Ordering::SeqCst);
    let total_files = state.total_files.load(Ordering::SeqCst);

    clear_spinner_line();

    // Merge Thread-Local Results and hierarchical bottom-up rollup
    let mut raw_dirs: HashMap<PathBuf, u64> = HashMap::new();
    let mut final_top_files: BinaryHeap<Reverse<(u64, PathBuf)>> = BinaryHeap::new();

    for res in all_results {
        for (k, v) in res.dir_sizes {
            *raw_dirs.entry(k).or_insert(0) += v;
        }
        for item in res.top_files {
            final_top_files.push(item);
        }
    }

    let max_depth = options.max_depth;
    let mut final_dirs: HashMap<PathBuf, u64> = HashMap::new();

    // Ensure all scanned directories with rel_depth <= max_depth exist in final_dirs
    for dir in raw_dirs.keys() {
        let cur_depth = dir.components().count();
        let rel_depth = cur_depth.saturating_sub(base_depth);
        if rel_depth <= max_depth {
            final_dirs.entry(dir.clone()).or_insert(0);
        }
    }

    // Hierarchical bottom-up rollup: add each directory's shallow file size to all its ancestors
    for (dir, direct_sz) in &raw_dirs {
        if *direct_sz == 0 {
            continue;
        }
        let mut curr: &Path = dir;
        let mut hit_root = false;
        loop {
            if curr == root {
                hit_root = true;
            }
            let cur_depth = curr.components().count();
            if cur_depth < base_depth {
                break;
            }
            let rel_depth = cur_depth.saturating_sub(base_depth);
            if rel_depth <= max_depth {
                *final_dirs.entry(curr.to_path_buf()).or_insert(0) += *direct_sz;
            }
            match curr.parent() {
                Some(p) if !p.as_os_str().is_empty() => curr = p,
                _ => break,
            }
        }
        if !hit_root {
            *final_dirs.entry(root.to_path_buf()).or_insert(0) += *direct_sz;
        }
    }

    let mut sorted_dirs: Vec<(PathBuf, u64)> = final_dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let max_dir_size = sorted_dirs.first().map(|(_, s)| *s).unwrap_or(1).max(1);

    let mut files_vec: Vec<(u64, PathBuf)> =
        final_top_files.into_iter().map(|Reverse(x)| x).collect();
    files_vec.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    files_vec.dedup_by(|a, b| a.1 == b.1);

    let max_file_size = files_vec.first().map(|(s, _)| *s).unwrap_or(1).max(1);

    Ok(ScanResult {
        elapsed,
        total_bytes,
        total_files,
        top_dirs: sorted_dirs.into_iter().take(options.top_limit).collect(),
        top_files: files_vec.into_iter().take(options.top_limit).collect(),
        max_dir_size,
        max_file_size,
    })
}

pub fn print_report(result: &ScanResult) {
    crate::ui::render_report(result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_excluded_dir() {
        let excludes = vec![
            b".git".to_vec(),
            b"/proc".to_vec(),
            b"/sys".to_vec(),
            b"/dev".to_vec(),
            b"/run".to_vec(),
        ];

        // 1. Exact name match
        assert!(is_excluded_dir(b"/home/user/.git", b".git", &excludes));

        // 2. Relative component match
        assert!(is_excluded_dir(
            b"/home/user/project/.git/objects",
            b"objects",
            &excludes
        ));

        // 3. Root prefix match
        assert!(is_excluded_dir(b"/dev/shm", b"shm", &excludes));

        // 4. Root prefix exact
        assert!(is_excluded_dir(b"/dev", b"dev", &excludes));

        // 5. Substring non-match: /home/user/development is NOT excluded by /dev
        assert!(!is_excluded_dir(
            b"/home/user/development/code",
            b"code",
            &excludes
        ));

        // 6. Substring non-match: /home/user/runner/job is NOT excluded by /run
        assert!(!is_excluded_dir(
            b"/home/user/runner/job",
            b"job",
            &excludes
        ));

        // 7. Substring non-match: /home/user/system_monitor is NOT excluded by /sys
        assert!(!is_excluded_dir(
            b"/home/user/system_monitor",
            b"system_monitor",
            &excludes
        ));

        // 8. Empty pattern
        assert!(!is_excluded_dir(b"/home/user/safe", b"safe", &[]));
    }

    #[test]
    fn test_aligned_buffer() {
        let mut buf = AlignedBuffer::new(512 * 1024, 4096);
        assert_eq!(buf.len(), 512 * 1024);
        assert!(!buf.is_empty());
        assert_eq!((buf.as_ptr() as usize) % 4096, 0);

        let slice = buf.as_mut_slice();
        slice[0] = 0xAA;
        slice[512 * 1024 - 1] = 0xBB;
        assert_eq!(slice[0], 0xAA);
        assert_eq!(slice[512 * 1024 - 1], 0xBB);
    }

    #[test]
    fn test_cache_padded_align() {
        assert_eq!(std::mem::align_of::<CachePadded<AtomicU64>>(), 64);
        let padded = CachePadded(AtomicU64::new(42));
        assert_eq!(padded.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn test_scan_tree_and_rollup() {
        let temp_dir = std::env::temp_dir().join(format!("dscan_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("parent/child1")).unwrap();
        fs::create_dir_all(temp_dir.join("parent/child2")).unwrap();

        fs::write(temp_dir.join("parent/child1/file1.bin"), vec![1u8; 8192]).unwrap();
        fs::write(temp_dir.join("parent/child2/file2.bin"), vec![2u8; 16384]).unwrap();

        let options = CliOptions {
            target_path: temp_dir.to_str().unwrap().to_string(),
            top_limit: 10,
            max_depth: 3,
            threads: 4,
            excludes: vec![],
            follow_symlinks: false,
            cross_filesystems: false,
        };

        let result = run_scan(&options).expect("run_scan failed");
        assert!(result.total_files >= 2);
        assert!(result.total_bytes >= 24576);

        // Verify parent directory size is >= child1 + child2
        let parent_path = temp_dir.join("parent");
        let parent_entry = result.top_dirs.iter().find(|(p, _)| p == &parent_path);
        assert!(
            parent_entry.is_some(),
            "parent directory missing from top_dirs"
        );
        let parent_sz = parent_entry.unwrap().1;

        let child1_path = temp_dir.join("parent/child1");
        let child1_sz = result
            .top_dirs
            .iter()
            .find(|(p, _)| p == &child1_path)
            .map(|(_, s)| *s)
            .unwrap_or(0);

        let child2_path = temp_dir.join("parent/child2");
        let child2_sz = result
            .top_dirs
            .iter()
            .find(|(p, _)| p == &child2_path)
            .map(|(_, s)| *s)
            .unwrap_or(0);

        assert!(
            parent_sz >= child1_sz + child2_sz,
            "parent_sz ({parent_sz}) must be >= child1 ({child1_sz}) + child2 ({child2_sz})"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
