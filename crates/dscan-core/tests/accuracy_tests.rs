use dscan_core::snapshot::build_extension_breakdown;
use dscan_core::{CliOptions, run_scan};
use std::fs::{self, File};
use std::io::Write as IoWrite;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(prefix: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dscan_acc_{prefix}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn create_file(&self, rel_path: &str, content: &[u8]) -> PathBuf {
        let full = self.root.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&full).unwrap();
        f.write_all(content).unwrap();
        full
    }

    #[cfg(unix)]
    fn create_sparse_file(
        &self,
        rel_path: &str,
        logical_size: u64,
        written_bytes: &[u8],
    ) -> PathBuf {
        let full = self.root.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let f = File::create(&full).unwrap();
        f.set_len(logical_size).unwrap();
        if !written_bytes.is_empty() {
            let offset = logical_size.saturating_sub(written_bytes.len() as u64);
            f.write_all_at(written_bytes, offset).unwrap();
        }
        full
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
#[cfg(unix)]
fn test_sparse_file_allocated_blocks_accounting() {
    let tree = TempTree::new("sparse");
    // 10 MiB logical size, but only 4 KiB written at the end
    let chunk = vec![0x5Au8; 4096];
    tree.create_sparse_file("sparse.dat", 10 * 1024 * 1024, &chunk);

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 10,
        max_depth: 3,
        threads: 2,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: true,
    };

    let result = run_scan(&opts).expect("scan should succeed");
    assert_eq!(result.total_files, 1);
    // Allocated space on disk should be <= 1 MiB (typically 4KB-8KB), definitely NOT 10 MiB
    assert!(
        result.total_bytes < 1024 * 1024,
        "Sparse file must report allocated blocks ({}), not 10 MiB logical",
        result.total_bytes
    );
    assert!(result.total_bytes > 0, "Allocated bytes must be non-zero");
}

#[test]
fn test_hierarchical_parent_greater_or_equal_children() {
    let tree = TempTree::new("hierarchy");
    // Level 1: dir_a, dir_b
    // Level 2: dir_a/sub_a1, dir_a/sub_a2, dir_b/sub_b1
    // Level 3: dir_a/sub_a1/deep
    tree.create_file("root_file.txt", &vec![0x11; 8192]);
    tree.create_file("dir_a/a_direct.rs", &vec![0x22; 16384]);
    tree.create_file("dir_a/sub_a1/sub1.rs", &vec![0x33; 32768]);
    tree.create_file("dir_a/sub_a1/deep/deep.bin", &vec![0x44; 65536]);
    tree.create_file("dir_a/sub_a2/sub2.txt", &vec![0x55; 4096]);
    tree.create_file("dir_b/b_direct.log", &vec![0x66; 2048]);
    tree.create_file("dir_b/sub_b1/b1.json", &vec![0x77; 10240]);

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 50,
        max_depth: 6,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: true,
    };

    let result = run_scan(&opts).expect("scan should succeed");
    assert_eq!(result.total_files, 7);

    let get_dir_size = |rel: &str| -> u64 {
        let p = dscan_core::normalize_scan_path(&tree.path().join(rel));
        result
            .top_dirs
            .iter()
            .find(|(path, _)| path == &p)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    };

    let root_sz = result
        .top_dirs
        .iter()
        .find(|(path, _)| path == &dscan_core::normalize_scan_path(tree.path()))
        .map(|(_, s)| *s)
        .unwrap_or(0);

    let a_sz = get_dir_size("dir_a");
    let a1_sz = get_dir_size("dir_a/sub_a1");
    let a2_sz = get_dir_size("dir_a/sub_a2");
    let deep_sz = get_dir_size("dir_a/sub_a1/deep");
    let b_sz = get_dir_size("dir_b");
    let b1_sz = get_dir_size("dir_b/sub_b1");

    assert!(
        root_sz >= a_sz + b_sz,
        "root_sz ({root_sz}) >= a_sz ({a_sz}) + b_sz ({b_sz})"
    );
    assert!(
        a_sz >= a1_sz + a2_sz,
        "a_sz ({a_sz}) >= a1_sz ({a1_sz}) + a2_sz ({a2_sz})"
    );
    assert!(a1_sz >= deep_sz, "a1_sz ({a1_sz}) >= deep_sz ({deep_sz})");
    assert!(b_sz >= b1_sz, "b_sz ({b_sz}) >= b1_sz ({b1_sz})");
    assert_eq!(
        root_sz, result.total_bytes,
        "Root total must match result total_bytes"
    );
}

