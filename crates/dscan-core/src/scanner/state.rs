use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::{Arc, Condvar, Mutex};

use crate::arena::{DirArena, LocalTopFiles};
use crate::simd::FastExclusionMatcher;
use crate::work_stealing::{Stealer, Worker, deque};

use super::ScanOptions;

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
    pub collect_ext_stats: bool,
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
    pub cancel_requested: CachePadded<AtomicBool>,
    pub pause_requested: CachePadded<AtomicBool>,
    pub visited_dirs: Mutex<HashSet<(u64, u64)>>,
}

impl GlobalState {
    pub fn all_stealers_empty(&self) -> bool {
        self.stealers.iter().all(|s| s.is_empty())
    }
}

pub struct ThreadLocalResult {
    pub top_files: LocalTopFiles,
    pub arena: DirArena,
    pub ext_stats: std::collections::HashMap<String, (u64, u64)>,
}

pub fn init_scan_state(
    options: &ScanOptions,
) -> Result<(Arc<GlobalState>, Vec<Worker<PathBuf>>), std::io::Error> {
    crate::sys::raise_fd_limit();

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

    let visited_dirs = Mutex::new(HashSet::new());
    if options.follow_symlinks {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            visited_dirs
                .lock()
                .unwrap()
                .insert((root_meta.dev(), root_meta.ino()));
        }
    }

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
            collect_ext_stats: options.collect_ext_stats,
        },
        total_bytes: CachePadded(AtomicU64::new(0)),
        total_files: CachePadded(AtomicU64::new(0)),
        active_workers: CachePadded(AtomicUsize::new(num_threads)),
        done: CachePadded(AtomicBool::new(false)),
        current_active: Mutex::new(String::new()),
        stealers,
        cvar: Condvar::new(),
        cvar_mutex: Mutex::new(()),
        cancel_requested: CachePadded(AtomicBool::new(false)),
        pause_requested: CachePadded(AtomicBool::new(false)),
        visited_dirs,
    });

    Ok((state, workers))
}
