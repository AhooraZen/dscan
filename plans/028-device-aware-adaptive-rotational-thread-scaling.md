# Plan 028: Device-Aware Adaptive Rotational Thread Scaling for Linux and Windows

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0330385..HEAD -- crates/dscan-core/src/sys/linux.rs crates/dscan-core/src/sys/windows.rs crates/dscan-core/src/sys/mod.rs crates/dscan-core/src/scanner.rs crates/dscan-cli/src/cli.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Status**: DONE
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (purely adaptive thread count optimization when user does not pass explicit `-j`; zero breaking changes to existing scan pipeline)
- **Depends on**: plans/025-fix-fd-exhaustion-and-prevent-silent-directory-drops.md, plans/026-extreme-performance-linear-depth-rollup-and-uring-batching.md, plans/027-fix-windows-nt-fallback-bounds-and-gui-stability.md
- **Category**: performance
- **Planned at**: commit `0330385`, 2026-10-06
- **Issue**: 

## Why this matters
Directory traversal performance on storage media is governed by mechanical vs solid-state physics:
1. **NVMe / SSD Storage (`rotational == 0` or `IncursSeekPenalty == false`)**:
   Solid-state storage has zero seek latency and leverages massive hardware parallelism (NVMe queue depth 64k across multiple controller channels). On desktop NVMe drives, scaling worker threads to `4x cores` (e.g. 32 threads on 8 cores) masks syscall and context switch latency, yielding up to **5.5x faster** completion times.
2. **Rotational Storage / Virtualized VPS Block Devices (`rotational == 1` or `IncursSeekPenalty == true`)**:
   Rotational hard drives (HDDs) and virtual disks backed by rotational spindles have physical heads that must physically seek to sectors. When 32 or 64 worker threads simultaneously issue random directory opens and `statx` calls, the disk heads thrash violently across tracks (I/O seek thrashing), resulting in high I/O wait (24%+ CPU wait) and severe degradation (e.g. 100.36 seconds on `/home/ahoura`).
   In stark contrast, running with low thread counts (`cores.clamp(1, 4)`) allows the kernel block layer and elevator scheduler to sequence requests linearly, slashing scan time from **100.36 seconds down to 3.33 seconds — an astonishing 30x speedup**.
3. **Cross-Platform Auto-Tuning**:
   Users should never be required to manually diagnose their storage technology or tune thread counts. When `--threads` is omitted:
   - On Linux: Query `/sys/dev/block/<major>:<minor>/queue/rotational` (or `../queue/rotational` for partitions).
   - On Windows: Query `IOCTL_STORAGE_QUERY_PROPERTY` with `StorageDeviceSeekPenaltyProperty` via `DeviceIoControl`. If `IncursSeekPenalty` is true, it is an HDD; otherwise an SSD/NVMe.
   - If Rotational HDD: Default threads = `cores.clamp(1, 4)`.
   - If Solid-State SSD/NVMe: Default threads = `(cores * 4).clamp(8, 64)`.
   - On Android: Maintain thermal/battery guard `(cores * 2).clamp(4, 16)`.
   - If the user explicitly passes `--threads N` or `-j N`, honor the user's manual override without interference.

## Current state
- `crates/dscan-cli/src/cli.rs:58-66`:
  ```rust
  let cores = std::thread::available_parallelism()
      .map(|n| n.get())
      .unwrap_or(8);
  #[cfg(target_os = "android")]
  let default_threads = (cores * 2).clamp(4, 16);
  #[cfg(not(target_os = "android"))]
  let default_threads = (cores * 4).clamp(8, 64);
  let mut threads = default_threads;
  ```
  `threads` defaults to `(cores * 4).clamp(8, 64)` regardless of whether the target path resides on an HDD or SSD.

## Implementation steps

### Step 1: Implement `is_rotational_device` in `crates/dscan-core/src/sys/linux.rs`
Add helper function in `crates/dscan-core/src/sys/linux.rs`:
```rust
/// Query Linux sysfs to determine if the block device is rotational (HDD) or solid-state (SSD/NVMe).
/// Checks /sys/dev/block/<major>:<minor>/queue/rotational and parent partition fallback.
pub fn is_rotational_device(dev: u64) -> bool {
    let major = dev_major(dev);
    let minor = dev_minor(dev);

    // 1. Check direct device path (e.g. whole disk)
    let p1 = format!("/sys/dev/block/{}:{}/queue/rotational", major, minor);
    if let Ok(content) = std::fs::read_to_string(&p1) {
        return content.trim() == "1";
    }

    // 2. Check parent device path (e.g. partition sda1 -> sda)
    let p2 = format!("/sys/dev/block/{}:{}/../queue/rotational", major, minor);
    if let Ok(content) = std::fs::read_to_string(&p2) {
        return content.trim() == "1";
    }

    false
}
```

