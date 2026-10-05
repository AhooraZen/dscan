# Plan 009: Optimize fstatat Directory FD Traversal
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
- **Depends on**: plans/001-test-harness-and-verification-baseline.md, plans/006-fix-getdents64-alignment-and-buffer-safety.md
- **Category**: perf
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/scanner.rs:152`, while `open_dir` holds an open directory file descriptor `fd`, child files call `item_path.symlink_metadata()`. This passes a full path string (e.g. `/home/user/deep/dir/file.txt`) to the kernel, forcing the VFS to traverse and resolve every parent dentry from the root on every single file. On a scan of 500,000 files, this generates millions of redundant directory lookups. Calling `fstatat(fd, name, &mut stat, AT_SYMLINK_NOFOLLOW)` resolves the file relative to the open directory descriptor in O(1) time.

## Current state
In `src/scanner.rs:151-160`:
```rust
                } else if d_type == DT_REG || d_type == DT_UNKNOWN {
                    if let Ok(meta) = item_path.symlink_metadata() {
                        if meta.is_dir() {
                            sub_dirs.push(item_path);
                        } else {
                            *files_cnt += 1;
                            if meta.dev() == root_dev {
                                let sz = meta.blocks() * 512;
                                total_dir_size += sz;
```
`fd` is open and in scope (`if let Some(fd) = open_dir(dir_path)` at line 94).

## Solution
1. In `src/sys.rs`, bind POSIX `fstatat` and define `LinuxStat` struct layout (or use `AT_FDCWD` / `AT_SYMLINK_NOFOLLOW = 0x100`).
2. In `scan_directory_tree`, call `fstatat(fd, name_ptr, &mut stat, AT_SYMLINK_NOFOLLOW)` to inspect child entries without resolving full paths from root.
3. Fall back to `symlink_metadata()` if `fstatat` is unavailable or errors.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/sys.rs` (define `fstatat` extern, `AT_SYMLINK_NOFOLLOW`, and stat struct)
- `src/scanner.rs` (use `fstatat` in `getdents64` loop)

**Out of scope**:
- Work queue synchronization, UI rendering

## Git workflow
- Branch: `advisor/009-optimize-fstatat`
- Commit message: `perf(scanner): use fstatat with open directory descriptor to eliminate VFS path walks`

## Steps

### Step 1: Add `fstatat` binding and constants to `src/sys.rs`
In `src/sys.rs`:
```rust
pub const AT_SYMLINK_NOFOLLOW: i32 = 0x100;

#[repr(C)]
#[derive(Default)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: i32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

pub const S_IFMT: u32 = 0o170000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;

unsafe extern "C" {
    pub fn fstatat(dirfd: i32, pathname: *const std::ffi::c_char, statbuf: *mut Stat, flags: i32) -> i32;
}
```

### Step 2: Use `fstatat` in `scan_directory_tree` in `src/scanner.rs`
When `d_type == DT_REG` or `DT_UNKNOWN`:
Instead of invoking `item_path.symlink_metadata()`:
```rust
let mut st = crate::sys::Stat::default();
let res = unsafe { crate::sys::fstatat(fd, name_ptr, &mut st, crate::sys::AT_SYMLINK_NOFOLLOW) };
if res == 0 {
    let is_dir = (st.st_mode & crate::sys::S_IFMT) == crate::sys::S_IFDIR;
    if is_dir {
        sub_dirs.push(item_path);
    } else {
        *files_cnt += 1;
        if st.st_dev == root_dev {
            let sz = (st.st_blocks as u64) * 512;
            total_dir_size += sz;
            
            if local_top_files.len() < top_limit {
                local_top_files.push(Reverse((sz, item_path)));
            } else if let Some(Reverse((min_sz, _))) = local_top_files.peek() {
                if sz > *min_sz {
                    local_top_files.pop();
                    local_top_files.push(Reverse((sz, item_path)));
                }
            }
        }
    }
}
```

### Step 3: Verify
Run `cargo test` and verify synthetic directory scans produce identical file counts and sizes.

**Verify**: `cargo test` → all pass.

## Test plan
- Run integration tests comparing results with standard metadata.
- Verification: `cargo test`

## Done criteria
- [ ] Child file metadata queries use `fstatat(fd, ...)`
- [ ] Device check and block allocation math remain exact
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 009 updated to DONE

## STOP conditions
- If `fstatat` returns -1 on valid files. Fallback to `symlink_metadata()` on error.

## Maintenance notes
- `AT_SYMLINK_NOFOLLOW` ensures symbolic links are not traversed into foreign directories or infinite cycles.