#[test]
fn test_extension_breakdown_consistency() {
    let tree = TempTree::new("exts");
    tree.create_file("code/main.rs", &vec![0xAA; 10000]);
    tree.create_file("code/lib.rs", &vec![0xAB; 20000]);
    tree.create_file("docs/readme.md", &vec![0xAC; 5000]);
    tree.create_file("docs/notes.txt", &vec![0xAD; 3000]);
    tree.create_file("binary/blob.dat", &vec![0xAE; 50000]);
    tree.create_file("binary/executable", &vec![0xAF; 12000]); // No extension

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 20,
        max_depth: 4,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: true,
    };

    let result = run_scan(&opts).expect("scan should succeed");
    assert_eq!(result.total_files, 6);

    let ext_list = build_extension_breakdown(&result.extension_stats, result.total_bytes, 100);

    let sum_ext_bytes: u64 = ext_list.iter().map(|e| e.total_bytes).sum();
    let sum_ext_files: u64 = ext_list.iter().map(|e| e.file_count).sum();

    assert_eq!(
        sum_ext_bytes, result.total_bytes,
        "Sum of extension breakdown bytes ({sum_ext_bytes}) must match total_bytes ({})",
        result.total_bytes
    );
    assert_eq!(
        sum_ext_files, result.total_files,
        "Sum of extension breakdown files ({sum_ext_files}) must match total_files ({})",
        result.total_files
    );

    // Verify presence of specific extensions
    let rs_ext = ext_list
        .iter()
        .find(|e| e.extension == ".rs")
        .expect(".rs should be present");
    assert_eq!(rs_ext.file_count, 2);

    let no_ext = ext_list
        .iter()
        .find(|e| e.extension == "[no ext]")
        .expect("[no ext] should be present");
    assert_eq!(no_ext.file_count, 1);
}

#[test]
fn test_multithreading_scan_invariance() {
    let tree = TempTree::new("threads_invar");
    for d in 0..10 {
        for f in 0..8 {
            let name = format!("dir_{d}/file_{f}.test");
            let file_size = (d + 1) * 4096;
            tree.create_file(&name, &vec![(d * 10 + f) as u8; file_size]);
        }
    }

    let mut baseline_bytes = None;
    let mut baseline_files = None;
    let mut baseline_top_dirs = None;

    for threads in [1, 2, 4, 8, 16] {
        let opts = CliOptions {
            target_path: tree.path().to_str().unwrap().to_string(),
            top_limit: 10,
            max_depth: 3,
            threads,
            excludes: vec![],
            follow_symlinks: false,
            cross_filesystems: true,
            collect_ext_stats: true,
        };

        let res = run_scan(&opts).expect("scan must succeed");

        if let Some(base_b) = baseline_bytes {
            assert_eq!(
                res.total_bytes, base_b,
                "Total bytes mismatch at threads={threads}"
            );
        } else {
            baseline_bytes = Some(res.total_bytes);
        }

        if let Some(base_f) = baseline_files {
            assert_eq!(
                res.total_files, base_f,
                "Total files mismatch at threads={threads}"
            );
        } else {
            baseline_files = Some(res.total_files);
        }

        if let Some(ref base_dirs) = baseline_top_dirs {
            assert_eq!(
                &res.top_dirs, base_dirs,
                "Top dirs mismatch at threads={threads}"
            );
        } else {
            baseline_top_dirs = Some(res.top_dirs.clone());
        }
    }
}

