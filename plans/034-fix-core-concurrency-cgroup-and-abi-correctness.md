# Plan 034: Core Concurrency, Container CFS Quota Scaling, and Cross-Platform ABI Safety

> **Executor instructions**: Follow this plan step by step. Run every verification command and confirm expected result before moving to next step. If anything in "STOP conditions" occurs, stop and report — do not improvise. When done, update status row for this plan in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat ba016d0..HEAD -- crates/dscan-core/src/scanner.rs crates/dscan-core/src/sys/linux.rs crates/dscan-core/src/sys/windows.rs crates/dscan-core/src/work_stealing.rs crates/dscan-core/tests/chase_lev_tests.rs crates/dscan-core/tests/scanner_integration.rs`
> If any in-scope file changed since plan written, compare "Current state" excerpts against live code before proceeding; on mismatch, treat as STOP condition.

## Status
- **Status**: TODO
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (strictly in-process zero-dependency concurrency optimizations, ABI stabilization, and container safety)
- **Depends on**: plans/033-extreme-scanner-performance.md
- **Category**: perf / systems / concurrency
- **Planned at**: commit `ba016d0`, 2026-10-08

## Why this matters
`dscan` parallel engine encounters three critical correctness and performance bottlenecks across containerized, non-x86_64, and high-core environments:
1. **Container CFS CPU Quota Freezing**: In Docker/Kubernetes containers pinned to 1–2 vCPUs on large multi-core hosts, `auto_threads_for_path` reads host core counts (`_SC_NPROCESSORS_ONLN`) and spawns 48–64 threads. These threads exhaust container CFS bandwidth within milliseconds, causing extreme kernel throttling and multi-second UI freezes.
2. **AArch64 Multi-Arch ABI Corruption**: Raw `KernelStat` struct in `sys/linux.rs` has an x86_64-specific layout. On AArch64 (ARM64 servers, Raspberry Pi, Android), field offset mismatches corrupt `st_blocks`, producing bogus file sizes and incorrect directory rollups.
3. **Hot-Loop Allocations, Contention, & Cycles**: Local directory recursion allocates a new `PathBuf` for 100% of visited directories (`scanner.rs:938, 1068`), directory reads allocate `Vec<Vec<u8>>` / `Vec<Vec<u16>>` for all subdirectories, work splitting triggers thundering-herd `notify_all()` wakeups on sleeping workers, `steal_batch` performs individual CAS steal loops rather than a single atomic batch transfer, and Windows NTFS directory junctions lack cycle tracking.

Resolving these issues eliminates all container throttling, guarantees exact cross-architecture ABI compliance, drops local directory heap allocations to zero, and achieves true atomic work stealing.

## Current state
- `crates/dscan-core/src/scanner.rs:80-115`:
  `auto_threads_for_path` uses `sysconf(84)` or `available_parallelism()` without checking cgroups v1 or v2 CFS quota limits.
- `crates/dscan-core/src/sys/linux.rs:277-317`:
  `KernelStat` struct uses x86_64 layout where `st_nlink` is 64-bit and precedes `st_mode`. On AArch64 Linux, `st_mode` precedes 32-bit `st_nlink`, and `st_blksize` is 32-bit with padding before `st_blocks`.
- `crates/dscan-core/src/scanner.rs:938, 1068`:
  `let child_path = PathBuf::from(std::ffi::OsStr::from_bytes(path_stack));` allocates `PathBuf` on every recursive directory step.
- `crates/dscan-core/src/scanner.rs:636, 1220`:
  `let mut sub_dirs: Vec<Vec<u8>> = Vec::with_capacity(16);` and `let mut sub_dirs_wide: Vec<Vec<u16>> = Vec::with_capacity(16);` allocate nested vectors for all directory listings.
- `crates/dscan-core/src/scanner.rs:916, 1046, 1396, 1517, 1596-1616`:
  `state.cvar.notify_all()` called unconditionally whenever subdirectories are split off, waking up all idle threads simultaneously. `active_workers` and `done` use heavy `SeqCst` ordering.
- `crates/dscan-core/src/work_stealing.rs:247-264`:
  `steal_batch` loops individual `self.steal()` calls with individual CAS operations per item instead of single-CAS batch extraction.
- `crates/dscan-core/src/scanner.rs:1301-1304`:
  Windows directory scan pushes reparse points without querying `(volume_serial, file_id)` against `state.visited_dirs`.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Check formatting | `cargo fmt --check` | exit 0 |
| Lint core and CLI | `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` | exit 0, zero warnings |
| Run all core tests | `cargo test -p dscan-core` | exit 0, all tests pass |
| Run Chase-Lev unit tests | `cargo test -p dscan-core --test chase_lev_tests` | exit 0, all tests pass |
| Run scanner integration tests | `cargo test -p dscan-core --test scanner_integration` | exit 0, all tests pass |
| Run full workspace tests | `cargo test --workspace` | exit 0, all tests pass |

## Suggested executor toolkit
- Use `rust-dev` rules for idiomatic ownership, explicit atomic memory ordering, and zero-allocation borrow patterns.
- Use `crates/dscan-core/tests/chase_lev_tests.rs` and `crates/dscan-core/tests/scanner_integration.rs` as verification baselines.

## Scope
**In scope**:
- `crates/dscan-core/src/scanner.rs`
- `crates/dscan-core/src/sys/linux.rs`
- `crates/dscan-core/src/sys/windows.rs`
- `crates/dscan-core/src/sys/mod.rs`
- `crates/dscan-core/src/work_stealing.rs`
- `crates/dscan-core/tests/chase_lev_tests.rs`
- `crates/dscan-core/tests/scanner_integration.rs`

**Out of scope**:
- No changes to `dscan-gui` UI rendering or shaders.
- No new external cargo dependencies (`crossbeam`, `rayon`, `nix` prohibited).
- No modification of CLI argument parsing flags.

## Git workflow
- Branch: `advisor/034-core-concurrency-and-abi-safety`
- Commit per step; message style: `feat(core): <action description>`
- Do NOT push or open PR unless instructed by operator.

---

## Steps

### Step 1: Implement Container CFS Quota Detection and Clamping in `sys/linux.rs` & `scanner.rs`
Read cgroup v2 `/sys/fs/cgroup/cpu.max` and cgroup v1 `/sys/fs/cgroup/cpu/cpu.cfs_quota_us` + `/sys/fs/cgroup/cpu/cpu.cfs_period_us` (also checking `/sys/fs/cgroup/cpu,cpuacct/`).

1. In `crates/dscan-core/src/sys/linux.rs` (and re-exported in `crates/dscan-core/src/sys/mod.rs`):
```rust
/// Read Linux container cgroups CFS bandwidth quota (cgroup v2 and v1).
/// Returns equivalent vCPU core count if a quota is active.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn get_cgroup_cpu_quota() -> Option<usize> {
    // 1. cgroup v2: /sys/fs/cgroup/cpu.max -> "$MAX $PERIOD"
    if let Ok(content) = std::fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        let mut parts = content.split_whitespace();
        if let (Some(quota_str), Some(period_str)) = (parts.next(), parts.next()) {
            if quota_str != "max" {
                if let (Ok(quota), Ok(period)) =
                    (quota_str.parse::<u64>(), period_str.parse::<u64>())
                {
                    if period > 0 && quota > 0 {
                        let cores = (quota + period - 1) / period;
                        return Some((cores as usize).max(1));
                    }
                }
            }
        }
    }

    // 2. cgroup v1: /sys/fs/cgroup/cpu/cpu.cfs_quota_us & cpu.cfs_period_us
    let quota_paths = [
        "/sys/fs/cgroup/cpu/cpu.cfs_quota_us",
        "/sys/fs/cgroup/cpu,cpuacct/cpu.cfs_quota_us",
    ];
    for q_path in quota_paths {
        if let Ok(quota_str) = std::fs::read_to_string(q_path) {
            if let Ok(quota) = quota_str.trim().parse::<i64>() {
                if quota > 0 {
                    let period_path = q_path.replace("cpu.cfs_quota_us", "cpu.cfs_period_us");
                    let period = std::fs::read_to_string(period_path)
                        .ok()
                        .and_then(|p| p.trim().parse::<u64>().ok())
                        .unwrap_or(100_000);
                    if period > 0 {
                        let cores = ((quota as u64) + period - 1) / period;
                        return Some((cores as usize).max(1));
                    }
                }
            }
        }
    }

    None
}
```
2. In `crates/dscan-core/src/sys/mod.rs`:
Provide dummy fallback on non-Linux platforms:
```rust
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn get_cgroup_cpu_quota() -> Option<usize> {
    None
}
```
3. In `crates/dscan-core/src/scanner.rs:80-116` (`ScanOptions::auto_threads_for_path`):
Clamp `cores` using `effective_cores = cores.min(cfs_cores)`.
```rust
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

    let effective_cores = if let Some(cfs) = crate::sys::get_cgroup_cpu_quota() {
        cores.min(cfs)
    } else {
        cores
    };

    #[cfg(target_os = "android")]
    {
        let _ = path;
        (effective_cores * 2).clamp(4, 16)
    }
    #[cfg(not(target_os = "android"))]
    {
        let dev = get_path_dev(path);
        if crate::sys::is_rotational(dev, path) {
            effective_cores.clamp(2, 4)
        } else if effective_cores <= 2 {
            (effective_cores * 2).clamp(2, 4)
        } else {
            (effective_cores * 4).clamp(8, 64)
        }
    }
}
```

**Verify**:
`cargo test -p dscan-core --lib scanner::tests::test_scan_tree_and_rollup` → `test result: ok`

---

### Step 2: Canonical Multi-Arch ABI Standardization with `sys_statx` on Linux/Android
Eliminate architecture-specific `KernelStat` layout corruption on AArch64 / ARM / RISC-V by standardizing `DT_REG` stat in `scanner.rs` on canonical 256-byte `sys_statx` (`STATX_BLOCKS | AT_STATX_DONT_SYNC | AT_SYMLINK_NOFOLLOW`).

1. In `crates/dscan-core/src/sys/linux.rs`:
Verify `Statx` struct is 256 bytes with `assert_eq!(std::mem::size_of::<Statx>(), 256);`.
2. In `crates/dscan-core/src/scanner.rs:748-773`:
Replace `sys_newfstatat` call under `DT_REG` with direct `sys_statx`:
```rust
DT_REG => {
    let mut stx = crate::sys::Statx::default();
    let path_c = buf_slice[name_start..].as_ptr() as *const std::ffi::c_char;
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
        record_file_ext(
            local_ext_stats,
            name_bytes,
            sz,
            state.config.collect_ext_stats,
        );
    }
}
```

**Verify**:
`cargo test -p dscan-core --lib sys::linux::tests::test_statx_struct_size` → `test result: ok`

---

### Step 3: Implement `FlatSubdirBuf` & Eliminate Heap Allocations in Directory Recursion
Eliminate `sub_dirs: Vec<Vec<u8>>` and `PathBuf::from(...)` heap allocations in local directory traversal.

1. In `crates/dscan-core/src/scanner.rs`:
Add reusable `FlatSubdirBuf` struct:
```rust
#[derive(Default)]
pub struct FlatSubdirBuf {
    pub bytes: Vec<u8>,
    pub entries: Vec<(u32, u32)>, // (start, len)
}

