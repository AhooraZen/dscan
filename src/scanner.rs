use std::cmp::Reverse;
use std::collections::BinaryHeap;
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

use crate::arena::{DirArena, LocalTopFiles, TopFileCandidate};
use crate::cli::CliOptions;
use crate::simd::FastExclusionMatcher;
#[cfg(unix)]
use crate::sys::{
    AT_STATX_DONT_SYNC, AT_SYMLINK_NOFOLLOW, DT_DIR, DT_LNK, DT_REG, DT_UNKNOWN, LinuxDirent64,
    S_IFDIR, S_IFMT, S_IFREG, STATX_BLOCKS, STATX_TYPE, SYS_GETDENTS64, open_dir,
};
use crate::ui::{clear_spinner_line, render_spinner_line};
use crate::work_stealing::{Steal, Stealer, Worker, deque};

#[derive(Debug)]
enum BufferSource {
    #[cfg(target_os = "linux")]
    HugePage {
        ptr: *mut u8,
        size: usize,
    },
    #[cfg(target_os = "linux")]
    PrePopulatedMmap {
        ptr: *mut u8,
        size: usize,
    },
    HeapLayout(std::alloc::Layout),
}

pub struct AlignedBuffer {
    ptr: *mut u8,
    source: BufferSource,
    len: usize,
}

impl AlignedBuffer {
    pub fn new(size: usize, align: usize) -> Self {
        #[cfg(target_os = "linux")]
        {
            // Try 2 MiB huge page allocation if size matches or exceeds 2 MiB
            if size >= 2 * 1024 * 1024 {
                // SAFETY: MAP_HUGETLB with anonymous private mapping.
                let ptr = unsafe {
                    crate::sys::mmap(
                        std::ptr::null_mut(),
                        size,
                        crate::sys::PROT_READ | crate::sys::PROT_WRITE,
                        crate::sys::MAP_PRIVATE
                            | crate::sys::MAP_ANONYMOUS
                            | crate::sys::MAP_HUGETLB
                            | crate::sys::MAP_POPULATE,
                        -1,
                        0,
                    )
                };
                if ptr != crate::sys::MAP_FAILED && !ptr.is_null() {
                    return AlignedBuffer {
                        ptr: ptr as *mut u8,
                        source: BufferSource::HugePage {
                            ptr: ptr as *mut u8,
                            size,
                        },
                        len: size,
                    };
                }
            }

            // Fallback to pre-populated anonymous mmap with transparent huge page madvise
            // SAFETY: Anonymous private mapping with MAP_POPULATE.
            let ptr = unsafe {
                crate::sys::mmap(
                    std::ptr::null_mut(),
                    size,
                    crate::sys::PROT_READ | crate::sys::PROT_WRITE,
                    crate::sys::MAP_PRIVATE | crate::sys::MAP_ANONYMOUS | crate::sys::MAP_POPULATE,
                    -1,
                    0,
                )
            };
            if ptr != crate::sys::MAP_FAILED && !ptr.is_null() {
                // SAFETY: ptr is valid mapped memory.
                unsafe {
                    crate::sys::madvise(ptr, size, crate::sys::MADV_HUGEPAGE);
                }
                return AlignedBuffer {
                    ptr: ptr as *mut u8,
                    source: BufferSource::PrePopulatedMmap {
                        ptr: ptr as *mut u8,
                        size,
                    },
                    len: size,
                };
            }
        }

        // Standard heap allocation fallback
        let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
        // SAFETY: layout has non-zero size and valid power-of-two alignment.
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        AlignedBuffer {
            ptr,
            source: BufferSource::HeapLayout(layout),
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
        match self.source {
            #[cfg(target_os = "linux")]
            BufferSource::HugePage { ptr, size } | BufferSource::PrePopulatedMmap { ptr, size } => {
                // SAFETY: ptr was allocated by mmap and size matches.
                unsafe {
                    crate::sys::munmap(ptr as *mut std::ffi::c_void, size);
                }
            }
            BufferSource::HeapLayout(layout) => {
                // SAFETY: self.ptr was allocated with self.layout by std::alloc::alloc.
                unsafe { std::alloc::dealloc(self.ptr, layout) };
            }
        }
    }
}

// SAFETY: AlignedBuffer owns its memory allocation and can be transferred across threads.
unsafe impl Send for AlignedBuffer {}

pub struct DualBuffer {
    pub buf_a: AlignedBuffer,
    pub buf_b: AlignedBuffer,
    pub active_is_a: bool,
}

impl DualBuffer {
    pub fn new(capacity: usize, align: usize) -> Self {
        Self {
            buf_a: AlignedBuffer::new(capacity, align),
            buf_b: AlignedBuffer::new(capacity, align),
            active_is_a: true,
        }
    }

