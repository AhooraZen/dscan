pub mod buffer;
#[cfg(unix)]
pub mod linux;
pub mod rollup;
pub mod state;
pub mod windows;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::arena::DirArena;
use crate::simd::FastExclusionMatcher;

pub use buffer::{AlignedBuffer, DualBuffer, FlatSubdirBuf};
pub use rollup::{execute_workers_and_rollup, worker_loop};
pub use state::{CachePadded, GlobalState, ScanConfig, ThreadLocalResult, init_scan_state};
pub use windows::WidePathStack;

pub use crate::snapshot::ScanSession;

pub type ProgressCallback = Arc<dyn Fn(u64, u64, usize, &str) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub target_path: String,
    pub excludes: Vec<String>,
    pub top_limit: usize,
    pub max_depth: usize,
    pub threads: usize,
    pub follow_symlinks: bool,
    pub cross_filesystems: bool,
    pub collect_ext_stats: bool,
}

pub type CliOptions = ScanOptions;

impl Default for ScanOptions {
    fn default() -> Self {
        #[cfg(unix)]
        let excludes = vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            "/run".to_string(),
            "/tmp".to_string(),
            ".git".to_string(),
        ];
        #[cfg(windows)]
        let excludes = vec![
            ".git".to_string(),
            "$Recycle.Bin".to_string(),
            "System Volume Information".to_string(),
            "pagefile.sys".to_string(),
            "hiberfil.sys".to_string(),
            "dumpstack.log.sys".to_string(),
        ];
        let threads = Self::auto_threads_for_path(Path::new("."));

        Self {
            target_path: ".".to_string(),
            excludes,
            top_limit: 25,
            max_depth: usize::MAX,
            threads,
            follow_symlinks: false,
            cross_filesystems: false,
            collect_ext_stats: false,
        }
    }
}

impl ScanOptions {
    pub fn auto_threads_for_path(path: &Path) -> usize {
        #[cfg(unix)]
        let cores = {
            unsafe extern "C" {
                fn sysconf(name: i32) -> i64;
            }
            let n = unsafe { sysconf(84) }; // _SC_NPROCESSORS_ONLN
            let avail = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1);
            if n > 0 {
                (n as usize).max(avail)
            } else {
                avail
            }
        };
        #[cfg(not(unix))]
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);

        #[cfg(target_os = "android")]
        {
            let _ = path;
            (cores * 2).clamp(4, 16)
        }
        #[cfg(not(target_os = "android"))]
        {
            let dev = get_path_dev(path);
            if crate::sys::is_rotational(dev, path) {
                cores.clamp(4, 8)
            } else {
                (cores * 4).clamp(8, 64)
            }
        }
    }
}

pub fn get_path_dev(path: &Path) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        path.metadata().map(|m| m.dev()).unwrap_or(0)
    }
    #[cfg(windows)]
    {
        if let Some(h) = crate::sys::open_dir(path) {
            // SAFETY: h is a valid open handle returned by open_dir.
            let dev = (unsafe { crate::sys::get_volume_serial_number(h) }).unwrap_or(0);
            // SAFETY: h is a valid open handle.
            unsafe { crate::sys::close_handle(h) };
            dev
        } else {
            0
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        0
    }
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
    pub arenas: Vec<DirArena>,
    pub root: PathBuf,
    pub extension_stats: std::collections::HashMap<String, (u64, u64)>,
    pub all_dirs: Vec<(PathBuf, u64)>,
}

#[inline(always)]
pub fn is_excluded_dir(path_bytes: &[u8], name_bytes: &[u8], excludes: &[Vec<u8>]) -> bool {
    let matcher = FastExclusionMatcher::new(excludes);
    matcher.is_excluded(path_bytes, name_bytes)
}

const BATCH_FILE_THRESHOLD: u64 = 8192;

#[inline(always)]
pub fn record_file_stat(
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

#[inline(always)]
pub fn extract_file_extension(name_bytes: &[u8]) -> String {
    if let Some(pos) = name_bytes.iter().rposition(|&b| b == b'.')
        && pos > 0
        && pos + 1 < name_bytes.len()
        && name_bytes.len() - pos <= 16
    {
        let ext = &name_bytes[pos..];
        return String::from_utf8_lossy(ext).to_lowercase();
    }
    "[no ext]".to_string()
}

#[inline(always)]
pub fn record_file_ext(
    ext_stats: &mut std::collections::HashMap<String, (u64, u64)>,
    name_bytes: &[u8],
    file_size: u64,
    collect: bool,
) {
    if !collect {
        return;
    }
    let ext = extract_file_extension(name_bytes);
    let entry = ext_stats.entry(ext).or_insert((0, 0));
    entry.0 += file_size;
    entry.1 += 1;
}

pub fn normalize_scan_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => continue,
            _ => out.push(c),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

pub fn run_scan(options: &ScanOptions) -> Result<ScanResult, std::io::Error> {
    run_scan_with_progress(options, None)
}

pub fn run_scan_with_progress(
    options: &ScanOptions,
    progress_cb: Option<ProgressCallback>,
) -> Result<ScanResult, std::io::Error> {
    let (state, workers) = init_scan_state(options)?;
    let root = Path::new(&options.target_path);
    let result = execute_workers_and_rollup(state, workers, root, options, progress_cb);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        assert_eq!(
            std::mem::align_of::<CachePadded<std::sync::atomic::AtomicU64>>(),
            64
        );
        let padded = CachePadded(std::sync::atomic::AtomicU64::new(42));
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
            collect_ext_stats: false,
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