impl FlatSubdirBuf {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(512),
            entries: Vec::with_capacity(32),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, name: &[u8]) {
        let start = self.bytes.len() as u32;
        let len = name.len() as u32;
        self.bytes.extend_from_slice(name);
        self.entries.push((start, len));
    }

    #[inline(always)]
    pub fn get(&self, idx: usize) -> &[u8] {
        let (start, len) = self.entries[idx];
        &self.bytes[start as usize..(start + len) as usize]
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.entries.clear();
    }
}
```

2. In `worker_loop` in `scanner.rs`:
Allocate `let mut subdir_stack = FlatSubdirBuf::new();` once per thread worker.
Pass `subdir_stack: &mut FlatSubdirBuf` through recursive `scan_directory_tree` calls.

3. In `scan_directory_tree` (Unix):
At directory entry:
```rust
let start_entry = subdir_stack.entries.len();
let start_byte = subdir_stack.bytes.len();
```
During local recursion loop:
Zero-allocation borrowed `Path` construction from `path_stack`:
```rust
let child_path = Path::new(std::ffi::OsStr::from_bytes(path_stack.as_slice()));
```

**Verify**:
`cargo test -p dscan-core --test scanner_integration` → `test result: ok`

---

### Step 4: True Single-CAS Atomic `steal_batch` in `work_stealing.rs`
Replace the multi-CAS item loop in `Stealer::steal_batch` with a true single-CAS atomic batch steal.

In `crates/dscan-core/src/work_stealing.rs:247-264`:
```rust
/// Atomically steals up to `max_batch` tasks (up to half the victim's queue) in a single CAS.
/// Stolen tasks are directly transferred into the caller's `dest` worker deque.
/// Returns the number of items successfully stolen (0 if empty or CAS lost).
pub fn steal_batch(&self, dest: &Worker<T>, max_batch: usize) -> usize {
    let t = self.inner.top.load(Ordering::Acquire);
    fence(Ordering::SeqCst);
    let b = self.inner.bottom.load(Ordering::Acquire);

    let size = b.wrapping_sub(t);
    if size <= 0 {
        return 0;
    }

    let max_cap = max_batch.clamp(1, 32) as isize;
    let batch_size = ((size + 1) / 2).clamp(1, max_cap);
    let new_t = t.wrapping_add(batch_size);

    if self
        .inner
        .top
        .compare_exchange(t, new_t, Ordering::SeqCst, Ordering::Relaxed)
        .is_err()
    {
        return 0;
    }

    // CAS succeeded: caller exclusively owns slots [t, new_t)
    let a = self.inner.array.load(Ordering::Acquire);
    let mut curr = t;
    while curr != new_t {
        // SAFETY: caller exclusively acquired slot curr via atomic CAS on top.
        let item = unsafe { (*a).read(curr) };
        dest.push(item);
        curr = curr.wrapping_add(1);
    }

    batch_size as usize
}
```

**Verify**:
`cargo test -p dscan-core --test chase_lev_tests` → `7 passed; 0 failed`

---

### Step 5: Thundering Herd Elimination & Explicit Memory Ordering in Worker Concurrency Loop
1. Replace `cvar.notify_all()` during directory work offload with conditional `notify_one()`:
   In `scanner.rs:916, 1046, 1396`:
   ```rust
   if state.active_workers.load(Ordering::Acquire) < state.config.threads {
       state.cvar.notify_one();
   }
   ```
2. Convert worker transition atomics in `scanner.rs:1517, 1596-1616` from `Ordering::SeqCst` to explicit `AcqRel` / `Acquire` / `Release`.

**Verify**:
`cargo test -p dscan-core --test scanner_integration test_multi_threaded_scalability` → `test result: ok`

---

### Step 6: Windows NTFS Junction & Reparse Point Cycle Guard
Guard Windows directory traversal against infinite cycles on directory junctions and symlinks.

In `crates/dscan-core/src/scanner.rs:1301-1305` (`scan_directory_tree_windows`):
```rust
let attrs = entry.file_attributes;
let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

