use dscan::{CliOptions, run_scan};
use std::fs;
use std::os::unix::fs::symlink;
use std::process::Command;

#[test]
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
        };

        let result = run_scan(&options).expect("run_scan failed");
        assert_eq!(result.total_files, 160);
        assert!(result.total_bytes >= 160 * 4096);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
