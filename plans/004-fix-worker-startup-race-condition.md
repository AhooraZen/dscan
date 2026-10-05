# Plan 004: Fix Worker Startup Race Condition
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
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/001-test-harness-and-verification-baseline.md
- **Category**: concurrency
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
At the start of `run_scan`, the global queue contains a single initial task (the root path), and `active_workers` is initialized to 0. When worker threads spawn, the first thread to acquire `queue_mutex` pops the root task and immediately releases the lock *before* incrementing `active_workers` (`src/scanner.rs:279`). The remaining worker threads (e.g. 31 of 32) then acquire the lock, observe an empty queue and `active_workers == 0`, conclude the entire scan is finished, and terminate immediately. This reduces multi-threaded scanning to a single thread.

## Current state
In `src/scanner.rs:250-280`:
```rust
        let task = {
            let mut q = state.queue_mutex.lock().unwrap();
            while q.tasks.is_empty() {
                if state.active_workers.load(Ordering::SeqCst) == 0 {
                    state.queue_cvar.notify_all();
                    return ThreadLocalResult {
                        dir_sizes: local_dirs,
                        top_files: local_top_files,
                    };
                }
                let (new_q, timeout_res) = state
                    .queue_cvar
                    .wait_timeout(q, Duration::from_millis(5))
                    .unwrap();
                q = new_q;
                if timeout_res.timed_out()
                    && q.tasks.is_empty()
                    && state.active_workers.load(Ordering::SeqCst) == 0
                {
                    state.queue_cvar.notify_all();
                    return ThreadLocalResult {
                        dir_sizes: local_dirs,
                        top_files: local_top_files,
                    };
                }
            }
            q.tasks.pop()
        };

        if let Some(current_dir) = task {
            state.active_workers.fetch_add(1, Ordering::SeqCst);
```

## Solution
Atomically transition task ownership inside the lock:
1. When a worker pops a task from `q.tasks`, increment `active_workers` *while holding `queue_mutex`*.
2. Only when `q.tasks.is_empty()` AND `active_workers == 0` can workers conclude that no further work will be generated.
3. This eliminates the race condition: while thread 0 is taking the root task, `active_workers` is already 1, preventing threads 1..31 from prematurely exiting.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/scanner.rs` (`worker_loop` synchronization)

**Out of scope**:
- UI spinner, argument parsing, file reading

## Git workflow
- Branch: `advisor/004-fix-startup-race`
- Commit message: `fix(scanner): increment active workers inside queue lock to prevent premature shutdown`

## Steps

### Step 1: Update task popping in `src/scanner.rs`
In `worker_loop`:
```rust
        let task = {
            let mut q = state.queue_mutex.lock().unwrap();
            loop {
                if let Some(dir) = q.tasks.pop() {
                    state.active_workers.fetch_add(1, Ordering::SeqCst);
                    break Some(dir);
                }
                
                if state.active_workers.load(Ordering::SeqCst) == 0 {
                    state.queue_cvar.notify_all();
                    return ThreadLocalResult {
                        dir_sizes: local_dirs,
                        top_files: local_top_files,
                    };
                }
                
                let (new_q, _) = state
                    .queue_cvar
                    .wait_timeout(q, Duration::from_millis(10))
                    .unwrap();
                q = new_q;
            }
        };
```
And remove the redundant `state.active_workers.fetch_add(1, Ordering::SeqCst);` at line 279 outside the lock.

### Step 2: Verify multi-thread worker participation
In `tests/scanner_integration.rs`:
Add a test asserting that when scanning a tree with multiple subdirectories, all spawned worker threads participate (e.g. verify `all_results.len() == threads` and multiple threads returned non-empty results).

**Verify**: `cargo test` → all tests pass.

## Test plan
- Run integration tests with 8, 16, and 32 threads.
- Confirm all threads remain active until the queue and active count are both exhausted.
- Verification: `cargo test`

## Done criteria
- [ ] `active_workers` is incremented inside `queue_mutex` when task is popped
- [ ] Worker threads do not exit while another thread is transitioning between queue and execution
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 004 updated to DONE

## STOP conditions
- If worker threads deadlock or fail to terminate at scan completion.
- If thread count mismatch occurs.

## Maintenance notes
- Condition variable notifications must wake waiting workers whenever new subdirectories are pushed to `q.tasks`.
