# Plan 007: Fallback for O_NOATIME on Unprivileged Scans
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
- **Depends on**: plans/001-test-harness-and-verification-baseline.md
- **Category**: security
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/sys.rs:44`, `open_dir` unconditionally passes the `O_NOATIME` flag when opening directories. The Linux kernel requires that the caller either owns the directory or holds `CAP_FOWNER` to use `O_NOATIME`; otherwise, the `open` syscall fails with `EPERM` (Operation not permitted). When an unprivileged user scans directories they do not own (such as `/var`, `/usr`, or multi-user home directories), `open_dir` returns `None`. This drops the scanner into the slower `std::fs::read_dir` fallback, negating the primary performance advantage of `dscan`.

## Current state
In `src/sys.rs:42-50`:
```rust
/// Open a directory with direct flags (O_DIRECTORY | O_CLOEXEC | O_NOATIME).
pub fn open_dir(path: &Path) -> Option<i32> {
    let path_c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let fd = unsafe { open(path_c.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOATIME) };
    if fd >= 0 {
        Some(fd)
    } else {
        None
    }
}
```

## Solution
If `open` with `O_NOATIME` fails, immediately retry `open` with `O_RDONLY | O_DIRECTORY | O_CLOEXEC` (omitting `O_NOATIME`). If the second call succeeds, return `Some(fd)`. Only return `None` if the directory cannot be opened at all (e.g. `EACCES` due to missing read permissions).

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/sys.rs` (`open_dir` implementation and unit tests)

**Out of scope**:
- `src/scanner.rs`, `src/ui.rs`, `src/cli.rs`

## Git workflow
- Branch: `advisor/007-o-noatime-fallback`
- Commit message: `fix(sys): retry open_dir without O_NOATIME when unprivileged`

## Steps

### Step 1: Update `open_dir` in `src/sys.rs`
Update `open_dir`:
```rust
pub fn open_dir(path: &Path) -> Option<i32> {
    let path_c = CString::new(path.as_os_str().as_bytes()).ok()?;
    
    // Attempt with O_NOATIME first for best performance when owned or root
    let fd = unsafe { open(path_c.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOATIME) };
    if fd >= 0 {
        return Some(fd);
    }
    
    // Fallback without O_NOATIME for directories not owned by current user (prevents EPERM)
    let fd = unsafe { open(path_c.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC) };
    if fd >= 0 {
        Some(fd)
    } else {
        None
    }
}
```

### Step 2: Add unit test in `src/sys.rs`
Add `#[cfg(test)] mod tests` in `src/sys.rs`:
Test:
1. `open_dir` on valid directory (`std::env::temp_dir()`) succeeds and returns `Some(fd >= 0)`.
2. Directory descriptor is closed cleanly with `crate::sys::close(fd)`.
3. `open_dir` on non-existent path returns `None`.

**Verify**: `cargo test sys::tests` → all pass.

## Test plan
- Verify `open_dir` opens valid directory and closes cleanly.
- Verify `open_dir` returns `None` on non-existent path.
- Verification: `cargo test`

## Done criteria
- [ ] `open_dir` retries without `O_NOATIME` on initial failure
- [ ] Unprivileged scans of foreign-owned directories use the fast `getdents64` path when read permissions exist
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 007 updated to DONE

## STOP conditions
- If retrying without `O_NOATIME` causes unexpected file descriptor leaks. Ensure returned `fd` is always closed by caller.

## Maintenance notes
- `O_NOATIME` is an optimization that avoids updating access timestamps on disk; omitting it when permissions forbid it has zero impact on scan correctness.
