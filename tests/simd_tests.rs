use dscan::simd::{FastExclusionMatcher, find_nul};
use std::ffi::CStr;

#[test]
fn test_find_nul_boundary_sizes() {
    let sizes = [
        0, 1, 2, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 255, 256,
    ];

    for &len in &sizes {
        let mut buf = vec![b'x'; len];
        assert_eq!(find_nul(&buf), len, "Non-nul buffer len {len}");

        if len > 0 {
            // Null at the very beginning
            buf[0] = 0;
            assert_eq!(find_nul(&buf), 0, "Null at 0 for len {len}");

            // Null at the very end
            buf[0] = b'x';
            buf[len - 1] = 0;
            assert_eq!(find_nul(&buf), len - 1, "Null at end for len {len}");

            // Null in the middle
            let mid = len / 2;
            buf[len - 1] = b'x';
            buf[mid] = 0;
            assert_eq!(find_nul(&buf), mid, "Null at mid {mid} for len {len}");
        }
    }
}

#[test]
fn test_find_nul_vs_cstr_exhaustive() {
    for len in 1..=128 {
        for nul_pos in 0..len {
            let mut buf = vec![b'a'; len];
            buf[nul_pos] = 0;

            let expected = CStr::from_bytes_until_nul(&buf).unwrap().to_bytes().len();
            let got = find_nul(&buf);
            assert_eq!(got, expected, "Mismatch at len={len}, nul_pos={nul_pos}");
        }
    }
}

#[test]
fn test_fast_exclusion_matcher_correctness() {
    let rules = vec![
        b".git".to_vec(),
        b"target".to_vec(),
        b"node_modules".to_vec(),
        b"/proc".to_vec(),
        b"/sys".to_vec(),
        b"/dev".to_vec(),
    ];

    let matcher = FastExclusionMatcher::new(&rules);

    // Common names
    assert!(matcher.is_excluded(b"/home/user/project/.git", b".git"));
    assert!(matcher.is_excluded(b"/home/user/project/target", b"target"));
    assert!(matcher.is_excluded(b"/home/user/project/node_modules", b"node_modules"));

    // Absolute paths
    assert!(matcher.is_excluded(b"/proc", b"proc"));
    assert!(matcher.is_excluded(b"/proc/1/stat", b"stat"));
    assert!(matcher.is_excluded(b"/sys/class/net", b"net"));
    assert!(matcher.is_excluded(b"/dev/shm", b"shm"));

    // Negative matches
    assert!(!matcher.is_excluded(b"/home/user/project/src", b"src"));
    assert!(!matcher.is_excluded(b"/home/user/project/target_old", b"target_old"));
    assert!(!matcher.is_excluded(b"/home/user/project/.github", b".github"));
    assert!(!matcher.is_excluded(b"/dev_extra", b"dev_extra"));
    assert!(!matcher.is_excluded(b"/sysfs", b"sysfs"));
}
