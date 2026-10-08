# Plan 035: Zero-Copy Subtree Arena Merge, O(N) Array Rollup, and Core Scanner Modularization

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat ba016d0..HEAD -- crates/dscan-core/src/ crates/dscan-android/src/ android/app/src/main/kotlin/com/dscan/app/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: plans/033-extreme-scanner-performance.md
- **Category**: perf | tech-debt
- **Planned at**: commit `ba016d0`, `2026-10-08`
- **Issue**: 

## Why this matters
Post-scan rollup currently allocates millions of `PathBuf` instances in worker threads, clones paths 3 times, inserts into `HashMap<PathBuf, u64>`, and re-sorts all paths by component depth to compute recursive directory sizes, taking 300ms–800ms. In addition, `scanner.rs` has grown to 2,128 lines containing dead `io_uring` allocation overhead in worker loops and an unsafe `Box<Arc<ScanSession>>` JNI cast in `dscan-android`. Replacing the post-scan hash map with direct tree-to-tree `DirArena` grafting and backwards array rollup cuts merge time to <5ms (100x speedup), eliminates dead ring allocations, and modularizes the engine into clean, platform-specific submodules.

## Current state
- `crates/dscan-core/src/arena.rs:93-227`: `DirArena` stores flat contiguous `ArenaNode` vectors with `parent_idx`, `direct_bytes`, `total_bytes`, and `rollup()` in O(N) reverse array time, but lacks multi-arena grafting (`merge_subtree`).
- `crates/dscan-core/src/scanner.rs:1625-1636`: Worker threads discard arena rollup benefits by formatting `PathBuf` for every directory node into `dir_sizes: Vec<(PathBuf, u64)>`.
- `crates/dscan-core/src/scanner.rs:1831-1905`: Post-scan rollup creates `HashMap<PathBuf, u64>`, clones keys repeatedly, splits ancestors, sorts by depth, and simulates tree rollup in hash maps.
- `crates/dscan-core/src/scanner.rs:1459-1466` & `539-610`: Worker loops allocate dead `IoUringBatcher` instances and `BatchState` buffers even though synchronous `sys_newfstatat` handles `DT_REG` directly.
- `crates/dscan-android/src/lib.rs:66-70, 90, 251`: Android JNI passes `Box<Arc<ScanSession>>` as `jlong` but casts to `&Arc<ScanSession>`, causing a double-indirection pointer mismatch, and serializes JSON strings every 100ms for numeric progress metrics.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Workspace Test | `cargo test` | all test suites pass, exit 0 |
| Android Crate Test | `cargo test -p dscan-android` | 1 passed, exit 0 |
| Core Integration Tests | `cargo test --test scanner_integration` | 12 passed, exit 0 |
| Core Arena Tests | `cargo test --test arena_tests` | all passed, exit 0 |
| Clippy & Linter | `cargo clippy --all-targets -- -D warnings` | exit 0, zero warnings |
| Formatting Check | `cargo fmt --check` | exit 0 |

## Scope
**In scope**:
- `crates/dscan-core/src/arena.rs` — add `DirArena::merge_subtree` and parent remapping.
- `crates/dscan-core/src/scanner.rs` — remove monolithic file, replace with modular directory.
- `crates/dscan-core/src/scanner/mod.rs` (new) — public API exports and scan entry points.
- `crates/dscan-core/src/scanner/state.rs` (new) — concurrency primitives and `GlobalState`.
- `crates/dscan-core/src/scanner/buffer.rs` (new) — `AlignedBuffer`, `DualBuffer`, `FlatSubdirBuf`.
- `crates/dscan-core/src/scanner/rollup.rs` (new) — zero-copy master arena merge & top-N formatting.
- `crates/dscan-core/src/scanner/linux.rs` (new) — Linux `getdents64` and `newfstatat` traversal engine.
- `crates/dscan-core/src/scanner/windows.rs` (new) — Windows NT directory traversal engine.
- `crates/dscan-android/src/lib.rs` — correct `Arc::into_raw`/`from_raw` lifecycle and primitive array polling.
- `android/app/src/main/kotlin/com/dscan/app/DscanBridge.kt` — receive primitive array for fast progress polling.

**Out of scope**:
- Modifying `crates/dscan-core/src/sys/uring.rs` (keep low-level syscall wrappers and isolated tests).
- Modifying GUI GPUI layout logic in `crates/dscan-gui/`.
- Changing CLI output flags or argument parsers in `crates/dscan-cli/src/cli.rs`.

## Git workflow
- Branch: `advisor/035-arena-merge-scanner-modularization`
- Commit per step with conventional commits: `perf: ...`, `refactor: ...`, `fix: ...`
- Do NOT push or open PR unless instructed.

