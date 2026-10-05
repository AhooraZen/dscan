# Plan 021: Fix Core Scanner Chase-Lev Concurrency Races, Premature Termination, and Path Separator Rollup

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- crates/dscan-core/src/work_stealing.rs crates/dscan-core/src/scanner.rs crates/dscan-core/src/arena.rs crates/dscan-core/src/sys/mod.rs crates/dscan-core/src/snapshot.rs crates/dscan-gui/src-tauri/src/commands.rs crates/dscan-core/tests/chase_lev_tests.rs crates/dscan-core/tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/019-fix-workspace-build-and-test-performance-baseline.md
- **Category**: bug
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**: 

## Why this matters
Critical concurrency, platform boundary, and path resolution bugs undermine scanner correctness in `crates/dscan-core`:
1. **Chase-Lev Double-Free & Memory Corruption**: `steal_batch` speculatively reads multiple slots with `assume_init_read()` before winning its CAS on `top`. When the local worker concurrently executes `pop()` and observes `size = b - t > 0`, the worker claims slot `b` without CAS. A thief winning a subsequent CAS claims the exact same slot, transferring ownership to both threads and causing double-free crashes on non-`Copy` types like `PathBuf`.
2. **Idle Worker Premature Termination**: Workers initialize with `is_active = thread_id == 0`. Idle threads steal tasks while `is_active == false` and delay `active_workers.fetch_add(1)` until after stealing. If the active worker drains its local queue and finds all stealers empty (because the stolen task is held in the thief's registers), it decrements `active_workers` to 0, declares the scan complete (`state.done = true`), and shuts down worker threads mid-scan, dropping subtrees.
3. **Corrupted Windows Hierarchical Rollup**: `DirArena::reconstruct_path` and `resolve_top_file_path` hardcode separator `b'/'`. On Windows, paths like `C:\Users` become `C:\Users/sub`, causing `strip_prefix` in `snapshot.rs` to fail, rendering empty treemaps and broken hierarchical rollups.
4. **Symlink Recursion Denial-of-Service**: When `follow_symlinks: true`, `DT_LNK` resolving to `S_IFDIR` pushes subdirectories without checking visited device/inode identities. Circular symlinks cause infinite directory recursion, memory exhaustion, and descriptor leaks.
5. **Platform Gate Violations**: `crates/dscan-core/src/sys/mod.rs` binds `linux.rs` under `#[cfg(unix)]`, exposing raw Linux syscall numbers (`SYS_GETDENTS64`, `SYS_STATX`, `openat2`) to macOS and FreeBSD where they cause invalid syscall traps or build failures.
6. **Treemap View Starvation**: `ScanResult.top_dirs` truncates `sorted_dirs` to `options.top_limit` (default 25) at `scanner.rs:1801`. The GUI treemap receives at most 25 directory nodes across the entire scanned filesystem.

Fixing these six issues ensures memory safety under high multi-threaded contention, prevents premature termination, enables accurate cross-platform path rollup, and supplies complete directory trees to GUI consumers.

## Current state
- `crates/dscan-core/src/work_stealing.rs:247-293`: `steal_batch` reads multiple slots before CAS:
  ```rust
  let count = ((size as usize) / 2).max(1).min(max_cap);
  let a = self.inner.array.load(Ordering::Acquire);

  let mut items: [MaybeUninit<T>; 32] = [const { MaybeUninit::uninit() }; 32];

  for (i, item) in items.iter_mut().take(count).enumerate() {
      let slot = t.wrapping_add(i as isize);
      // SAFETY: slot is within [t, b) observed bounds.
      let val = unsafe { (*a).read(slot) };
      item.write(val);
  }

  let new_t = t.wrapping_add(count as isize);
  if self
      .inner
      .top
      .compare_exchange(t, new_t, Ordering::SeqCst, Ordering::Relaxed)
      .is_ok()
  { ... }
  ```
- `crates/dscan-core/src/scanner.rs:1371, 1413-1417, 1492-1505`: Worker lifecycle allows active count to hit 0 while tasks are in flight:
  ```rust
  let mut is_active = thread_id == 0;
  ...
  if let Some(current_dir) = task {
      if !is_active {
          state.active_workers.fetch_add(1, Ordering::SeqCst);
          is_active = true;
      }
  ...
  } else {
      if is_active {
          state.active_workers.fetch_sub(1, Ordering::SeqCst);
          is_active = false;
      }
      if state.active_workers.load(Ordering::SeqCst) == 0 && state.all_stealers_empty() {
          state.done.store(true, Ordering::SeqCst);
          state.cvar.notify_all();
          break;
      }
  ```
- `crates/dscan-core/src/arena.rs:243-250`: `reconstruct_path` hardcodes `b'/'`:
  ```rust
  for (i, &idx) in chain.iter().enumerate() {
      let name = self.name_of(idx);
      if i > 0 && !out.ends_with(b"/") && !name.starts_with(b"/") {
          out.push(b'/');
      }
      out.extend_from_slice(name);
  }
  ```
- `crates/dscan-core/src/scanner.rs:778-800`: Symlink directory resolution lacks cycle detection:
  ```rust
  let file_type = mode & S_IFMT;
  if file_type == S_IFDIR {
      let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
      if state.config.cross_filesystems || dev == root_dev {
          sub_dirs.push(name_bytes.to_vec());
      }
  }
  ```
- `crates/dscan-core/src/sys/mod.rs:1-5`: Unconditional Unix gate:
  ```rust
  #[cfg(unix)]
  mod linux;
  #[cfg(unix)]
  pub use linux::*;
  ```
- `crates/dscan-core/src/scanner.rs:1801-1802`: Truncates `top_dirs` to `top_limit` (25):
  ```rust
  ScanResult {
      elapsed,
      total_bytes,
      total_files,
      top_dirs: sorted_dirs.into_iter().take(options.top_limit).collect(),
      top_files: files_vec.into_iter().take(options.top_limit).collect(),
  ```

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build core | `cargo build -p dscan-core` | exit 0 |
| Test core | `cargo test -p dscan-core` | exit 0, all passed |
| Test work stealing | `cargo test -p dscan-core --test chase_lev_tests` | exit 0, all passed |
| Check clippy | `cargo clippy -p dscan-core -- -D warnings` | exit 0, no warnings |
| Check formatting | `cargo fmt -- --check` | exit 0 |

## Suggested executor toolkit
- `rust-dev` skill for memory orderings (`SeqCst`, `Acquire`, `Release`) and lock-free concurrency invariants.
- `ponytail` discipline: clean, minimal diffs without adding third-party crates or extraneous abstractions.

## Scope
**In scope**:
- `crates/dscan-core/src/work_stealing.rs`: Replace speculative multi-slot `steal_batch` with safe CAS-loop stealing.
- `crates/dscan-core/src/scanner.rs`: Fix idle worker active count lifecycle, termination barrier under condvar lock, symlink `(dev, ino)` cycle detection, and preserve full `all_dirs` hierarchy in `ScanResult`.
- `crates/dscan-core/src/arena.rs`: Normalize path separator logic in `reconstruct_path` and `resolve_top_file_path` for Windows.
- `crates/dscan-core/src/sys/mod.rs`: Restrict `linux.rs` to `#[cfg(any(target_os = "linux", target_os = "android"))]`.
- `crates/dscan-core/src/snapshot.rs`: Update `get_hierarchical_view` to use full rolled-up directories `all_dirs`.
- `crates/dscan-gui/src-tauri/src/commands.rs`: Configure GUI `ScanOptions` with high `top_limit` for treemap visualization.
- `crates/dscan-core/tests/chase_lev_tests.rs`: Add concurrent batch steal test with non-`Copy` type (`String`).
- `crates/dscan-core/tests/scanner_integration.rs`: Add test verifying symlink cycle detection terminates cleanly.

**Out of scope**:
- `crates/dscan-cli/src/ui.rs`: Terminal rendering and table formatting remains unchanged (already handles top 25).
- `crates/dscan-core/src/simd.rs`: SIMD exclusion matcher remains unchanged.
- `Cargo.toml`: No external crates added.

## Git workflow
- Branch: `advisor/021-fix-core-scanner-chase-lev-concurrency-and-path-bugs`
- Commit message: `fix(core): resolve chase-lev batch steal race, termination barrier, path separator rollup, and symlink cycles`

---

## Steps

### Step 1: Fix Chase-Lev `steal_batch` Race Condition and Double-Free in `work_stealing.rs`

In `crates/dscan-core/src/work_stealing.rs:247-293`, replace the speculative multi-slot array read with a CAS-guaranteed steal loop.

**Root Cause**: Speculatively reading multiple slots (`(*a).read(slot)`) before winning `top.compare_exchange` creates a data race with the worker's `pop()`. If the worker pops from the bottom while the thief is reading, the worker does not CAS when `b - t > 0`. If the thief subsequently wins its CAS, both threads own the same allocated object, causing double-free panics when dropped.

**Target Code**:
In `crates/dscan-core/src/work_stealing.rs`, rewrite `steal_batch`:
```rust
    /// Atomically steals up to `max_batch` tasks from this stealer, transferring them
    /// directly into the caller's `dest` worker deque.
    /// Each item is safely acquired via atomic CAS on `top`, preventing data races
    /// and double-free with concurrent worker `pop()` operations.
    pub fn steal_batch(&self, dest: &Worker<T>, max_batch: usize) -> usize {
        let max_cap = max_batch.clamp(1, 32);
        let mut stolen = 0;

        while stolen < max_cap {
            match self.steal() {
                Steal::Success(item) => {
                    dest.push(item);
                    stolen += 1;
                }
                Steal::Empty => break,
                Steal::Retry => {
                    // Contention on this stealer; stop batch and allow caller to proceed
                    break;
                }
            }
        }

        stolen
    }
```

**Verify**:
`cargo test -p dscan-core --test chase_lev_tests` → all tests pass.

---

### Step 2: Fix Idle Worker Premature Termination Race in `scanner.rs`

In `crates/dscan-core/src/scanner.rs`, eliminate the race where `active_workers` drops to 0 while tasks are being stolen or held by idle workers.

**Root Cause**:
1. Workers initialized `let mut is_active = thread_id == 0`. Worker 1..N began in an inactive state.
2. When worker 1 stole a task at lines 1392-1401, it held the task in `stolen` / `task` while `is_active` remained `false`.
3. If worker 0 drained its local deque at that exact instant, worker 0 decremented `active_workers` to 0.
4. Worker 0 observed `active_workers == 0` AND `all_stealers_empty()` (because worker 1 had taken the task out of the deque), stored `state.done = true`, and exited.
5. Worker 1 then observed `state.done == true`, aborting traversal and discarding the remaining subtrees.

**Fix**:
1. All workers initialize `is_active = true`, and `active_workers` is initialized to `num_threads` in `GlobalState::new` / `init_scan_state`.
2. When a worker has no local tasks and fails to steal from all peers, it acquires `state.cvar_mutex.lock()`.
3. Under the lock:
   - Worker decrements `active_workers`.
   - Worker checks if `active_workers == 0` AND `all_stealers_empty()`. If true, this worker is the last remaining worker and all queues are confirmed empty: it sets `state.done = true`, calls `state.cvar.notify_all()`, and breaks.
   - If not done, it calls `state.cvar.wait_timeout(guard, Duration::from_millis(1))`.
   - Upon waking or resuming, it increments `active_workers` under the lock before attempting to pop or steal again.

**Target Code in `crates/dscan-core/src/scanner.rs`**:
In `init_scan_state`:
```rust
    // crates/dscan-core/src/scanner.rs:1609
    active_workers: CachePadded(AtomicUsize::new(num_threads)),
```

In `worker_loop` (`crates/dscan-core/src/scanner.rs:1371`):
```rust
    let mut is_active = true;
    let num_stealers = state.stealers.len();

    loop {
        if state.cancel_requested.load(Ordering::Relaxed) {
            break;
        }
        while state.pause_requested.load(Ordering::Relaxed)
            && !state.cancel_requested.load(Ordering::Relaxed)
        {
            thread::sleep(Duration::from_millis(10));
        }

        // 1. Try local worker deque (LIFO)
        let task = if let Some(t) = worker.pop() {
            Some(t)
        } else {
            // 2. Try bulk stealing from peers (FIFO)
            let mut stolen = None;
            for offset in 1..num_stealers {
                let target_idx = (thread_id + offset) % num_stealers;
                let count = state.stealers[target_idx].steal_batch(&worker, 16);
                if count > 0 {
                    stolen = worker.pop();
                    break;
                }
            }
            if stolen.is_none() {
                for offset in 1..num_stealers {
                    let target_idx = (thread_id + offset) % num_stealers;
                    match state.stealers[target_idx].steal() {
                        Steal::Success(t) => {
                            stolen = Some(t);
                            break;
                        }
                        Steal::Retry | Steal::Empty => continue,
                    }
                }
            }
            stolen
        };

        if let Some(current_dir) = task {
            // Ensure worker is marked active if it previously slept
            if !is_active {
                state.active_workers.fetch_add(1, Ordering::SeqCst);
                is_active = true;
            }

            if let Ok(mut act) = state.current_active.try_lock() {
                *act = current_dir.to_string_lossy().to_string();
            }
            // ... process directory ...
        } else {
            // Flush remaining counts when transitioning to idle
            if local_files > 0 {
                state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
                state.total_files.fetch_add(local_files, Ordering::Relaxed);
                local_bytes = 0;
                local_files = 0;
            }

            let mut guard = state.cvar_mutex.lock().unwrap();
            if is_active {
                state.active_workers.fetch_sub(1, Ordering::SeqCst);
                is_active = false;
            }

            if state.done.load(Ordering::SeqCst) {
                break;
            }

            if state.active_workers.load(Ordering::SeqCst) == 0 && state.all_stealers_empty() {
                state.done.store(true, Ordering::SeqCst);
                state.cvar.notify_all();
                break;
            }

            let _ = state.cvar.wait_timeout(guard, Duration::from_millis(1));

            // Upon waking, mark active under lock if we are continuing search
            if state.done.load(Ordering::SeqCst) {
                break;
            }
            if !is_active {
                state.active_workers.fetch_add(1, Ordering::SeqCst);
                is_active = true;
            }
        }
    }
```

**Verify**:
`cargo test -p dscan-core --test scanner_integration` → passes without hangs or deadlocks.

---

### Step 3: Fix Path Separator Normalization in `DirArena` (`arena.rs`)

In `crates/dscan-core/src/arena.rs:243-262`, fix separator handling in `reconstruct_path` and `resolve_top_file_path`.

**Root Cause**: Hardcoding `b'/'` causes Windows drive-letter paths (`C:\dir`) to construct malformed mixed paths (`C:\dir/subdir`). Downstream `Path::strip_prefix` fails to match components across mixed separators, corrupting hierarchical aggregation.

**Target Code in `crates/dscan-core/src/arena.rs`**:
```rust
    /// Reconstructs the full path for a directory node into `out`.
    pub fn reconstruct_path(&self, node_idx: u32, out: &mut Vec<u8>) {
        let mut curr = node_idx;
        let mut chain = Vec::with_capacity(16);
        while curr != u32::MAX && (curr as usize) < self.nodes.len() {
            chain.push(curr);
            curr = self.nodes[curr as usize].parent_idx;
        }
        chain.reverse();

        let use_backslash = cfg!(windows)
            && (!chain.is_empty() && self.name_of(chain[0]).contains(&b'\\') || out.contains(&b'\\'));
        let sep: u8 = if use_backslash { b'\\' } else { b'/' };

        for (i, &idx) in chain.iter().enumerate() {
            let name = self.name_of(idx);
            if i > 0
                && !out.ends_with(b"/")
                && !out.ends_with(b"\\")
                && !name.starts_with(b"/")
                && !name.starts_with(b"\\")
            {
                out.push(sep);
            }
            out.extend_from_slice(name);
        }
    }

    /// Reconstructs the full absolute path for a top file candidate into `out`.
    pub fn resolve_top_file_path(&self, cand: &TopFileCandidate, out: &mut Vec<u8>) {
        self.reconstruct_path(cand.dir_node_idx, out);
        let sep: u8 = if cfg!(windows) && out.contains(&b'\\') {
            b'\\'
        } else {
            b'/'
        };
        if !out.ends_with(b"/") && !out.ends_with(b"\\") && !out.is_empty() {
            out.push(sep);
        }
        if cand.is_spill == 0 {
            let len = cand.name_len as usize;
            out.extend_from_slice(&cand.name_inline[..len]);
        } else {
            let start = cand.spill_offset as usize;
            let end = start + cand.name_len as usize;
            if end <= self.names.len() {
                out.extend_from_slice(&self.names[start..end]);
            }
        }
    }
```

**Verify**:
`cargo test -p dscan-core --test arena_tests` → all tests pass.

---

### Step 4: Implement Cycle Detection for Directory Symlinks in `scanner.rs`

In `crates/dscan-core/src/scanner.rs:778-814`, track visited directory inodes when `follow_symlinks` is true to prevent infinite recursion.

**Root Cause**: When following symlinks, pointing a symlink to an ancestor directory creates a circular directory graph. Without tracking `(dev, ino)`, traversal recurses infinitely until process memory or descriptor limits are exhausted.

**Target Code in `crates/dscan-core/src/scanner.rs`**:
1. In `GlobalState`, add thread-safe cycle tracking:
```rust
    pub visited_dirs: Mutex<std::collections::HashSet<(u64, u64)>>,
```
Initialize in `init_scan_state`:
```rust
    let mut visited_dirs = std::collections::HashSet::new();
    #[cfg(unix)]
    if options.follow_symlinks {
        if let Ok(meta) = root.metadata() {
            use std::os::unix::fs::MetadataExt;
            visited_dirs.insert((meta.dev(), meta.ino()));
        }
    }
```
2. In `scan_directory_tree` (`crates/dscan-core/src/scanner.rs:778-814`), query `STATX_INO` and guard symlinked directories:
```rust
                    DT_UNKNOWN | DT_LNK => {
                        let flags = if state.config.follow_symlinks {
                            AT_STATX_DONT_SYNC
                        } else {
                            AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC
                        };
                        let mut stx = crate::sys::Statx::default();
                        let path_c = buf_slice[name_start..].as_ptr() as *const std::ffi::c_char;
                        let res = crate::sys::sys_statx(
                            fd,
                            path_c,
                            flags,
                            STATX_TYPE | STATX_BLOCKS | STATX_INO,
                            &mut stx,
                        );
                        if res == 0 {
                            let mode = stx.stx_mode;
                            let file_type = mode & S_IFMT;
                            if file_type == S_IFDIR {
                                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                                if state.config.cross_filesystems || dev == root_dev {
                                    let mut should_traverse = true;
                                    if state.config.follow_symlinks {
                                        let mut visited = state.visited_dirs.lock().unwrap();
                                        if !visited.insert((dev, stx.stx_ino)) {
                                            should_traverse = false;
                                        }
                                    }
                                    if should_traverse {
                                        sub_dirs.push(name_bytes.to_vec());
                                    }
                                }
                            } else if file_type == S_IFREG {
                                // ... file stats ...
                            }
                        }
                    }
```

**Verify**:
`cargo check -p dscan-core` → exit 0.

---

### Step 5: Strictly Gate `sys/linux.rs` to Linux and Android in `sys/mod.rs`

In `crates/dscan-core/src/sys/mod.rs`, ensure Linux syscall wrappers are compiled only on Linux and Android targets.

**Target Code in `crates/dscan-core/src/sys/mod.rs`**:
```rust
#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::*;

#[cfg(target_os = "linux")]
pub mod uring;
#[cfg(target_os = "linux")]
pub use uring::*;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;
```

For non-Linux Unix (such as macOS Darwin or FreeBSD), `scan_directory_tree` in `scanner.rs` already contains standard library `fs::read_dir` fallbacks when raw directory descriptors are unavailable.

**Verify**:
`cargo check -p dscan-core --target x86_64-unknown-linux-gnu` → exit 0.

---

### Step 6: Prevent Treemap Starvation by Preserving Full Rolled-Up Directory Hierarchy in `ScanResult`

In `crates/dscan-core/src/scanner.rs:388-399` and `1801`, preserve all rolled-up directory nodes for hierarchical viewers while retaining `top_dirs` for CLI top-N display.

**Root Cause**: Truncating `sorted_dirs` with `.take(options.top_limit)` at line 1801 discards all directories except the top 25. When `ScanSession::get_hierarchical_view` builds treemap nodes for the GUI, it only receives 25 directory nodes.

**Target Code in `crates/dscan-core/src/scanner.rs`**:
1. Add `all_dirs` to `ScanResult`:
```rust
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
```

2. In `execute_workers_and_rollup` (`crates/dscan-core/src/scanner.rs:1797-1808`):
```rust
    let top_dirs: Vec<(PathBuf, u64)> = sorted_dirs
        .iter()
        .take(options.top_limit)
        .cloned()
        .collect();

    ScanResult {
        elapsed,
        total_bytes,
        total_files,
        top_dirs,
        top_files: files_vec.into_iter().take(options.top_limit).collect(),
        max_dir_size,
        max_file_size,
        arenas,
        root: root.to_path_buf(),
        extension_stats: merged_ext_stats,
        all_dirs: sorted_dirs,
    }
```

3. In `crates/dscan-core/src/snapshot.rs:147-157`:
Update `get_hierarchical_view` to use `res.all_dirs`:
```rust
    pub fn get_hierarchical_view(&self, max_depth: u16, max_nodes: usize) -> Vec<TreemapNodeDto> {
        let guard = self.result.lock().unwrap();
        if let Some(ref res) = *guard {
            let dirs = if !res.all_dirs.is_empty() {
                &res.all_dirs
            } else {
                &res.top_dirs
            };
            build_treemap_nodes(
                &res.root,
                dirs,
                &res.top_files,
                res.total_bytes,
                max_depth,
                max_nodes,
            )
        } else { ... }
```

4. In `crates/dscan-gui/src-tauri/src/commands.rs:33-38`:
Set `top_limit: 5000` when initiating a scan from the GUI so file min-heaps in `LocalTopFiles` retain sufficient top files for detailed treemap rendering:
```rust
    let opts = ScanOptions {
        target_path: target_path.clone(),
        threads: threads.unwrap_or(default_threads),
        top_limit: 5000,
        collect_ext_stats: true,
        ..Default::default()
    };
```

**Verify**:
`cargo check -p dscan-core && cargo check -p dscan-gui` → exit 0.

---

### Step 7: Add Regression Tests for Batch Stealing, Symlink Cycles, and Windows Path Rollup

1. In `crates/dscan-core/tests/chase_lev_tests.rs`:
Add `test_chase_lev_steal_batch_non_copy_drop()` using `String` elements to guarantee that concurrent `steal_batch` and `pop()` do not cause double-free memory corruption.
2. In `crates/dscan-core/tests/scanner_integration.rs`:
Add `test_symlink_cycle_detection()` creating a recursive symlink (`dir/cycle -> dir`) and asserting scan terminates without infinite loops.
3. In `crates/dscan-core/tests/arena_tests.rs`:
Add `test_arena_path_reconstruction_windows_separators()` verifying that paths containing backslashes retain backslashes and strip prefixes cleanly.

**Verify**:
`cargo test -p dscan-core` → all tests pass.

---

## Test plan
- **Batch Steal Concurrency**:
  - Run `cargo test -p dscan-core --test chase_lev_tests` with 8 concurrent thief threads stealing `String` objects from a single worker. Must complete with 0 double-free errors.
- **Symlink Cycle Protection**:
  - Run `cargo test -p dscan-core --test scanner_integration test_symlink_cycle_detection` with circular symlinks. Must terminate within 200ms.
- **Path Normalization**:
  - Run `cargo test -p dscan-core --test arena_tests`. Assert reconstructed Windows paths do not mix `/` and `\`.
- **Full Core Test Suite**:
  - Run `cargo test -p dscan-core` → all unit and integration tests pass.

## Done criteria
- [ ] `cargo check -p dscan-core` exits 0.
- [ ] `cargo test -p dscan-core` exits 0 with zero failures.
- [ ] `chase_lev_tests` passes under multi-threaded concurrency using non-`Copy` data types.
- [ ] Symlink cycle test verifies termination without descriptor exhaustion.
- [ ] `sys/linux.rs` is compiled only under `cfg(any(target_os = "linux", target_os = "android"))`.
- [ ] `ScanResult` contains `all_dirs`, enabling complete treemap views in GUI mode.
- [ ] `cargo clippy -p dscan-core -- -D warnings` exits 0.
- [ ] `cargo fmt -- --check` exits 0.
- [ ] No files outside the in-scope list are modified (`git status`).
- [ ] `plans/README.md` status row for Plan 021 updated.

## STOP conditions
- Stop and report back if:
  - Code excerpts in `work_stealing.rs` or `scanner.rs` do not match live files.
  - Test `test_chase_lev_steal_batch_concurrent` fails or deadlocks.
  - Cross-platform build on Linux or Android fails to compile `sys_statx` bindings.

## Maintenance notes
- **Work-Stealing Guarantees**: Implementing `steal_batch` as a sequential CAS-loop preserves Chase-Lev lock-free properties: every single task transfer is validated by an atomic compare-exchange on `top`.
- **Termination Invariant**: `active_workers` transitions must always occur while synchronizing on `cvar_mutex` when checking for 0 active workers and empty deques. This guarantees that no stolen task is hidden in transit.