    #[inline(always)]
    pub fn current_mut(&mut self) -> &mut AlignedBuffer {
        if self.active_is_a {
            &mut self.buf_a
        } else {
            &mut self.buf_b
        }
    }

    #[inline(always)]
    pub fn swap(&mut self) {
        self.active_is_a = !self.active_is_a;
    }
}

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
    pub matcher: FastExclusionMatcher,
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
    pub dir_sizes: Vec<(PathBuf, u64)>,
    pub top_files: LocalTopFiles,
    pub arena: DirArena,
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
    let matcher = FastExclusionMatcher::new(excludes);
    matcher.is_excluded(path_bytes, name_bytes)
}

const BATCH_FILE_THRESHOLD: u64 = 8192;

#[inline(always)]
fn record_file_stat(
    local_files: &mut u64,
    local_bytes: &mut u64,
    file_size: u64,
    state: &Arc<GlobalState>,
) {
    *local_files += 1;
    *local_bytes += file_size;
    if *local_files >= BATCH_FILE_THRESHOLD {
        state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
        state.total_files.fetch_add(*local_files, Ordering::Relaxed);
        *local_bytes = 0;
        *local_files = 0;
    }
}

#[cfg(target_os = "linux")]
const BATCH_CAP: usize = 128;

#[cfg(target_os = "linux")]
struct BatchState {
    statx_bufs: Vec<crate::sys::Statx>,
    name_bufs: Vec<[u8; 256]>,
    name_lens: Vec<usize>,
    count: usize,
}