---

## Steps

### Step 1: Implement `DirArena::merge_subtree` in `crates/dscan-core/src/arena.rs`
1. Add `merge_subtree` method to `DirArena` in `crates/dscan-core/src/arena.rs`:
   - Takes `&worker_arena`, `target_master_idx: u32`, and `node_remapping: &mut Vec<u32>`.
   - Iterates worker nodes for the subtree starting at worker node `subtree_root_idx`.
   - Appends node slice and name bytes into master arena with single reallocations.
   - Remaps `parent_idx` for subtree root to `target_master_idx`, and for children to `node_remapping[child.parent_idx]`.
   - Adjusts `name_offset` by `master_names_base_offset`.
2. Add unit test `test_arena_merge_subtree_and_rollup` in `crates/dscan-core/src/arena.rs`.

**Verify**: `cargo test -p dscan-core --lib arena::tests` → all pass.

---

### Step 2: Create `crates/dscan-core/src/scanner/buffer.rs`
1. Create `crates/dscan-core/src/scanner/buffer.rs`.
2. Move buffer structs and methods from `scanner.rs`:
   - `BufferSource` enum (`MmapHuge`, `Mmap4k`, `VecAligned`).
   - `AlignedBuffer` struct with aligned allocation and hugepage fallback.
   - `DualBuffer` struct with ping-pong `swap()` and `current_mut()`.
   - `FlatSubdirBuf` struct with flat `names: Vec<u8>` and `offsets: Vec<(u32, u16)>`.
3. Add unit tests for `AlignedBuffer`, `DualBuffer`, and `FlatSubdirBuf`.

**Verify**: `cargo check -p dscan-core` → exit 0.

---

### Step 3: Create `crates/dscan-core/src/scanner/state.rs`
1. Create `crates/dscan-core/src/scanner/state.rs`.
2. Move concurrency state from `scanner.rs`:
   - `CachePadded<T>` wrapper for false-sharing mitigation.
   - `ScanConfig` holding `FastExclusionMatcher`, `root_dev`, and limits.
   - `GlobalState` containing atomic counters, stealers, and condvar.
   - `ThreadLocalResult` containing `top_files`, `arena: DirArena`, and `ext_stats` (omit `dir_sizes: Vec<(PathBuf, u64)>`).
   - `init_scan_state(options: &ScanOptions) -> Result<(Arc<GlobalState>, Vec<Worker<PathBuf>>), std::io::Error>`.

**Verify**: `cargo check -p dscan-core` → exit 0.

---

### Step 4: Create `crates/dscan-core/src/scanner/linux.rs` & Strip Dead `io_uring`
1. Create `crates/dscan-core/src/scanner/linux.rs`.
2. Implement `scan_directory_tree` for Unix/Linux:
   - Remove `IoUringBatcher`, `BatchState`, and `flush_statx_batch` parameters and calls.
   - Use `newfstatat` directly for `DT_REG` on Linux/Android.
   - Use `open_dir_at2` with relative parent file descriptors and byte stack.
   - Record stats directly to `local_arena`, `local_top_files`, and atomics.

**Verify**: `cargo check -p dscan-core` → exit 0.

---

### Step 5: Create `crates/dscan-core/src/scanner/windows.rs`
1. Create `crates/dscan-core/src/scanner/windows.rs`.
2. Move Windows traversal engine:
   - `WidePathStack` struct and wide path stack operations.
   - `scan_directory_tree_windows` using `NtQueryDirectoryFile` and UTF-16 SIMD transcoding.

**Verify**: `cargo check -p dscan-core` → exit 0.

---

### Step 6: Create `crates/dscan-core/src/scanner/rollup.rs`
1. Create `crates/dscan-core/src/scanner/rollup.rs`.
2. Implement `worker_loop`:
   - Run worker loop pop/steal logic.
   - Return `ThreadLocalResult { top_files: local_top_files, arena: local_arena, ext_stats: local_ext_stats }` without path formatting.
3. Implement `execute_workers_and_rollup`:
   - Collect `ThreadLocalResult` from all worker join handles.
   - Merge worker arenas into master arena using `DirArena::merge_subtree` and path resolution.
   - Remap `TopFileCandidate` directory indices and merge top-files heap.
   - Call `master_arena.rollup()` once in O(N) reverse array order.
   - Extract top N directories into `top_dirs: Vec<(PathBuf, u64)>` by formatting `PathBuf` ONLY for selected top-N nodes.
   - Resolve top N files into `top_files: Vec<(u64, PathBuf)>`.
   - Store `arenas: vec![master_arena]` and build `all_dirs` if requested.

**Verify**: `cargo test -p dscan-core --lib` → all pass.

