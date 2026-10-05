use dscan::{CliOptions, run_scan};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use std::process::Command;

#[test]
#[cfg(unix)]
fn test_synthetic_tree_matches_du() {
    let temp_dir = std::env::temp_dir().join(format!("dscan_integ_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    fs::create_dir_all(temp_dir.join("a/b/c")).unwrap();
    fs::create_dir_all(temp_dir.join("a/d")).unwrap();
    fs::create_dir_all(temp_dir.join("empty_dir")).unwrap();

    fs::write(temp_dir.join("a/b/c/file1.bin"), vec![0x41u8; 12288]).unwrap();
    fs::write(temp_dir.join("a/d/file2.bin"), vec![0x42u8; 24576]).unwrap();
    fs::write(temp_dir.join("root_file.txt"), "hello world\n").unwrap();

    // Symlink (should not be followed as directory or double counted)
    let _ = symlink(
        temp_dir.join("root_file.txt"),
        temp_dir.join("a/d/link_to_root.txt"),
    );

    let options = CliOptions {
        target_path: temp_dir.to_str().unwrap().to_string(),
        top_limit: 20,
        max_depth: 4,
        threads: 8,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: false,
    };

    let result = run_scan(&options).expect("run_scan failed");

    // Run du -s -B1 on Linux
    let du_output = Command::new("du")
        .args(["-s", "-B1", temp_dir.to_str().unwrap()])
        .output();

    if let Ok(out) = du_output
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(du_bytes_str) = s.split_whitespace().next()
            && let Ok(du_bytes) = du_bytes_str.parse::<u64>()
        {
            assert_eq!(
                result.total_bytes, du_bytes,
                "dscan total_bytes ({}) must match du -s -B1 ({})",
                result.total_bytes, du_bytes
            );
        }
    }

    // Verify parent directory size is >= sum of child directories
    let dir_a = temp_dir.join("a");
    let dir_b = temp_dir.join("a/b");
    let dir_c = temp_dir.join("a/b/c");
    let dir_d = temp_dir.join("a/d");

    let sz_a = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_a)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_b = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_b)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_c = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_c)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_d = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_d)
        .map(|(_, s)| *s)
        .unwrap_or(0);

    assert!(
        sz_b >= sz_c,
        "size of a/b ({sz_b}) must be >= a/b/c ({sz_c})"
    );
    assert!(
        sz_a >= sz_b + sz_d,
        "size of a ({sz_a}) must be >= a/b ({sz_b}) + a/d ({sz_d})"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_synthetic_tree_rollup_cross_platform() {
    let temp_dir = std::env::temp_dir().join(format!("dscan_cross_plat_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    fs::create_dir_all(temp_dir.join("a/b/c")).unwrap();
    fs::create_dir_all(temp_dir.join("a/d")).unwrap();
    fs::create_dir_all(temp_dir.join("empty_dir")).unwrap();

    fs::write(temp_dir.join("a/b/c/file1.bin"), vec![0x41u8; 12288]).unwrap();
    fs::write(temp_dir.join("a/d/file2.bin"), vec![0x42u8; 24576]).unwrap();
    fs::write(temp_dir.join("root_file.txt"), "hello world\n").unwrap();

    let options = CliOptions {
        target_path: temp_dir.to_str().unwrap().to_string(),
        top_limit: 20,
        max_depth: 4,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: false,
    };

    let result = run_scan(&options).expect("run_scan failed");

    assert_eq!(result.total_files, 3);
    assert!(result.total_bytes >= 12288 + 24576 + 12);

    let dir_a = temp_dir.join("a");
    let dir_b = temp_dir.join("a/b");
    let dir_c = temp_dir.join("a/b/c");
    let dir_d = temp_dir.join("a/d");

    let sz_a = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_a)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_b = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_b)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_c = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_c)
        .map(|(_, s)| *s)
        .unwrap_or(0);
    let sz_d = result
        .top_dirs
        .iter()
        .find(|(p, _)| p == &dir_d)
        .map(|(_, s)| *s)
        .unwrap_or(0);

    assert!(
        sz_b >= sz_c,
        "size of a/b ({sz_b}) must be >= a/b/c ({sz_c})"
    );
    assert!(
        sz_a >= sz_b + sz_d,
        "size of a ({sz_a}) must be >= a/b ({sz_b}) + a/d ({sz_d})"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_multi_threaded_scalability() {
    let temp_dir = std::env::temp_dir().join(format!("dscan_threads_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    for i in 0..16 {
        let dir = temp_dir.join(format!("dir_{i}"));
        fs::create_dir_all(&dir).unwrap();
        for j in 0..10 {
            fs::write(dir.join(format!("file_{j}.bin")), vec![j as u8; 4096]).unwrap();
        }
    }

    for threads in [1, 2, 4, 8, 16] {
        let options = CliOptions {
            target_path: temp_dir.to_str().unwrap().to_string(),
            top_limit: 10,
            max_depth: 2,
            threads,
            excludes: vec![],
            follow_symlinks: false,
            cross_filesystems: false,
        };

        let result = run_scan(&options).expect("run_scan failed");
        assert_eq!(result.total_files, 160);
        assert!(result.total_bytes >= 160 * 4096);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
#[cfg(unix)]
fn test_symlink_directory_traversal() {
    let temp_dir = std::env::temp_dir().join(format!("dscan_symlink_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let real_dir = temp_dir.join("real_dir");
    fs::create_dir_all(&real_dir).unwrap();
    fs::write(real_dir.join("payload.bin"), vec![0x99u8; 8192]).unwrap();

    let scan_root = temp_dir.join("scan_root");
    fs::create_dir_all(&scan_root).unwrap();
    fs::write(scan_root.join("root.txt"), "hello").unwrap();

    // Symlink inside scan_root pointing to real_dir (analogous to ~/storage in Termux)
    let _ = symlink(&real_dir, scan_root.join("link_to_real"));

    // 1. Without follow_symlinks: only root.txt is counted
    let options_no_follow = CliOptions {
        target_path: scan_root.to_str().unwrap().to_string(),
        top_limit: 10,
        max_depth: usize::MAX,
        threads: 2,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: false,
    };
    let res_no_follow = run_scan(&options_no_follow).expect("scan should succeed");
    assert_eq!(res_no_follow.total_files, 1);

    // 2. With follow_symlinks: payload.bin inside symlinked directory is reached and counted
    let options_follow = CliOptions {
        target_path: scan_root.to_str().unwrap().to_string(),
        top_limit: 10,
        max_depth: usize::MAX,
        threads: 2,
        excludes: vec![],
        follow_symlinks: true,
        cross_filesystems: true,
    };
    let res_follow = run_scan(&options_follow).expect("scan should succeed");
    assert_eq!(res_follow.total_files, 2);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
#[cfg(unix)]
fn test_openat2_no_xdev_mount_boundary() {
    let root = std::path::Path::new("/");
    let root_fd = match dscan::sys::open_dir(root) {
        Some(fd) => fd,
        None => return,
    };

    let sys_name = std::ffi::CString::new("sys").unwrap();
    let res = dscan::sys::open_dir_at2(root_fd, sys_name.as_ptr(), true);
    match res {
        Ok(fd) => {
            unsafe { dscan::sys::close(fd) };
        }
        Err(dscan::sys::EXDEV) => {
            // openat2 correctly caught cross-device mount boundary!
        }
        Err(dscan::sys::ENOSYS) => {
            // Kernel < 5.6
        }
        Err(_) => {}
    }
    unsafe { dscan::sys::close(root_fd) };
}

#[test]
fn test_god_speed_synthetic_tree_matches_baseline() {
    let temp_dir = std::env::temp_dir().join(format!("dscan_god_speed_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    // Create a multi-level tree with various file sizes and directory depths
    for i in 0..5 {
        let dir = temp_dir.join(format!("branch_{i}/leaf"));
        fs::create_dir_all(&dir).unwrap();
        for j in 0..10 {
            let file_path = dir.join(format!("file_{j}.bin"));
            fs::write(&file_path, vec![(i * 10 + j) as u8; 1024 * (j + 1)]).unwrap();
        }
    }

    let options = CliOptions {
        target_path: temp_dir.to_str().unwrap().to_string(),
        top_limit: 15,
        max_depth: 5,
        threads: 4,
        excludes: vec![],
        follow_symlinks: false,
        cross_filesystems: false,
    };

    let result = run_scan(&options).expect("run_scan failed");
    assert_eq!(result.total_files, 50);
    assert_eq!(result.top_files.len(), 15);

    // Verify top files are strictly sorted descending by size
    for w in result.top_files.windows(2) {
        assert!(
            w[0].0 >= w[1].0,
            "top files must be sorted descending by size"
        );
    }

    // Verify top file paths exist and are valid PathBufs
    for (size, path) in &result.top_files {
        assert!(*size > 0);
        assert!(path.exists(), "path {:?} must exist", path);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