#[cfg(target_os = "linux")]
impl BatchState {
    fn new() -> Self {
        Self {
            statx_bufs: vec![crate::sys::Statx::default(); BATCH_CAP],
            name_bufs: vec![[0u8; 256]; BATCH_CAP],
            name_lens: vec![0; BATCH_CAP],
            count: 0,
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
#[inline]
fn flush_statx_batch(
    batcher: &mut crate::sys::uring::IoUringBatcher,
    batch: &mut BatchState,
    current_node: u32,
    local_dir_size: &mut u64,
    local_files: &mut u64,
    local_bytes: &mut u64,
    local_top_files: &mut LocalTopFiles,
    local_arena: &mut DirArena,
    state: &Arc<GlobalState>,
) {
    if batch.count == 0 {
        return;
    }

    let expected = batch.count;
    let mut reaped = 0;

    while reaped < expected {
        let to_wait = (expected - reaped) as u32;
        let _ = batcher.submit_and_wait(to_wait);
        batcher.reap_completions(|user_data, res| {
            let idx = user_data as usize;
            if idx < batch.count && res == 0 {
                let stx = &batch.statx_bufs[idx];
                let sz = stx.stx_blocks * 512;
                *local_dir_size += sz;
                record_file_stat(local_files, local_bytes, sz, state);

                let name_len = batch.name_lens[idx];
                let name_bytes = &batch.name_bufs[idx][..name_len];

                local_top_files.push(sz, current_node, name_bytes, local_arena);
            }
            reaped += 1;
        });
    }

    batch.count = 0;
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn scan_directory_tree(
    dir_path: &Path,
    dir_fd: Option<i32>,
    current_node: u32,
    rel_depth: usize,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    dual_buffer: &mut DualBuffer,
    #[cfg(target_os = "linux")] batcher: &mut Option<crate::sys::uring::IoUringBatcher>,
    #[cfg(target_os = "linux")] batch: &mut BatchState,
    path_stack: &mut Vec<u8>,
    local_arena: &mut DirArena,
    local_top_files: &mut LocalTopFiles,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    let mut local_dir_size: u64 = 0;
    let mut sub_dirs: Vec<Vec<u8>> = Vec::new();

    let root_dev = state.config.root_dev;
    let matcher = &state.config.matcher;

    let dir_bytes = dir_path.as_os_str().as_bytes();
    path_stack.clear();
    path_stack.extend_from_slice(dir_bytes);

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
                        #[cfg(target_os = "linux")]
                        if let Some(b) = batcher.as_mut() {
                            if name_bytes.len() < 255 {
                                let idx = batch.count;
                                batch.name_bufs[idx][..name_bytes.len()]
                                    .copy_from_slice(name_bytes);
                                batch.name_bufs[idx][name_bytes.len()] = 0;
                                batch.name_lens[idx] = name_bytes.len();
                                batch.statx_bufs[idx] = crate::sys::Statx::default();

                                b.prep_statx(
                                    idx as u64,
                                    fd,
                                    batch.name_bufs[idx].as_ptr() as *const std::ffi::c_char,
                                    AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
                                    STATX_BLOCKS,
                                    &mut batch.statx_bufs[idx],
                                );
                                batch.count += 1;

                                if batch.count == BATCH_CAP {
                                    flush_statx_batch(
                                        b,
                                        batch,
                                        current_node,
                                        &mut local_dir_size,
                                        local_files,
                                        local_bytes,
                                        local_top_files,
                                        local_arena,
                                        state,
                                    );
                                }
                            } else {
                                // Fallback for very long filenames (>254 bytes)
                                let mut stx = crate::sys::Statx::default();
                                let mut long_c = name_bytes.to_vec();
                                long_c.push(0);
                                let res = crate::sys::sys_statx(
                                    fd,
                                    long_c.as_ptr() as *const std::ffi::c_char,
                                    AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
                                    STATX_BLOCKS,
                                    &mut stx,
                                );
                                if res == 0 {
                                    let sz = stx.stx_blocks * 512;
                                    local_dir_size += sz;
                                    record_file_stat(local_files, local_bytes, sz, state);
                                    local_top_files.push(sz, current_node, name_bytes, local_arena);
                                }
                            }
                        } else {
                            let mut stx = crate::sys::Statx::default();
                            let mut name_c = [0u8; 256];
                            let path_c = if name_bytes.len() < 255 {
                                name_c[..name_bytes.len()].copy_from_slice(name_bytes);
                                name_c[name_bytes.len()] = 0;
                                name_c.as_ptr() as *const std::ffi::c_char
                            } else {
                                let mut long_c = name_bytes.to_vec();
                                long_c.push(0);
                                long_c.as_ptr() as *const std::ffi::c_char
                            };

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
                            }
                        }

                        #[cfg(not(target_os = "linux"))]
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
                            STATX_TYPE | STATX_BLOCKS,
                            &mut stx,
                        );
                        if res == 0 {
                            let mode = stx.stx_mode;
                            let file_type = mode & S_IFMT;
                            if file_type == S_IFDIR {
                                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                                if state.config.cross_filesystems || dev == root_dev {
                                    sub_dirs.push(name_bytes.to_vec());
                                }
                            } else if file_type == S_IFREG {
                                let sz = stx.stx_blocks * 512;
                                local_dir_size += sz;
                                record_file_stat(local_files, local_bytes, sz, state);
                                local_top_files.push(sz, current_node, name_bytes, local_arena);
                            }
                        }
                    }
                    _ => {}
                }