---

### Step 7: Create `crates/dscan-core/src/scanner/mod.rs` & Remove Monolithic `scanner.rs`
1. Create `crates/dscan-core/src/scanner/mod.rs` exporting:
   - `pub mod buffer;`
   - `pub mod state;`
   - `pub mod rollup;`
   - `#[cfg(unix)] pub mod linux;`
   - `#[cfg(windows)] pub mod windows;`
   - Public types: `ScanOptions`, `ScanResult`, `CliOptions`, `ProgressCallback`, `run_scan`, `run_scan_with_progress`, `normalize_scan_path`, `get_path_dev`, `is_excluded_dir`, `extract_file_extension`, `record_file_stat`, `record_file_ext`.
2. Delete monolithic `crates/dscan-core/src/scanner.rs`.
3. Update `crates/dscan-core/src/lib.rs` and verify all public re-exports.

**Verify**: `cargo test --all-targets` → all pass.

---

### Step 8: Fix JNI Pointer Lifecycle & Add Primitive Polling in `dscan-android`
1. In `crates/dscan-android/src/lib.rs`:
   - Replace `Box::into_raw(Box::new(session))` in `startScan` with `Arc::into_raw(session) as jlong`.
   - In `pollProgress`, `getTreemapNodes`, `getExtensionBreakdown`, `cancelScan`, dereference as `&*(session_ptr as *const ScanSession)`.
   - In `stopScan`, drop via `unsafe { let _ = Arc::from_raw(session_ptr as *const ScanSession); }`.
   - Update `pollProgress` to return `jlongArray` with metrics `[total_bytes, total_files, active_workers as u64, elapsed_millis, (files_per_sec * 100.0) as u64, (bytes_per_sec * 100.0) as u64, is_complete as u64]`.
2. In `android/app/src/main/kotlin/com/dscan/app/DscanBridge.kt`:
   - Update `pollProgress(sessionPtr: Long): LongArray?`.
   - Unpack primitive array into `ScanProgress` without JSON parsing.

**Verify**: `cargo test -p dscan-android` → exit 0.

---

## Test plan
- Unit tests:
  - `test_arena_merge_subtree_and_rollup` in `crates/dscan-core/src/arena.rs` (graft multi-worker arenas, verify rollup equivalence with single-threaded build).
  - `test_aligned_buffer_alloc_and_slice` in `crates/dscan-core/src/scanner/buffer.rs`.
  - `test_flat_subdir_buf_append_and_clear` in `crates/dscan-core/src/scanner/buffer.rs`.
- Integration tests:
  - `cargo test --test scanner_integration` (12 tests including `test_synthetic_tree_matches_du`, `test_synthetic_tree_rollup_cross_platform`, `test_multi_threaded_scalability`).
  - `cargo test --test arena_tests` (3 tests verifying tree rollup and zero-allocation top files).
  - `cargo test -p dscan-cli` (17 tests verifying JSON serialization and CLI formatting).

## Done criteria
- [ ] `cargo check --workspace --all-targets` exits 0.
- [ ] `cargo test --workspace` exits 0 with all unit and integration tests passing.
- [ ] Monolithic `crates/dscan-core/src/scanner.rs` removed and replaced with `crates/dscan-core/src/scanner/` submodules.
- [ ] `grep -rn "IoUringBatcher::new" crates/dscan-core/src/scanner/` returns 0 matches.
- [ ] `grep -rn "dir_map" crates/dscan-core/src/scanner/` returns 0 matches.
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0.
- [ ] `cargo fmt --check` exits 0.

## STOP conditions
- Subtree parent path resolution in master arena causes orphan nodes under cyclic symlinks (symlinks must not be followed as directories unless `follow_symlinks` is true).
- Android JNI `Arc::from_raw` double-free if `stopScan` called more than once (guard pointer with atomic or zeroing on Kotlin side).
- Compilation failure on Windows targets due to path separator assumptions (use `cfg!(windows)` path separator handling in `DirArena::reconstruct_path`).

## Maintenance notes
- `master_arena` now stores the single source of truth for the entire directory hierarchy; snapshots and treemap visualizers can traverse it directly via index rather than reconstructing intermediate `PathBuf` trees.
- `crates/dscan-core/src/sys/uring.rs` remains available for future batch I/O operations (e.g. parallel file hashing or deep metadata extraction) without burdening directory traversal.

---

### Critical Files for Implementation
- `/home/ahoura/dscan/crates/dscan-core/src/arena.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/scanner/rollup.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/scanner/linux.rs`
- `/home/ahoura/dscan/crates/dscan-core/src/scanner/mod.rs`
- `/home/ahoura/dscan/crates/dscan-android/src/lib.rs`