if is_dir {
    if !is_reparse {
        sub_dirs_wide.push(name_slice);
    } else if state.config.follow_symlinks {
        let vol_id = vol;
        let file_id = entry.file_id as u64;
        if state.visited_dirs.lock().unwrap().insert((vol_id, file_id)) {
            sub_dirs_wide.push(name_slice);
        }
    }
}
```

**Verify**:
`cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` → exit 0, no errors

---

## Test plan
1. **Container CFS Quota Unit Tests**:
   - Verify `auto_threads_for_path` behavior when cgroup limits are present.
2. **Work-Stealing Single-CAS Batch Tests**:
   - `test_chase_lev_steal_batch`: Verify single-CAS batch extraction preserves exact FIFO order and counts.
   - `test_chase_lev_steal_batch_concurrent`: Verify multi-thief concurrent batch stealing under contention without item loss or duplication.
3. **Multi-Threaded Traversal Scalability & Cycle Guard**:
   - `test_symlink_cycle_detection`: Verify cycle guards terminate correctly without memory blowup.
   - `test_god_speed_synthetic_tree_matches_baseline`: Verify exact file count and byte count parity.

**Verification command**:
`cargo test --workspace` → all tests pass.

---

## Done criteria
- [ ] `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` exits 0 with zero warnings.
- [ ] `cargo fmt --check` exits 0.
- [ ] `cargo test --workspace` exits 0 with all unit and integration tests passing.
- [ ] Container CFS quota detection properly clamps thread counts on cgroup v1/v2 limits.
- [ ] No `PathBuf::from(...)` heap allocations exist on local directory recursion in `scanner.rs`.
- [ ] `Stealer::steal_batch` performs a single atomic CAS operation on `top`.
- [ ] `plans/README.md` updated with Plan 034 status.

---

## STOP conditions
Stop and report back if:
- Live `scanner.rs` or `work_stealing.rs` structure differs significantly from "Current state" excerpts.
- AArch64 / Linux `Statx` struct definition fails size assertion `size_of::<Statx>() == 256`.
- `cargo test` fails twice after a reasonable bug fix.

---

## Maintenance notes
- If Linux kernel introduces new cgroup CPU controllers in future versions, update `get_cgroup_cpu_quota` paths.
- `FlatSubdirBuf` capacity defaults to 512 bytes / 32 entries and automatically grows for wide directories without reallocation on subsequent calls.

---

### Critical Files for Implementation
- `/home/ahoura/dscan/crates/dscan-core/src/scanner.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/sys/linux.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/sys/windows.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/work_stealing.rs`
- `/home/ahoura/dscan/crates/dscan-core/tests/chase_lev_tests.rs`
