# Plan 002: Fix Work-Stealing Recursive Size Rollup
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: Compare the "Current state" excerpts against
> the live code before proceeding; on a mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: plans/001-test-harness-and-verification-baseline.md
- **Category**: bug
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
`dscan` advertises "Accurate Recursive Size Rollup: Accurately sums directory subtree sizes similar to Filelight/GNU du". However, when dynamic work-stealing splits subdirectories (`sub_dirs.split_off(...)` at `src/scanner.rs:212`) and pushes them to the global work queue, those subdirectories are processed as independent root tasks by other worker threads. Their subtree size is added to the global byte counter, but is NEVER credited to their parent or ancestor directories. As a result, parent directories display smaller sizes than their own child subdirectories, or are excluded from the "Top Directories By Recursive Size" table entirely.

## Current state
In `src/scanner.rs:210-238`:
```rust
    // Dynamic Work-Stealing at ANY depth when workers are idle
    if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < threads {
        let mut q = state.queue_mutex.lock().unwrap();
        let half = sub_dirs.split_off(sub_dirs.len() / 2);
        for d in half {
            q.tasks.push(d);
        }
        state.queue_cvar.notify_all();
    }

    for sub in sub_dirs {
        let child_size = scan_directory_tree(
            &sub,
            state,
            buffer,
            local_dirs,
            local_top_files,
            files_cnt,
        );
        total_dir_size += child_size;
    }
```
And in `worker_loop` (`src/scanner.rs:284-295`):
```rust
    let tree_sz = scan_directory_tree(
        &current_dir,
        &state,
        &mut heap_buffer,
        &mut local_dirs,
        &mut local_top_files,
        &mut local_files_cnt,
    );

    state.total_bytes.fetch_add(tree_sz, Ordering::Relaxed);
```
When `half` is stolen, the current thread never receives the size of `half`, and the thread that pops `current_dir` from `tasks` does not know `current_dir`'s ancestors.

## Solution Architecture: Prefix Rollup or Bottom-Up Path Aggregation
Instead of relying on return-value summation across thread boundaries:
Whenever a file of size `sz` is discovered under directory `D`:
The file's size `sz` belongs to all parent directories of `D` up to `max_depth` (relative to `base_depth`).
In thread-local aggregation (`local_dirs`):
For any discovered directory `dir` whose relative depth <= `max_depth`, or for every file discovered, aggregate its size into its ancestor paths that fall within `base_depth..=base_depth + max_depth`.
Alternatively: Each thread records directory sizes. During final reduction in `run_scan`, perform a hierarchical bottom-up rollup on `final_dirs`:
Sort all paths by descending component depth, and for each path `P`, add its recursive size to `P.parent()`.
This bottom-up rollup guarantees that `parent_size >= sum(children_sizes)` regardless of which thread stole which task.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |
| Run integration test | `cargo test --test scanner_integration` | exit 0, all pass |

## Scope
**In scope**:
- `src/scanner.rs` (`scan_directory_tree`, `worker_loop`, `run_scan`)
- `tests/scanner_integration.rs` (add test case verifying parent size >= child size under multi-threading)

**Out of scope**:
- `src/ui.rs`, `src/cli.rs`, `src/sys.rs`

## Git workflow
- Branch: `advisor/002-fix-work-stealing-rollup`
- Commit message: `fix(scanner): ensure accurate recursive directory size rollup across work-stealing boundaries`

## Steps

### Step 1: Add a test reproducing the size discrepancy
In `tests/scanner_integration.rs`:
Create a nested directory structure:
`test_root/parent/child1/data1.bin` (5 MB)
`test_root/parent/child2/data2.bin` (5 MB)
Run scan with `threads = 8` and `max_depth = 3`.
Assert that `parent` size in `top_dirs` is at least 10 MB (and exactly matches `child1` + `child2`).
Verify that the test fails or exhibits size loss prior to the fix.

### Step 2: Implement hierarchical rollup in `run_scan`
In `src/scanner.rs`:
1. In `scan_directory_tree`: each directory tracks its immediate file content size (or subtrees), and records into `local_dirs[dir]`.
2. After `all_results` are merged into `final_dirs: HashMap<PathBuf, u64>` in `run_scan`:
   Ensure that every directory's recursive size includes all child directories:
   - Identify all distinct directories in `final_dirs`.
   - Propagate sizes upwards: For each directory `dir` in `final_dirs`, if `dir != root`, its size must roll up into its ancestor components that are `>= base_depth` and `<= base_depth + max_depth`.
   - Ensure deduplication so child files are not double counted. Specifically, having each directory track its direct shallow byte total, and then rolling up shallow totals to all ancestors up to `max_depth`, guarantees exact correctness:
     `recursive_size(P) = sum(shallow_size(C) for all C where C.starts_with(P))`.

**Verify**: `cargo test --test scanner_integration` passes.

## Test plan
- Verify synthetic hierarchy where subdirectories are distributed to multiple worker threads.
- Assert parent directory recursive size is strictly greater than or equal to any child directory.
- Verification: `cargo test`

## Done criteria
- [ ] Parent directory sizes accurately reflect all child subdirectories regardless of work-stealing splits
- [ ] `cargo test` exits 0 with no failures
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0
- [ ] `plans/README.md` status row for 002 updated to DONE

## STOP conditions
- If double-counting occurs (parent size > total scanned bytes). The sum of all top-level children must equal root subtree size.
- If performance drops significantly (>20% slowdown on benchmarks).

## Maintenance notes
- Retain the check `meta.dev() == root_dev` so external mount points are not traversed or aggregated into the origin filesystem total.