#[test]
fn test_top_files_sorted_descending_and_accurate() {
    let tree = TempTree::new("top_files");
    let mut created_sizes = Vec::new();
    for i in 1..=20 {
        let sz = i * 4096;
        tree.create_file(&format!("sub/file_{i}.bin"), &vec![0xFE; sz]);
        created_sizes.push(sz);
    }

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 5,
        max_depth: 3,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: false,
    };

    let result = run_scan(&opts).expect("scan must succeed");
    assert_eq!(result.top_files.len(), 5);

    // Verify strictly descending order
    for w in result.top_files.windows(2) {
        assert!(
            w[0].0 >= w[1].0,
            "Top files must be sorted descending: {} >= {}",
            w[0].0,
            w[1].0
        );
    }
}

#[test]
fn test_deep_tree_exact_mathematical_sum() {
    let tree = TempTree::new("deep_math");
    let mut expected_total_bytes = 0u64;
    let mut expected_files = 0u64;

    let mut cur_dir = tree.path().to_path_buf();
    for depth in 0..8 {
        cur_dir = cur_dir.join(format!("level_{depth}"));
        fs::create_dir_all(&cur_dir).unwrap();
        for f in 0..3 {
            let file_size = (depth + 1) * (f + 1) * 2048;
            let file_path = cur_dir.join(format!("file_{f}.dat"));
            fs::write(&file_path, vec![0x33; file_size]).unwrap();

            // Get actual allocated size for this file
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let meta = fs::metadata(&file_path).unwrap();
                expected_total_bytes += meta.blocks() * 512;
            }
            #[cfg(not(unix))]
            {
                let meta = fs::metadata(&file_path).unwrap();
                expected_total_bytes += meta.len();
            }
            expected_files += 1;
        }
    }

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 30,
        max_depth: 10,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: true,
    };

    let result = run_scan(&opts).expect("scan must succeed");
    assert_eq!(result.total_files, expected_files);
    #[cfg(unix)]
    assert_eq!(result.total_bytes, expected_total_bytes);
    #[cfg(not(unix))]
    assert!(result.total_bytes >= expected_total_bytes);
}

#[test]
fn test_complex_extensions_and_hidden_files() {
    let tree = TempTree::new("complex_exts");
    tree.create_file("sub/archive.tar.gz", &vec![0x11; 4096]);
    tree.create_file("sub/config.local.json", &vec![0x22; 4096]);
    tree.create_file(".hidden_file", &vec![0x33; 4096]);
    tree.create_file("no_ext_binary", &vec![0x44; 4096]);
    tree.create_file("UPPERCASE.PNG", &vec![0x55; 4096]);

    let opts = CliOptions {
        target_path: tree.path().to_str().unwrap().to_string(),
        top_limit: 10,
        max_depth: 3,
        threads: 2,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: true,
        collect_ext_stats: true,
    };

    let result = run_scan(&opts).expect("scan must succeed");
    assert_eq!(result.total_files, 5);

    let ext_breakdown = build_extension_breakdown(&result.extension_stats, result.total_bytes, 10);
    let get_ext_entry = |ext: &str| ext_breakdown.iter().find(|e| e.extension == ext);

    assert!(
        get_ext_entry(".gz").is_some(),
        ".gz must be extracted from .tar.gz"
    );
    assert!(
        get_ext_entry(".json").is_some(),
        ".json must be extracted from .local.json"
    );
    assert!(
        get_ext_entry(".png").is_some(),
        ".png must be lowercased from .PNG"
    );
    assert!(
        get_ext_entry("[no ext]").is_some(),
        "[no ext] must account for no_ext_binary and .hidden_file"
    );
}
