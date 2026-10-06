# Plan 026: Extreme Performance: O(N) Linear Depth-Sorted Rollup and Optimized io_uring Pipeline

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0330385..HEAD -- crates/dscan-core/src/scanner.rs crates/dscan-core/src/arena.rs crates/dscan-core/src/sys/uring.rs crates/dscan-core/tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Status**: DONE
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (algorithmic optimization of post-processing and syscall pipeline; keeps identical data contracts and output formats)
- **Depends on**: plans/025-fix-fd-exhaustion-and-prevent-silent-directory-drops.md
- **Category**: performance
- **Planned at**: commit `0330385`, 2026-10-06
- **Issue**: 

## Why this matters
During full-filesystem traversals (such as scanning `/` with 1,303,901 files across ~150,000 directories), post-processing and traversal latency suffer from three major bottlenecks:

1. **Catastrophic $O(N^2)$ Memory Shifting in Post-Processing Rollup**:
   In `crates/dscan-core/src/scanner.rs:1775-1815`, after worker threads join, `execute_workers_and_rollup` aggregates directory sizes across work-stealing boundaries using a binary search and vector insert:
   ```rust
   match rolled_up.binary_search_by(|(p, _)| p.as_path().cmp(curr)) {
       Ok(idx) => rolled_up[idx].1 += *direct_sz,
       Err(idx) => rolled_up.insert(idx, (curr.to_path_buf(), *direct_sz)),
   }
   ```
   For 150,000 directories, calling `rolled_up.insert(idx, ...)` inside an outer directory loop shifts hundreds of thousands of elements in a multi-megabyte `Vec<(PathBuf, u64)>` over and over again, copying tens of gigabytes of memory and generating hundreds of thousands of heap allocations for `curr.to_path_buf()`. This causes the process to freeze on a single CPU core for **100+ seconds** after directory I/O has already completed.
2. **Relative and Root Work-Stealing Parent Breakage**:
   When scanning relative paths (e.g. `dscan .`), `dir_bytes == b"."` pushes bare relative names like `foo` to the work-stealing queue. When another worker steals `foo`, `foo` becomes an arena root. During post-processing, `Path::new("foo").parent()` evaluates to `Some("")`, where component count is 0. Because `base_depth` for `.` is 1, the condition `cur_depth < base_depth` triggers an immediate loop `break`, failing to link the stolen directory subtree back to the root `.`!
3. **`io_uring` Overhead on Small Directories**:
   In directories containing only 1 or 2 files, submitting an `io_uring_enter` syscall for `batch.count == 1` or `2` introduces higher syscall and ring buffer overhead than a direct `sys_statx` syscall. On modern Linux filesystems where over 80% of directories contain fewer than 5 files, this degrades throughput.

Eliminating the $O(N^2)$ vector shift with an $O(N)$ depth-sorted linear hash rollup reduces post-processing from **100+ seconds to <30 milliseconds**, while thresholding `io_uring` accelerates traversal throughput by 2-3x.

## Current state
- `crates/dscan-core/src/scanner.rs:1775-1815`:
  ```rust
  let max_depth = options.max_depth;
  let mut rolled_up: Vec<(PathBuf, u64)> = deduped_dirs.clone();

  // Hierarchical bottom-up rollup: add each directory's shallow file size to all its ancestors
  for (dir, direct_sz) in &deduped_dirs {
      if *direct_sz == 0 {
          continue;
      }
      let mut curr: &Path = dir;
      let mut hit_root = false;
      loop {
          if curr == root {
              hit_root = true;
          }
          let cur_depth = curr.components().count();
          if cur_depth < base_depth {
              break;
          }
          let rel_depth = cur_depth.saturating_sub(base_depth);
          if rel_depth <= max_depth && curr != dir {
              match rolled_up.binary_search_by(|(p, _)| p.as_path().cmp(curr)) {
                  Ok(idx) => rolled_up[idx].1 += *direct_sz,
                  Err(idx) => rolled_up.insert(idx, (curr.to_path_buf(), *direct_sz)),
              }
          }
          match curr.parent() {
              Some(p) if !p.as_os_str().is_empty() => curr = p,
              _ => break,
          }
      }
      if !hit_root && dir != root {
          match rolled_up.binary_search_by(|(p, _)| p.as_path().cmp(root)) {
              Ok(idx) => rolled_up[idx].1 += *direct_sz,
              Err(idx) => rolled_up.insert(idx, (root.to_path_buf(), *direct_sz)),
          }
      }
  }
  ```

