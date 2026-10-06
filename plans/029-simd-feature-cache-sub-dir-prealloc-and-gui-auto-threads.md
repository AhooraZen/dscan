# Plan 029: SIMD Feature Cache, Hot-Path Exclusion Fast-Bypass, Cache-Optimized Directory Buffers, and GUI Auto-Threads

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0330385..HEAD -- crates/dscan-core/src/simd.rs crates/dscan-core/src/scanner.rs crates/dscan-core/src/sys/linux.rs crates/dscan-gui/src/app.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Status**: DONE
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (purely algorithmic CPU instruction and memory bandwidth reduction; zero behavioral breaking changes)
- **Depends on**: plans/028-device-aware-adaptive-rotational-thread-scaling.md
- **Category**: performance
- **Planned at**: commit `faa2735`, 2026-10-06
- **Issue**: 

## Why this matters
Across massive directory trees (hundreds of thousands of files), several micro-bottlenecks accumulate substantial CPU overhead:
1. **Redundant AVX2 Runtime Detection in Hot Dirent Loop**:
   In `crates/dscan-core/src/simd.rs`, `find_nul` executed `is_x86_feature_detected!("avx2")` on every single dirent name. With 250,000 files, this incurred 250,000 redundant CPU feature checks. Caching the result in an `AtomicU8` eliminates this call overhead entirely in hot loops, and SSE2 is already part of the baseline x86_64 target.
2. **Name-Exclusion Path-Stack Bypass**:
   In `crates/dscan-core/src/scanner.rs`, the scanner previously pushed every entry's name to `path_stack`, ran `matcher.is_excluded`, and truncated `path_stack` back. Adding `is_name_excluded` allows checking relative exclusion rules (e.g. `.git`, `node_modules`, `target`, `.cache`) directly on the dirent name slice, avoiding string and slice mutations for excluded directories.
3. **L2 Cache-Fitting Directory Buffers (128 KiB vs 512 KiB)**:
   A 512 KiB double-buffer consumes 1 MiB per worker thread (32 MiB across 32 threads), blowing past L1/L2 caches and causing high L3 cache pressure. Scaling to 128 KiB (page-aligned) keeps buffers resident in L2 CPU cache while still accommodating thousands of dirents per syscall.
4. **Adaptive Retry on EMFILE in `open_dir_at2`**:
   `open_dir_at2` now yields and retries once on `EMFILE`/`ENFILE`, preventing fallback to slower full-path directory resolution during temporary file descriptor spikes.
5. **GUI Device-Aware Adaptive Threading**:
   In `crates/dscan-gui/src/app.rs`, `start_scan` was hardcoded to `(cores * 4).clamp(8, 64)`. Adapting to `dscan_core::ScanOptions::auto_threads_for_path` prevents disk thrashing on rotational HDD volumes.

## Current state
- `crates/dscan-core/src/simd.rs:104-129`: Calls `is_x86_feature_detected!("avx2")` directly on every invocation.
- `crates/dscan-core/src/scanner.rs:705-720`: Mutates `path_stack` before checking name exclusions.
- `crates/dscan-gui/src/app.rs:46-59`: Hardcodes 4x core scaling.

## Implementation steps
1. Update `crates/dscan-core/src/simd.rs` with `has_avx2()` caching and `is_name_excluded()`.
2. Update `crates/dscan-core/src/scanner.rs` to fast-path name exclusions before path stack mutations, pre-allocate `sub_dirs` with capacity 16, and size `DualBuffer` to 128 KiB.
3. Update `crates/dscan-core/src/sys/linux.rs` to add `EMFILE`/`ENFILE` yield retry in `open_dir_at2`.
4. Update `crates/dscan-gui/src/app.rs` to use `ScanOptions::auto_threads_for_path`.
5. Run tests, clippy, and fmt.
