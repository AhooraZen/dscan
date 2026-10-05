use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::ffi::CStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::CliOptions;
use crate::format::format_bytes;
use crate::sys::{open_dir, LinuxDirent64, DT_DIR, DT_REG, DT_UNKNOWN, SYS_GETDENTS64};
use crate::ui::{
    clear_spinner_line, make_bar, render_spinner_line, C_BOLD, C_CYAN, C_DIM, C_GREEN, C_PURPLE,
    C_RESET, C_YELLOW,
};

pub struct ScanConfig {
    pub excludes: Vec<Vec<u8>>,
    pub root_dev: u64,
    pub base_depth: usize,
    pub max_depth: usize,
    pub top_limit: usize,
    pub threads: usize,
}

struct QueueState {
    tasks: Vec<PathBuf>,
}

pub struct GlobalState {
    pub config: ScanConfig,
    queue_mutex: Mutex<QueueState>,
    queue_cvar: Condvar,
    pub active_workers: AtomicUsize,
    pub total_bytes: AtomicU64,
    pub total_files: AtomicU64,
    pub current_active: Mutex<String>,
    pub done: AtomicBool,
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
fn is_excluded_dir(path_bytes: &[u8], name_bytes: &[u8], excludes: &[Vec<u8>]) -> bool {
    if name_bytes == b".git" {
        return true;
    }
    for ex in excludes {
        if path_bytes.windows(ex.len()).any(|w| w == ex.as_slice()) {
            return true;
        }
    }
    false
}

fn scan_directory_tree(
    dir_path: &Path,
    state: &Arc<GlobalState>,
    buffer: &mut [u8],
    local_dirs: &mut HashMap<PathBuf, u64>,
    local_top_files: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
    files_cnt: &mut u64,
) -> u64 {
    let mut total_dir_size: u64 = 0;
    let dir_bytes = dir_path.as_os_str().as_bytes();
    let mut sub_dirs = Vec::new();

    let root_dev = state.config.root_dev;
    let base_depth = state.config.base_depth;
    let max_depth = state.config.max_depth;
    let top_limit = state.config.top_limit;
    let excludes = &state.config.excludes;
    let threads = state.config.threads;

    // Fast path: raw getdents64
    if let Some(fd) = open_dir(dir_path) {
        loop {
            let nread = unsafe {
                crate::sys::syscall(
                    SYS_GETDENTS64,
                    fd as i64,
                    buffer.as_mut_ptr() as i64,
                    buffer.len() as i64,
                )
            };

            if nread <= 0 {
                break;
            }

            let mut pos = 0usize;
            let nread = nread as usize;

            while pos < nread {
                let dirent_ptr = unsafe { buffer.as_ptr().add(pos) as *const LinuxDirent64 };
                let dirent = unsafe { *dirent_ptr };
                let reclen = dirent.d_reclen as usize;

                let name_ptr = unsafe { buffer.as_ptr().add(pos + 19) as *const std::ffi::c_char };
                let name_cstr = unsafe { CStr::from_ptr(name_ptr) };
                let name_bytes = name_cstr.to_bytes();

                pos += reclen;

                if name_bytes == b"." || name_bytes == b".." {
                    continue;
                }

                let d_type = dirent.d_type;

                let mut full_path_bytes =
                    Vec::with_capacity(dir_bytes.len() + 1 + name_bytes.len());
                full_path_bytes.extend_from_slice(dir_bytes);
                if !dir_bytes.ends_with(b"/") && dir_bytes != b"." {
                    full_path_bytes.push(b'/');
                } else if dir_bytes == b"." {
                    full_path_bytes.clear();
                }
                full_path_bytes.extend_from_slice(name_bytes);

                if is_excluded_dir(&full_path_bytes, name_bytes, excludes) {
                    continue;
                }

                let item_path = if full_path_bytes.is_empty() {
                    PathBuf::from(".")
                } else {
                    PathBuf::from(std::ffi::OsStr::from_bytes(&full_path_bytes))
                };

                if d_type == DT_DIR {
                    sub_dirs.push(item_path);
                } else if d_type == DT_REG || d_type == DT_UNKNOWN {
                    if let Ok(meta) = item_path.symlink_metadata() {
                        if meta.is_dir() {
                            sub_dirs.push(item_path);
                        } else {
                            *files_cnt += 1;
                            if meta.dev() == root_dev {
                                let sz = meta.blocks() * 512;
                                total_dir_size += sz;

                                if local_top_files.len() < top_limit {
                                    local_top_files.push(Reverse((sz, item_path)));
                                } else if let Some(Reverse((min_sz, _))) = local_top_files.peek() {
                                    if sz > *min_sz {
                                        local_top_files.pop();
                                        local_top_files.push(Reverse((sz, item_path)));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
                } else {
                    *files_cnt += 1;
                    if meta.dev() == root_dev {
                        let sz = meta.blocks() * 512;
                        total_dir_size += sz;

                        if local_top_files.len() < top_limit {
                            local_top_files.push(Reverse((sz, path)));
                        } else if let Some(Reverse((min_sz, _))) = local_top_files.peek() {
                            if sz > *min_sz {
                                local_top_files.pop();
                                local_top_files.push(Reverse((sz, path)));
                            }
                        }
                    }
                }
            }
        }
    }

    // Dynamic Work-Stealing at ANY depth when workers are idle
    if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < threads {
        let mut q = state.queue_mutex.lock().unwrap();
        let half = sub_dirs.split_off(sub_dirs.len() / 2);
        for d in half {
            q.tasks.push(d);
        }
        state.queue_cvar.notify_all();
    }

    for sub in sub_dirs {
        let child_size = scan_directory_tree(
            &sub,
            state,
            buffer,
            local_dirs,
            local_top_files,
            files_cnt,
        );
        total_dir_size += child_size;
    }

    let cur_depth = dir_path.components().count();
    let rel_depth = cur_depth.saturating_sub(base_depth);
    if rel_depth <= max_depth {
        *local_dirs.entry(dir_path.to_path_buf()).or_insert(0) += total_dir_size;
    }

    total_dir_size
}

fn worker_loop(state: Arc<GlobalState>) -> ThreadLocalResult {
    let mut local_dirs: HashMap<PathBuf, u64> = HashMap::with_capacity(4096);
    let mut local_top_files: BinaryHeap<Reverse<(u64, PathBuf)>> =
        BinaryHeap::with_capacity(state.config.top_limit + 10);
    let mut heap_buffer = vec![0u8; 65536];

    let mut local_files_cnt: u64 = 0;

    loop {
        let task = {
            let mut q = state.queue_mutex.lock().unwrap();
            while q.tasks.is_empty() {
                if state.active_workers.load(Ordering::SeqCst) == 0 {
                    state.queue_cvar.notify_all();
                    return ThreadLocalResult {
                        dir_sizes: local_dirs,
                        top_files: local_top_files,
                    };
                }
                let (new_q, timeout_res) = state
                    .queue_cvar
                    .wait_timeout(q, Duration::from_millis(5))
                    .unwrap();
                q = new_q;
                if timeout_res.timed_out()
                    && q.tasks.is_empty()
                    && state.active_workers.load(Ordering::SeqCst) == 0
                {
                    state.queue_cvar.notify_all();
                    return ThreadLocalResult {
                        dir_sizes: local_dirs,
                        top_files: local_top_files,
                    };
                }
            }
            q.tasks.pop()
        };

        if let Some(current_dir) = task {
            state.active_workers.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut act) = state.current_active.try_lock() {
                *act = current_dir.to_string_lossy().to_string();
            }

            let tree_sz = scan_directory_tree(
                &current_dir,
                &state,
                &mut heap_buffer,
                &mut local_dirs,
                &mut local_top_files,
                &mut local_files_cnt,
            );

            state.total_bytes.fetch_add(tree_sz, Ordering::Relaxed);
            state.total_files.fetch_add(local_files_cnt, Ordering::Relaxed);
            local_files_cnt = 0;

            state.active_workers.fetch_sub(1, Ordering::SeqCst);
            state.queue_cvar.notify_all();
        }
    }
}

pub fn run_scan(options: &CliOptions) -> Result<ScanResult, std::io::Error> {
    let root = Path::new(&options.target_path);
    let root_meta = root.metadata()?;
    let root_dev = root_meta.dev();
    let base_depth = root.components().count();

    let excludes_bytes: Vec<Vec<u8>> = options
        .excludes
        .iter()
        .map(|s| s.as_bytes().to_vec())
        .collect();

    let initial_dirs = vec![root.to_path_buf()];

    let state = Arc::new(GlobalState {
        config: ScanConfig {
            excludes: excludes_bytes,
            root_dev,
            base_depth,
            max_depth: options.max_depth,
            top_limit: options.top_limit,
            threads: options.threads,
        },
        queue_mutex: Mutex::new(QueueState {
            tasks: initial_dirs,
        }),
        queue_cvar: Condvar::new(),
        active_workers: AtomicUsize::new(0),
        total_bytes: AtomicU64::new(0),
        total_files: AtomicU64::new(0),
        current_active: Mutex::new(String::new()),
        done: AtomicBool::new(false),
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

    let mut handles = Vec::with_capacity(options.threads);
    for _ in 0..options.threads {
        let state_clone = Arc::clone(&state);
        handles.push(thread::spawn(move || worker_loop(state_clone)));
    }

    let mut all_results = Vec::with_capacity(options.threads);
    for h in handles {
        if let Ok(res) = h.join() {
            all_results.push(res);
        }
    }

    state.done.store(true, Ordering::SeqCst);
    let _ = rep_handle.join();

    let elapsed = start_time.elapsed();
    let total_bytes = state.total_bytes.load(Ordering::SeqCst);
    let total_files = state.total_files.load(Ordering::SeqCst);

    clear_spinner_line();

    // Merge Thread-Local Results
    let mut final_dirs: HashMap<PathBuf, u64> = HashMap::new();
    let mut final_top_files: BinaryHeap<Reverse<(u64, PathBuf)>> = BinaryHeap::new();

    for res in all_results {
        for (k, v) in res.dir_sizes {
            *final_dirs.entry(k).or_insert(0) += v;
        }
        for item in res.top_files {
            final_top_files.push(item);
        }
    }

    let mut sorted_dirs: Vec<(PathBuf, u64)> = final_dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1));

    let max_dir_size = sorted_dirs.first().map(|(_, s)| *s).unwrap_or(1).max(1);

    let mut files_vec: Vec<(u64, PathBuf)> =
        final_top_files.into_iter().map(|Reverse(x)| x).collect();
    files_vec.sort_by(|a, b| b.0.cmp(&a.0));
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
    println!("{C_BOLD}{C_GREEN}╭──────────────────────────────────────────────────────────────────────────────╮{C_RESET}");
    println!(
        "{C_BOLD}{C_GREEN}│  {C_CYAN}✔ Scan Finished in {C_YELLOW}{:.2?}{C_CYAN}  │  Total Used: {C_YELLOW}{:<11}{C_CYAN}  │  Files: {C_YELLOW}{:<9}{C_GREEN}│{C_RESET}",
        result.elapsed,
        format_bytes(result.total_bytes),
        result.total_files
    );
    println!("{C_BOLD}{C_GREEN}╰──────────────────────────────────────────────────────────────────────────────╯{C_RESET}\n");

    println!("{C_BOLD}{C_CYAN}📁 Top Directories By Recursive Size:{C_RESET}");
    println!("{C_DIM}────────────────────────────────────────────────────────────────────────────────{C_RESET}");

    if result.top_dirs.is_empty() {
        println!("  {C_DIM}(no subdirectories found){C_RESET}");
    } else {
        for (path, size) in &result.top_dirs {
            let pct = (*size as f64 / result.max_dir_size as f64) * 100.0;
            let bar = make_bar(pct, 16);
            println!(
                "  {C_BOLD}{C_GREEN}{:>10}{C_RESET}  {} {C_CYAN}{}{C_RESET}",
                format_bytes(*size),
                bar,
                path.display()
            );
        }
    }

    println!("\n{C_BOLD}{C_PURPLE}📄 Top Largest Files:{C_RESET}");
    println!("{C_DIM}────────────────────────────────────────────────────────────────────────────────{C_RESET}");

    if result.top_files.is_empty() {
        println!("  {C_DIM}(no files found){C_RESET}");
    } else {
        for (size, path) in &result.top_files {
            let pct = (*size as f64 / result.max_file_size as f64) * 100.0;
            let bar = make_bar(pct, 12);
            println!(
                "  {C_BOLD}{C_YELLOW}{:>10}{C_RESET}  {} {C_PURPLE}{}{C_RESET}",
                format_bytes(*size),
                bar,
                path.display()
            );
        }
    }
    println!();
}