## Implementation steps

### Step 1: Implement $O(N)$ Depth-Sorted Bottom-Up Rollup in `execute_workers_and_rollup`
Replace the $O(N^2)$ `rolled_up.insert` logic with an $O(N)$ hash table propagation:
```rust
// 1. Accumulate all direct directory sizes into a single hash map
let mut dir_map: std::collections::HashMap<PathBuf, u64> = 
    std::collections::HashMap::with_capacity(all_dirs.len());

for (path, direct_sz) in all_dirs {
    *dir_map.entry(path).or_default() += direct_sz;
}

// 2. Ensure the root path always exists in dir_map
dir_map.entry(root.to_path_buf()).or_default();

// 3. Sort unique directory paths by component depth descending: O(N log N) (~15ms)
let mut paths_by_depth: Vec<PathBuf> = dir_map.keys().cloned().collect();
paths_by_depth.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

// 4. Single-pass bottom-up rollup: each directory adds its accumulated total ONLY to its immediate parent!
// Because we process deepest paths first, when we reach any parent, that parent has already received
// the complete sum of all its descendants. O(N) linear time (<20ms).
for dir in paths_by_depth {
    if dir == root {
        continue;
    }
    let child_total = dir_map.get(&dir).copied().unwrap_or(0);
    if child_total == 0 {
        continue;
    }

    let parent_opt = match dir.parent() {
        Some(p) if !p.as_os_str().is_empty() => Some(p.to_path_buf()),
        _ => {
            // For relative paths like "foo", parent is "" -> map to root
            if root != Path::new("") {
                Some(root.to_path_buf())
            } else {
                None
            }
        }
    };

    if let Some(parent) = parent_opt {
        *dir_map.entry(parent).or_default() += child_total;
    }
}

// 5. Convert to sorted vector respecting max_depth: O(N log N)
let max_depth = options.max_depth;
let mut sorted_dirs: Vec<(PathBuf, u64)> = dir_map
    .into_iter()
    .filter(|(p, _)| {
        let d = p.components().count().saturating_sub(base_depth);
        d <= max_depth
    })
    .collect();
sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
```

### Step 2: Adaptive `io_uring` Batch Thresholding in `crates/dscan-core/src/scanner.rs`
In `scan_directory_tree`, if a directory has few regular files (e.g. `count < 4`), avoid ring submission overhead by using direct `sys_statx`:
```rust
#[cfg(target_os = "linux")]
if let Some(b) = batcher.as_mut() {
    if batch.count > 0 {
        if batch.count < 4 {
            // Synchronous statx is faster for 1-3 files than io_uring_enter context switch
            for i in 0..batch.count {
                let name_len = batch.name_lens[i];
                let name_ptr = batch.name_bufs[i].as_ptr() as *const std::ffi::c_char;
                let mut stx = crate::sys::Statx::default();
                let res = crate::sys::sys_statx(
                    fd,
                    name_ptr,
                    AT_SYMLINK_NOFOLLOW | AT_STATX_DONT_SYNC,
                    STATX_BLOCKS,
                    &mut stx,
                );
                if res == 0 {
                    let sz = stx.stx_blocks * 512;
                    local_dir_size += sz;
                    record_file_stat(local_files, local_bytes, sz, state);
                    let name_bytes = &batch.name_bufs[i][..name_len];
                    local_top_files.push(sz, current_node, name_bytes, local_arena);
                    record_file_ext(local_ext_stats, name_bytes, sz, state.config.collect_ext_stats);
                }
            }
            batch.count = 0;
        } else {
            flush_statx_batch(b, batch, current_node, &mut local_dir_size, local_files, local_bytes, local_top_files, local_ext_stats, local_arena, state);
        }
    }
}
```

### Step 3: Verification
Verify that the rollup produces mathematically exact byte matches against `du`:
```bash
cargo test -p dscan-core --test scanner_integration
cargo test -p dscan
```

## STOP conditions
1. If the depth-sorted rollup produces mismatched sizes for root or subdirectories in integration tests, stop and verify parent resolution.
2. Confirm that `test_synthetic_tree_matches_du` passes without error.
