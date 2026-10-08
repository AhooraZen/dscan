use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::arena::{DirArena, LocalTopFiles};
use crate::sys::{
    AT_STATX_DONT_SYNC, AT_SYMLINK_NOFOLLOW, DT_DIR, DT_LNK, DT_REG, DT_UNKNOWN, LinuxDirent64,
    S_IFDIR, S_IFMT, S_IFREG, STATX_BLOCKS, STATX_INO, STATX_TYPE, SYS_GETDENTS64, open_dir,
};
use crate::work_stealing::Worker;

use super::buffer::DualBuffer;
use super::state::GlobalState;
use super::{record_file_ext, record_file_stat};

#[allow(clippy::too_many_arguments)]
pub fn scan_directory_tree(
    dir_path: &Path,
    dir_fd: Option<i32>,
    current_node: u32,
    rel_depth: usize,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    dual_buffer: &mut DualBuffer,
    path_stack: &mut Vec<u8>,
    local_arena: &mut DirArena,
    local_top_files: &mut LocalTopFiles,
    local_ext_stats: &mut std::collections::HashMap<String, (u64, u64)>,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    if state.cancel_requested.load(Ordering::Relaxed) {
        return;
    }

    let mut local_dir_size: u64 = 0;
    let mut sub_dirs: Vec<Vec<u8>> = Vec::with_capacity(16);

    let root_dev = state.config.root_dev;
    let matcher = &state.config.matcher;

    let dir_bytes = dir_path.as_os_str().as_bytes();
    if path_stack.as_slice() != dir_bytes {
        path_stack.clear();
        path_stack.extend_from_slice(dir_bytes);
    }

    let (fd_opt, need_dev_check) = if let Some(fd) = dir_fd {
        (Some(fd), false)
    } else {
        (open_dir(dir_path), true)
    };

    if let Some(fd) = fd_opt {
        if need_dev_check && !state.config.cross_filesystems {
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
                    // SAFETY: fd was opened by open_dir or open_dir_at2.
                    unsafe { crate::sys::close(fd) };
                    return;
                }
            }
        }

        let buf_slice = dual_buffer.current_mut().as_mut_slice();
        loop {
            // SAFETY: fd is a valid open directory descriptor; buf_slice is a valid aligned slice.
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

                // SAFETY: pos + size_of::<LinuxDirent64>() <= nread; pointer arithmetic stays within buffer bounds.
                let dirent_ptr = unsafe { buf_slice.as_ptr().add(pos) as *const LinuxDirent64 };
                // SAFETY: dirent_ptr is within buffer bounds; read_unaligned handles unaligned records safely.
                let dirent = unsafe { std::ptr::read_unaligned(dirent_ptr) };
                let reclen = dirent.d_reclen as usize;

                if reclen < 19 || pos + reclen > nread {
                    break;
                }

                let name_start = pos + 19;
                let name_record_slice = &buf_slice[name_start..pos + reclen];
                let nul_pos = crate::simd::find_nul(name_record_slice);
                if nul_pos >= name_record_slice.len() {
                    pos += reclen;
                    continue;
                }
                let name_bytes = &name_record_slice[..nul_pos];

                pos += reclen;

                if name_bytes == b"." || name_bytes == b".." {
                    continue;
                }

                // Fast-path: bypass path_stack operations if relative rule matches (e.g. .git, target, node_modules)
                if matcher.is_name_excluded(name_bytes) {
                    continue;
                }

                let orig_len = path_stack.len();
                if dir_bytes == b"." {
                    path_stack.clear();
                    path_stack.extend_from_slice(name_bytes);
                } else {
                    if !path_stack.ends_with(b"/") && !path_stack.is_empty() {
                        path_stack.push(b'/');
                    }
                    path_stack.extend_from_slice(name_bytes);
                }

                if matcher.is_excluded(path_stack, name_bytes) {
                    path_stack.truncate(orig_len);
                    continue;
                }

                let d_type = dirent.d_type;
                match d_type {
                    DT_DIR => {
                        sub_dirs.push(name_bytes.to_vec());
                    }
                    DT_REG => {
                        #[cfg(any(target_os = "linux", target_os = "android"))]
                        {
                            let mut kstat = crate::sys::KernelStat::default();
                            let path_c =
                                buf_slice[name_start..].as_ptr() as *const std::ffi::c_char;
                            let res = crate::sys::sys_newfstatat(
                                fd,
                                path_c,
                                AT_SYMLINK_NOFOLLOW,
                                &mut kstat,
                            );
                            if res == 0 {
                                let sz = (kstat.st_blocks as u64) * 512;
                                local_dir_size += sz;
                                record_file_stat(local_files, local_bytes, sz, state);
                                local_top_files.push(sz, current_node, name_bytes, local_arena);
                                record_file_ext(
                                    local_ext_stats,
                                    name_bytes,
                                    sz,
                                    state.config.collect_ext_stats,
                                );
                            }
                        }

                        #[cfg(not(any(target_os = "linux", target_os = "android")))]
                        {
                            let mut stx = crate::sys::Statx::default();
                            let path_c =
                                buf_slice[name_start..].as_ptr() as *const std::ffi::c_char;
                            let res = crate::sys::sys_statx(
                                fd,
                                path_c,
                                AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
                                STATX_BLOCKS,
                                &mut stx,
                            );
                            if res == 0 {
                                let sz = stx.stx_blocks * 512;
                                local_dir_size += sz;
                                record_file_stat(local_files, local_bytes, sz, state);
                                local_top_files.push(sz, current_node, name_bytes, local_arena);
                                record_file_ext(
                                    local_ext_stats,
                                    name_bytes,
                                    sz,
                                    state.config.collect_ext_stats,
                                );
                            }
                        }
                    }
                    DT_UNKNOWN | DT_LNK => {
                        let flags = if state.config.follow_symlinks {
                            AT_STATX_DONT_SYNC
                        } else {
                            AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC
                        };
                        let mut stx = crate::sys::Statx::default();
                        let path_c = buf_slice[name_start..].as_ptr() as *const std::ffi::c_char;
                        let res = crate::sys::sys_statx(
                            fd,
                            path_c,
                            flags,
                            STATX_TYPE | STATX_BLOCKS | STATX_INO,
                            &mut stx,
                        );
                        if res == 0 {
                            let mode = stx.stx_mode;
                            let file_type = mode & S_IFMT;
                            if file_type == S_IFDIR {
                                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                                if state.config.cross_filesystems || dev == root_dev {
                                    if state.config.follow_symlinks {
                                        if state
                                            .visited_dirs
                                            .lock()
                                            .unwrap()
                                            .insert((dev, stx.stx_ino))
                                        {
                                            sub_dirs.push(name_bytes.to_vec());
                                        }
                                    } else {
                                        sub_dirs.push(name_bytes.to_vec());
                                    }
                                }
                            } else if file_type == S_IFREG {
                                let sz = stx.stx_blocks * 512;
                                local_dir_size += sz;
                                record_file_stat(local_files, local_bytes, sz, state);
                                local_top_files.push(sz, current_node, name_bytes, local_arena);
                                record_file_ext(
                                    local_ext_stats,
                                    name_bytes,
                                    sz,
                                    state.config.collect_ext_stats,
                                );
                            }
                        }
                    }
                    _ => {}
                }

                path_stack.truncate(orig_len);
            }
        }

        local_arena.add_direct_bytes(current_node, local_dir_size);

        // Dynamic work-stealing offload to idle peers
        if sub_dirs.len() > 1 {
            let half = sub_dirs.split_off(sub_dirs.len() / 2);
            for sub_name in half {
                let child_path = if dir_bytes == b"." {
                    PathBuf::from(std::ffi::OsStr::from_bytes(&sub_name))
                } else {
                    dir_path.join(std::ffi::OsStr::from_bytes(&sub_name))
                };
                worker.push(child_path);
            }
            state.cvar.notify_all();
        }

        for sub_name in sub_dirs {
            let (child_node, next_rel_depth) = if rel_depth < state.config.max_depth {
                let next_d = rel_depth + 1;
                let node = local_arena.add_node(current_node, next_d as u16, &sub_name);
                (node, next_d)
            } else {
                (current_node, rel_depth + 1)
            };

            let orig_len = path_stack.len();
            if dir_bytes == b"." {
                path_stack.clear();
                path_stack.extend_from_slice(&sub_name);
            } else {
                if !path_stack.ends_with(b"/") && !path_stack.is_empty() {
                    path_stack.push(b'/');
                }
                path_stack.extend_from_slice(&sub_name);
            }
            let child_path = PathBuf::from(std::ffi::OsStr::from_bytes(path_stack));

            let child_fd = if sub_name.len() < 255 {
                let mut name_c = [0u8; 256];
                name_c[..sub_name.len()].copy_from_slice(&sub_name);
                name_c[sub_name.len()] = 0;
                match crate::sys::open_dir_at2(
                    fd,
                    name_c.as_ptr() as *const std::ffi::c_char,
                    !state.config.cross_filesystems,
                ) {
                    Ok(cfd) => Some(cfd),
                    Err(crate::sys::EXDEV) => {
                        path_stack.truncate(orig_len);
                        continue;
                    }
                    Err(24 /* EMFILE */) | Err(23 /* ENFILE */) => {
                        std::thread::yield_now();
                        crate::sys::open_dir_at2(
                            fd,
                            name_c.as_ptr() as *const std::ffi::c_char,
                            !state.config.cross_filesystems,
                        )
                        .ok()
                    }
                    _ => None,
                }
            } else {
                None
            };

            dual_buffer.swap();
            scan_directory_tree(
                &child_path,
                child_fd,
                child_node,
                next_rel_depth,
                state,
                worker,
                dual_buffer,
                path_stack,
                local_arena,
                local_top_files,
                local_ext_stats,
                local_files,
                local_bytes,
            );
            dual_buffer.swap();

            path_stack.truncate(orig_len);
        }

        // SAFETY: fd was opened by open_dir or open_dir_at2.
        unsafe { crate::sys::close(fd) };
    } else if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let p_bytes = path.as_os_str().as_bytes();
            let name_bytes = entry.file_name().as_os_str().as_bytes().to_vec();

            if matcher.is_excluded(p_bytes, &name_bytes) {
                continue;
            }

            let meta_res = if state.config.follow_symlinks {
                entry.metadata()
            } else {
                std::fs::symlink_metadata(&path)
            };

            if let Ok(meta) = meta_res {
                let is_on_root_dev = state.config.cross_filesystems || meta.dev() == root_dev;
                if meta.is_dir() {
                    if is_on_root_dev {
                        sub_dirs.push(name_bytes);
                    }
                } else if is_on_root_dev {
                    let sz = meta.blocks() * 512;
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

        local_arena.add_direct_bytes(current_node, local_dir_size);

        // Dynamic work-stealing offload to idle peers
        if sub_dirs.len() > 1 {
            let half = sub_dirs.split_off(sub_dirs.len() / 2);
            for sub_name in half {
                let child_path = if dir_bytes == b"." {
                    PathBuf::from(std::ffi::OsStr::from_bytes(&sub_name))
                } else {
                    dir_path.join(std::ffi::OsStr::from_bytes(&sub_name))
                };
                worker.push(child_path);
            }
            state.cvar.notify_all();
        }

        for sub_name in sub_dirs {
            let (child_node, next_rel_depth) = if rel_depth < state.config.max_depth {
                let next_d = rel_depth + 1;
                let node = local_arena.add_node(current_node, next_d as u16, &sub_name);
                (node, next_d)
            } else {
                (current_node, rel_depth + 1)
            };

            let orig_len = path_stack.len();
            if dir_bytes == b"." {
                path_stack.clear();
                path_stack.extend_from_slice(&sub_name);
            } else {
                if !path_stack.ends_with(b"/") && !path_stack.is_empty() {
                    path_stack.push(b'/');
                }
                path_stack.extend_from_slice(&sub_name);
            }
            let child_path = PathBuf::from(std::ffi::OsStr::from_bytes(path_stack));

            dual_buffer.swap();
            scan_directory_tree(
                &child_path,
                None,
                child_node,
                next_rel_depth,
                state,
                worker,
                dual_buffer,
                path_stack,
                local_arena,
                local_top_files,
                local_ext_stats,
                local_files,
                local_bytes,
            );
            dual_buffer.swap();

            path_stack.truncate(orig_len);
        }
    }
}