### Step 2: Implement `is_rotational_path` in `crates/dscan-core/src/sys/windows.rs`
Add helper function in `crates/dscan-core/src/sys/windows.rs` using raw Win32 `DeviceIoControl`:
```rust
/// Query Windows volume to determine if the storage device incurs a seek penalty (HDD) or not (SSD/NVMe).
/// Uses IOCTL_STORAGE_QUERY_PROPERTY with StorageDeviceSeekPenaltyProperty.
pub fn is_rotational_path(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    // Resolve volume root e.g. "\\\\.\\C:"
    let path_str = path.to_string_lossy();
    let volume_drive = if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
        format!("\\\\.\\{}:", path_str.chars().next().unwrap_or('C'))
    } else {
        "\\\\.\\C:".to_string()
    };

    let mut wide: Vec<u16> = std::ffi::OsStr::new(&volume_drive)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Open volume handle
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0, // No access rights required for query device properties
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return false;
    }

    #[repr(C)]
    struct StoragePropertyQuery {
        property_id: u32,
        query_type: u32,
        additional_parameters: [u8; 1],
    }

    #[repr(C)]
    struct DeviceSeekPenaltyDescriptor {
        version: u32,
        size: u32,
        incurs_seek_penalty: u8,
    }

    const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: u32 = 7;
    const PROPERTY_STANDARD_QUERY: u32 = 0;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;

    let query = StoragePropertyQuery {
        property_id: STORAGE_DEVICE_SEEK_PENALTY_PROPERTY,
        query_type: PROPERTY_STANDARD_QUERY,
        additional_parameters: [0],
    };

    let mut desc = DeviceSeekPenaltyDescriptor {
        version: 0,
        size: 0,
        incurs_seek_penalty: 0,
    };

    let mut bytes_returned = 0u32;
    let res = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            &mut desc as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<DeviceSeekPenaltyDescriptor>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    unsafe { CloseHandle(handle) };

    if res != 0 {
        desc.incurs_seek_penalty != 0
    } else {
        false
    }
}
```

### Step 3: Expose Unified `is_rotational` in `crates/dscan-core/src/sys/mod.rs`
In `crates/dscan-core/src/sys/mod.rs`:
```rust
#[cfg(target_os = "linux")]
pub fn is_rotational(dev: u64, _path: &std::path::Path) -> bool {
    linux::is_rotational_device(dev)
}

#[cfg(target_os = "windows")]
pub fn is_rotational(_dev: u64, path: &std::path::Path) -> bool {
    windows::is_rotational_path(path)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn is_rotational(_dev: u64, _path: &std::path::Path) -> bool {
    false
}
```

### Step 4: Add Adaptive Auto-Tuning to `dscan-cli` and `dscan-core`
In `crates/dscan-cli/src/cli.rs`:
Track whether user explicitly passed `--threads` / `-j` flag:
```rust
let mut user_threads: Option<usize> = None;
```
When parsing `--threads` or `-j`:
```rust
"--threads" | "-j" => {
    if let Some(n) = parse_val(args, &mut i).and_then(|v| v.parse::<usize>().ok()) {
        user_threads = Some(n.clamp(1, 64));
    }
    i += 1;
}
```
If `user_threads` is `None`, dynamically probe the target path:
```rust
let threads = if let Some(n) = user_threads {
    n
} else {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    #[cfg(target_os = "android")]
    {
        (cores * 2).clamp(4, 16)
    }
    #[cfg(not(target_os = "android"))]
    {
        let target_p = std::path::Path::new(&target_path);
        let root_dev = dscan_core::get_path_dev(target_p);
        if dscan_core::is_rotational(root_dev, target_p) {
            cores.clamp(1, 4)
        } else {
            (cores * 4).clamp(8, 64)
        }
    }
};
```
Export helper `dscan_core::is_rotational` and `dscan_core::get_path_dev` in `crates/dscan-core/src/lib.rs`.

### Step 5: Verification and Unit Tests
1. Add `test_rotational_detection_does_not_panic` in `crates/dscan-core/src/sys/linux.rs` and `scanner_integration.rs`.
2. Run `cargo test -p dscan-core -p dscan`.
3. Check `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.
4. Update `plans/README.md` to DONE.

## Hard boundaries
- Never add external crates (like `sysinfo` or `winapi`). Use stdlib and existing FFI definitions only.
- Respect explicit user input: `--threads` / `-j` overrides auto-tuning unconditionally.