                path_stack.truncate(orig_len);
            }
        }

        #[cfg(target_os = "linux")]
        if let Some(b) = batcher.as_mut() {
            flush_statx_batch(
                b,
                batch,
                current_node,
                &mut local_dir_size,
                local_files,
                local_bytes,
                local_top_files,
                local_arena,
                state,
            );
        }

        local_arena.add_direct_bytes(current_node, local_dir_size);

        // Dynamic work-stealing offload to idle peers
        if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads
        {
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
                #[cfg(target_os = "linux")]
                batcher,
                #[cfg(target_os = "linux")]
                batch,
                path_stack,
                local_arena,
                local_top_files,
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

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    sub_dirs.push(name_bytes);
                } else if meta.dev() == root_dev {
                    let sz = meta.blocks() * 512;
                    local_dir_size += sz;
                    record_file_stat(local_files, local_bytes, sz, state);
                    local_top_files.push(sz, current_node, &name_bytes, local_arena);
                }
            }
        }

        local_arena.add_direct_bytes(current_node, local_dir_size);

        // Dynamic work-stealing offload to idle peers
        if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads
        {
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
                #[cfg(target_os = "linux")]
                batcher,
                #[cfg(target_os = "linux")]
                batch,
                path_stack,
                local_arena,
                local_top_files,
                local_files,
                local_bytes,
            );
            dual_buffer.swap();

            path_stack.truncate(orig_len);
        }
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn scan_directory_tree_windows(
    dir_path: &Path,
    current_node: u32,
    rel_depth: usize,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    dual_buffer: &mut DualBuffer,
    scratch: &mut Vec<u16>,
    local_arena: &mut DirArena,
    local_top_files: &mut LocalTopFiles,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    use crate::sys::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut local_dir_size: u64 = 0;
    let mut sub_dirs: Vec<PathBuf> = Vec::new();

    let root_dev = state.config.root_dev;
    let matcher = &state.config.matcher;

    if let Some(h_dir) = open_dir_scratch(dir_path, scratch) {
        if !state.config.cross_filesystems
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
            // SAFETY: h_dir is a valid directory handle, buf_ptr is a valid aligned buffer pointer.
            let (status, info_bytes) =
                unsafe { sys_nt_query_directory_file(h_dir, buf_ptr, buf_len, restart_scan) };

            if status != STATUS_SUCCESS || info_bytes == 0 {
                break;
            }
            restart_scan = false;

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
                    let os_name_str = os_name.to_string_lossy();

                    let excluded =
                        matcher.is_excluded(child_path_str.as_bytes(), os_name_str.as_bytes());

                    if !excluded {
                        let attrs = entry.file_attributes;
                        let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                        let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                        if is_dir {
                            if !is_reparse || state.config.follow_symlinks {
                                sub_dirs.push(child_path);
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
                                os_name_str.as_bytes(),
                                local_arena,
                            );
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

            if matcher.is_excluded(&p_bytes, &name_bytes) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    sub_dirs.push(path);
                } else {
                    let sz = meta.len();
                    local_dir_size += sz;
                    record_file_stat(local_files, local_bytes, sz, state);
                    local_top_files.push(sz, current_node, &name_bytes, local_arena);
                }
            }
        }
    }

    local_arena.add_direct_bytes(current_node, local_dir_size);

    if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads {
        let half = sub_dirs.split_off(sub_dirs.len() / 2);
        for d in half {
            worker.push(d);
        }
        state.cvar.notify_all();
    }

    for sub in sub_dirs {
        let sub_name = sub.file_name().unwrap_or_default().to_string_lossy();
        let (child_node, next_rel_depth) = if rel_depth < state.config.max_depth {
            let next_d = rel_depth + 1;
            let node = local_arena.add_node(current_node, next_d as u16, sub_name.as_bytes());
            (node, next_d)
        } else {
            (current_node, rel_depth + 1)
        };

        dual_buffer.swap();
        scan_directory_tree_windows(
            &sub,
            child_node,
            next_rel_depth,
            state,
            worker,
            dual_buffer,
            scratch,
            local_arena,
            local_top_files,
            local_files,
            local_bytes,
        );
        dual_buffer.swap();
    }
}

