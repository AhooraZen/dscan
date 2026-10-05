#![cfg(target_os = "linux")]

use dscan::sys::*;
use std::fs;

#[test]
fn test_uring_batch_statx_matches_sync() {
    if !IoUringBatcher::probe_supported() {
        eprintln!("io_uring not supported on this platform, skipping");
        return;
    }

    let temp_dir = std::env::temp_dir().join(format!("dscan_uring_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let num_files = 16;
    for j in 0..num_files {
        fs::write(temp_dir.join(format!("file_{j}.bin")), vec![j as u8; 4096]).unwrap();
    }

    let fd = open_dir(&temp_dir).expect("open temp_dir");
    let mut batcher = IoUringBatcher::new(32).expect("create batcher");

    let mut stx_bufs = vec![Statx::default(); num_files];
    let mut name_bufs = vec![[0u8; 256]; num_files];

    for j in 0..num_files {
        let name = format!("file_{j}.bin");
        name_bufs[j][..name.len()].copy_from_slice(name.as_bytes());
        name_bufs[j][name.len()] = 0;

        batcher.prep_statx(
            j as u64,
            fd,
            name_bufs[j].as_ptr() as *const std::ffi::c_char,
            AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
            STATX_BLOCKS,
            &mut stx_bufs[j],
        );
    }

    let submitted = batcher.submit_and_wait(num_files as u32).expect("submit");
    assert_eq!(submitted, num_files as u32);

    let mut reaped = 0;
    batcher.reap_completions(|ud, res| {
        let j = ud as usize;
        assert_eq!(res, 0);
        assert!(stx_bufs[j].stx_blocks > 0);
        assert_eq!(stx_bufs[j].stx_size, 4096);
        reaped += 1;
    });
    assert_eq!(reaped, num_files);

    unsafe { close(fd) };
    let _ = fs::remove_dir_all(&temp_dir);
}
