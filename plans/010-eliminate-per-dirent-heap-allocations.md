# Plan 010: Eliminate Per-Dirent Heap Allocations
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
- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/001-test-harness-and-verification-baseline.md, plans/009-optimize-fstatat-directory-fd-traversal.md
- **Category**: perf
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/scanner.rs:129-148`, the scanner allocates a fresh `Vec<u8>` for `full_path_bytes` and an owned `PathBuf` for `item_path` for *every single entry* returned by `getdents64`. On a scan of 1,000,000 files, this triggers 2,000,000 heap allocations and deallocations across worker threads, creating heavy memory allocator lock contention. Because >99% of files are regular files that never make it into the top 25 files list, constructing full `PathBuf` objects upfront is almost entirely wasted work.

## Current state
In `src/scanner.rs:129-148`:
```rust
                let mut full_path_bytes =
                    Vec::with_capacity(dir_bytes.len() + 1 + name_bytes.len());
                full_path_bytes.extend_from_slice(dir_bytes);
                if !dir_bytes.ends_with(b"/") && dir_bytes != b"." {
                    full_path_bytes.push(b'/');
                } else if dir_bytes == b"." {
                    full_path_bytes.clear();
                }
                full_path_bytes.extend_from_slice(name_bytes);

                if is_excluded_dir(&full_path_bytes, name_bytes, excludes) {
                    continue;
                }

                let item_path = if full_path_bytes.is_empty() {
                    PathBuf::from(".")
                } else {
                    PathBuf::from(std::ffi::OsStr::from_bytes(&full_path_bytes))
                };
```

## Solution
1. Defer path construction: Check exclusions on `name_bytes` and directory context first.
2. With `fstatat` (Plan 009), child metadata is queried using `fd` and `name_ptr` without requiring full path strings.
3. Only construct `PathBuf` when:
   - The entry is confirmed to be a directory (`DT_DIR` or `S_ISDIR`) to push to `sub_dirs`.
   - The entry is a file whose size actually qualifies for `local_top_files`.
4. Use a reusable byte buffer when constructing directory paths.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/scanner.rs` (inner loop of `scan_directory_tree`)

**Out of scope**:
- `src/cli.rs`, `src/ui.rs`, `src/sys.rs`

## Git workflow
- Branch: `advisor/010-eliminate-allocations`
- Commit message: `perf(scanner): defer PathBuf allocation to surviving directories and top files`

## Steps

### Step 1: Defer PathBuf construction in `src/scanner.rs`
In `scan_directory_tree`:
1. When checking `is_excluded_dir`, pass parent directory bytes and `name_bytes`.
2. For regular files: query size via `fstatat`.
3. If `sz` does not qualify for `local_top_files` (i.e. `local_top_files.len() >= top_limit` and `sz <= min_sz`), do NOT construct `PathBuf` at all!
4. Only when `sz > min_sz` or `is_dir` is true:
   ```rust
   let make_path = || -> PathBuf {
       if dir_bytes == b"." {
           PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes))
       } else {
           let mut p = Vec::with_capacity(dir_bytes.len() + 1 + name_bytes.len());
           p.extend_from_slice(dir_bytes);
           if !dir_bytes.ends_with(b"/") {
               p.push(b'/');
           }
           p.extend_from_slice(name_bytes);
           PathBuf::from(std::ffi::OsStr::from_bytes(&p))
       }
   };
   ```
5. Pass `make_path()` only when pushing to `sub_dirs` or `local_top_files`.

### Step 2: Verify
Run integration tests. Verify that total scanned bytes, file counts, and top file lists match exactly.

**Verify**: `cargo test` → all pass.

## Test plan
- Verify top files list still contains correct full paths.
- Verify total byte count and file count remain identical.
- Verification: `cargo test`

## Done criteria
- [ ] No `PathBuf` or `Vec<u8>` allocation for files that do not qualify for `top_files`
- [ ] Traversal memory churn reduced
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 010 updated to DONE

## STOP conditions
- If deferred path construction produces relative paths with missing parent segments in `top_files`.

## Maintenance notes
- Deferring allocation is an idiomatic Rust zero-cost pattern: compute only when the result is retained.