fn worker_loop(
    thread_id: usize,
    worker: Worker<PathBuf>,
    state: Arc<GlobalState>,
) -> ThreadLocalResult {
    crate::sys::pin_thread_to_core(thread_id);

    let mut local_arena = DirArena::new();
    let mut local_top_files = LocalTopFiles::new(state.config.top_limit);
    let mut dual_buffer = DualBuffer::new(512 * 1024, 4096);
    #[cfg(unix)]
    let mut path_stack = Vec::with_capacity(4096);
    #[cfg(windows)]
    let mut scratch = Vec::with_capacity(512);

    #[cfg(target_os = "linux")]
    let mut batcher = if crate::sys::uring::IoUringBatcher::probe_supported() {
        crate::sys::uring::IoUringBatcher::new(128).ok()
    } else {
        None
    };
    #[cfg(target_os = "linux")]
    let mut batch = BatchState::new();

    let mut local_files: u64 = 0;
    let mut local_bytes: u64 = 0;

    let mut is_active = thread_id == 0;
    let num_stealers = state.stealers.len();

    loop {
        // 1. Try local worker deque (LIFO)
        let task = if let Some(t) = worker.pop() {
            Some(t)
        } else {
            // 2. Try bulk stealing from peers (FIFO)
            let mut stolen = None;
            for offset in 1..num_stealers {
                let target_idx = (thread_id + offset) % num_stealers;
                let count = state.stealers[target_idx].steal_batch(&worker, 32);
                if count > 0 {
                    stolen = worker.pop();
                    break;
                }
            }
            if stolen.is_none() {
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

            let cur_depth = current_dir.components().count();
            let rel_depth = cur_depth.saturating_sub(state.config.base_depth);

            #[cfg(unix)]
            let root_name = current_dir.as_os_str().as_bytes();
            #[cfg(windows)]
            let root_name_str = current_dir.to_string_lossy();
            #[cfg(windows)]
            let root_name = root_name_str.as_bytes();

            let task_node = local_arena.add_root(rel_depth as u16, root_name);

            #[cfg(unix)]
            scan_directory_tree(
                &current_dir,
                None,
                task_node,
                rel_depth,
                &state,
                &worker,
                &mut dual_buffer,
                #[cfg(target_os = "linux")]
                &mut batcher,
                #[cfg(target_os = "linux")]
                &mut batch,
                &mut path_stack,
                &mut local_arena,
                &mut local_top_files,
                &mut local_files,
                &mut local_bytes,
            );

            #[cfg(windows)]
            scan_directory_tree_windows(
                &current_dir,
                task_node,
                rel_depth,
                &state,
                &worker,
                &mut dual_buffer,
                &mut scratch,
                &mut local_arena,
                &mut local_top_files,
                &mut local_files,
                &mut local_bytes,
            );
        } else {
            // Flush remaining counts when transitioning to idle
            if local_files > 0 {
                state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
                state.total_files.fetch_add(local_files, Ordering::Relaxed);
                local_bytes = 0;
                local_files = 0;
            }

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

    if local_files > 0 {
        state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
        state.total_files.fetch_add(local_files, Ordering::Relaxed);
    }

    local_arena.rollup();
    let mut dir_sizes = Vec::with_capacity(local_arena.len());
    let mut path_scratch = Vec::with_capacity(512);
    for i in 0..local_arena.len() as u32 {
        let direct = local_arena.nodes[i as usize].direct_bytes;
        local_arena.reconstruct_path(i, &mut path_scratch);
        #[cfg(unix)]
        let p = PathBuf::from(std::ffi::OsStr::from_bytes(&path_scratch));
        #[cfg(windows)]
        let p = PathBuf::from(String::from_utf8_lossy(&path_scratch).as_ref());
        dir_sizes.push((p, direct));
    }

    ThreadLocalResult {
        dir_sizes,
        top_files: local_top_files,
        arena: local_arena,
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

    let matcher = FastExclusionMatcher::new(&excludes_bytes);

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
            matcher,
            excludes: excludes_bytes,
            root_dev,
            base_depth,
            max_depth: options.max_depth,
            top_limit: options.top_limit,
            threads: num_threads,
            follow_symlinks: options.follow_symlinks,
            cross_filesystems: options.cross_filesystems,
        },
        total_bytes: CachePadded(AtomicU64::new(0)),
        total_files: CachePadded(AtomicU64::new(0)),
        active_workers: CachePadded(AtomicUsize::new(1)),
        done: CachePadded(AtomicBool::new(false)),
        current_active: Mutex::new(String::new()),
        stealers,
        cvar: Condvar::new(),
        cvar_mutex: Mutex::new(()),
    });

    let start_time = Instant::now();

    // Spawn progress reporter
    let rep_state = Arc::clone(&state);
    let rep_handle = thread::spawn(move || {
        let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0;

        loop {
            thread::sleep(Duration::from_millis(60));
            if rep_state.done.load(Ordering::SeqCst) {
                break;
            }

            let cur_files = rep_state.total_files.load(Ordering::Relaxed);
            let cur_bytes = rep_state.total_bytes.load(Ordering::Relaxed);
            let workers = rep_state.active_workers.load(Ordering::Relaxed);
            let active_path = rep_state
                .current_active
                .try_lock()
                .map(|g| g.clone())
                .unwrap_or_default();

            render_spinner_line(spinners[idx], cur_bytes, cur_files, workers, &active_path);
            idx = (idx + 1) % spinners.len();
        }
    });

    // Spawn worker threads
    let mut handles = Vec::with_capacity(num_threads);
    for (i, worker) in workers.into_iter().enumerate() {
        let st = Arc::clone(&state);
        handles.push(thread::spawn(move || worker_loop(i, worker, st)));
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
    let mut merged_top_heap: BinaryHeap<Reverse<(TopFileCandidate, usize)>> =
        BinaryHeap::with_capacity(options.top_limit + 16);
    let mut all_dirs: Vec<(PathBuf, u64)> = Vec::new();
    let mut arenas = Vec::with_capacity(all_results.len());

    for (worker_idx, res) in all_results.into_iter().enumerate() {
        for (k, v) in res.dir_sizes {
            all_dirs.push((k, v));
        }
        if options.top_limit > 0 {
            for Reverse(cand) in res.top_files.heap {
                if merged_top_heap.len() >= options.top_limit {
                    if let Some(Reverse((min_cand, _))) = merged_top_heap.peek()
                        && cand.size <= min_cand.size
                    {
                        continue;
                    }
                    merged_top_heap.pop();
                }
                merged_top_heap.push(Reverse((cand, worker_idx)));
            }
        }
        arenas.push(res.arena);
    }

    // Sort all_dirs so duplicates (if any) are adjacent and can be merged in-place
    all_dirs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut deduped_dirs: Vec<(PathBuf, u64)> = Vec::with_capacity(all_dirs.len());
    for (path, sz) in all_dirs {
        if let Some(last) = deduped_dirs.last_mut()
            && last.0 == path
        {
            last.1 += sz;
        } else {
            deduped_dirs.push((path, sz));
        }
    }

    let max_depth = options.max_depth;
    let mut rolled_up: Vec<(PathBuf, u64)> = deduped_dirs.clone();

    // Hierarchical bottom-up rollup: add each directory's shallow file size to all its ancestors
    for (dir, direct_sz) in &deduped_dirs {
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
            if rel_depth <= max_depth && curr != dir {
                match rolled_up.binary_search_by(|(p, _)| p.as_path().cmp(curr)) {
                    Ok(idx) => rolled_up[idx].1 += *direct_sz,
                    Err(idx) => rolled_up.insert(idx, (curr.to_path_buf(), *direct_sz)),
                }
            }
            match curr.parent() {
                Some(p) if !p.as_os_str().is_empty() => curr = p,
                _ => break,
            }
        }
        if !hit_root && dir != root {
            match rolled_up.binary_search_by(|(p, _)| p.as_path().cmp(root)) {
                Ok(idx) => rolled_up[idx].1 += *direct_sz,
                Err(idx) => rolled_up.insert(idx, (root.to_path_buf(), *direct_sz)),
            }
        }
    }

    // Filter to rel_depth <= max_depth and sort by size descending
    let mut sorted_dirs: Vec<(PathBuf, u64)> = rolled_up
        .into_iter()
        .filter(|(p, _)| {
            let d = p.components().count().saturating_sub(base_depth);
            d <= max_depth
        })
        .collect();
    sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let max_dir_size = sorted_dirs.first().map(|(_, s)| *s).unwrap_or(1).max(1);

    let mut files_vec: Vec<(u64, PathBuf)> = Vec::with_capacity(merged_top_heap.len());
    let mut path_buf = Vec::with_capacity(512);

    for Reverse((cand, worker_idx)) in merged_top_heap {
        path_buf.clear();
        arenas[worker_idx].resolve_top_file_path(&cand, &mut path_buf);
        #[cfg(unix)]
        let pb = PathBuf::from(std::ffi::OsStr::from_bytes(&path_buf));
        #[cfg(windows)]
        let pb = PathBuf::from(String::from_utf8_lossy(&path_buf).as_ref());
        files_vec.push((cand.size, pb));
    }

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

    #[test]
    fn test_hugepage_or_mmap_buffer() {
        let mut buf_2m = AlignedBuffer::new(2 * 1024 * 1024, 2 * 1024 * 1024);
        assert_eq!(buf_2m.len(), 2 * 1024 * 1024);
        let slice = buf_2m.as_mut_slice();
        slice[0] = 0x11;
        slice[2 * 1024 * 1024 - 1] = 0x22;
        assert_eq!(slice[0], 0x11);
        assert_eq!(slice[2 * 1024 * 1024 - 1], 0x22);
    }

    #[test]
    fn test_thread_pinning() {
        let pinned = crate::sys::pin_thread_to_core(0);
        let _ = pinned;
    }

    #[test]
    fn test_dual_buffer() {
        let mut dual = DualBuffer::new(4096, 64);
        assert!(dual.active_is_a);
        dual.current_mut().as_mut_slice()[0] = 42;
        dual.swap();
        assert!(!dual.active_is_a);
        dual.current_mut().as_mut_slice()[0] = 84;
        assert_eq!(dual.current_mut().as_mut_slice()[0], 84);
        dual.swap();
        assert_eq!(dual.current_mut().as_mut_slice()[0], 42);
    }
}
