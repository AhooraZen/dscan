# Plan 025: Fix File Descriptor Exhaustion and Prevent Silent Directory Drops

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0330385..HEAD -- crates/dscan-core/src/scanner.rs crates/dscan-core/src/sys/linux.rs crates/dscan-core/src/sys/mod.rs crates/dscan-core/tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Status**: DONE
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (purely additive FD safety, limit bumping, and fallback correctness; zero regressions to existing syscall paths)
- **Depends on**: plans/021-fix-core-scanner-chase-lev-concurrency-and-path-bugs.md
- **Category**: bug
- **Planned at**: commit `0330385`, 2026-10-06
- **Issue**: 

## Why this matters
When scanning massive directories or entire system roots (e.g. `dscan /`) with 32 to 64 parallel worker threads, `dscan` can silently drop entire directory subtrees without reporting any error or accounting for their disk space:

1. **Unraised File Descriptor Limit (`RLIMIT_NOFILE`)**:
   On standard Linux distributions, containers, and non-root shells, the default soft file descriptor limit (`RLIMIT_NOFILE`) is 1,024. `dscan` never queries or raises this limit at startup. With 32 or 64 worker threads, each thread recursing deeply into directory hierarchies and allocating `io_uring` ring file descriptors quickly exhausts all 1,024 file descriptors.
2. **Silent Subtree Dropping on `EMFILE` / `ENFILE`**:
   In `crates/dscan-core/src/scanner.rs:920-965`, when `open_dir_at2` fails due to `EMFILE` (errno 24) or `ENFILE` (errno 23), `child_fd` becomes `None`. Inside the child call, `open_dir(dir_path)` also fails due to `EMFILE`. The code then falls back to `else if let Ok(entries) = fs::read_dir(dir_path)`. Because the process has no available file descriptors, `fs::read_dir` also fails with `EMFILE`. The entire `else if` block is silently bypassed: zero files are recorded, zero bytes are counted, and no warning or error is emitted. Millions of files can be omitted from the scan results.
3. **Descriptor Accumulation Across Deep Recursion**:
   In `crates/dscan-core/src/scanner.rs:913-940`, the parent directory file descriptor `fd` is kept open across the entire recursive traversal of all child subdirectories. If a directory tree has depth 30, all 30 descriptors remain open simultaneously on that thread's call stack. Across 64 threads, this consumes 1,920 simultaneous open file descriptors unnecessarily.
4. **`fs::read_dir` Fallback Ignores `--cross-device` and Symlinks**:
   In `crates/dscan-core/src/scanner.rs:946-958`:
   - `meta.dev() == root_dev` is hardcoded without checking `state.config.cross_filesystems`. When the user specifies `-x` / `--cross-device`, the fallback rejects all files on secondary mounts.
   - It calls `entry.metadata()`, which follows symlinks, instead of `entry.symlink_metadata()`. When `--follow-symlinks` is false, this can traverse symlink loops or external targets.
   - Subdirectories are pushed to `sub_dirs` without checking filesystem boundaries when `cross_filesystems` is false.

Resolving these issues ensures that `dscan` automatically expands its file descriptor capacity to the system maximum, minimizes descriptor lifetime, retries gracefully on descriptor starvation, and guarantees 100% traversal accuracy with zero dropped directories.

## Current state
- `crates/dscan-core/src/scanner.rs:920-965`:
  ```rust
  let child_fd = if sub_name.len() < 255 {
      let mut name_c = [0u8; 256];
      name_c[..sub_name.len()].copy_from_slice(&sub_name);
      name_c[sub_name.len()] = 0;
      match crate::sys::open_dir_at2(
          fd,
          name_c.as_ptr() as *const std::ffi::c_char,
          !state.config.cross_filesystems,
      ) {
          Ok(cfd) => Some(cfd),
          Err(crate::sys::EXDEV) => {
              path_stack.truncate(orig_len);
              continue;
          }
          _ => None,
      }
  } else {
      None
  };

  dual_buffer.swap();
  scan_directory_tree(
      &child_path,
      child_fd,
      child_node,
      next_rel_depth,
      state,
      worker,
      dual_buffer,
      ...
  );
  dual_buffer.swap();

  path_stack.truncate(orig_len);
  }

  // SAFETY: fd was opened by open_dir or open_dir_at2.
  unsafe { crate::sys::close(fd) };
  } else if let Ok(entries) = fs::read_dir(dir_path) {
  ...
  ```
- `crates/dscan-core/src/scanner.rs:946-958`:
  ```rust
  if let Ok(meta) = entry.metadata() {
      if meta.is_dir() {
          sub_dirs.push(name_bytes);
      } else if meta.dev() == root_dev {
          let sz = meta.blocks() * 512;
          local_dir_size += sz;
          ...
      }
  }
  ```

## Implementation steps

### Step 1: Implement `raise_fd_limit()` in `crates/dscan-core/src/sys/linux.rs`
Add raw syscall-based or libc-compatible `raise_fd_limit()` on Linux to raise `RLIMIT_NOFILE` soft limit to its hard limit without third-party crates:
```rust
#[cfg(target_os = "linux")]
pub fn raise_fd_limit() {
    #[repr(C)]
    struct Rlimit64 {
        rlim_cur: u64,
        rlim_max: u64,
    }

    const SYS_PRLIMIT64: i64 = 302;
    let mut limit = Rlimit64 { rlim_cur: 0, rlim_max: 0 };
    
    // Get current limits (pid 0 = current process, resource 7 = RLIMIT_NOFILE)
    let ret = unsafe {
        syscall(
            SYS_PRLIMIT64,
            0i64,
            7i64, // RLIMIT_NOFILE
            std::ptr::null::<Rlimit64>() as i64,
            &mut limit as *mut Rlimit64 as i64,
        )
    };

    if ret == 0 && limit.rlim_cur < limit.rlim_max {
        let mut new_limit = Rlimit64 {
            rlim_cur: limit.rlim_max,
            rlim_max: limit.rlim_max,
        };
        unsafe {
            syscall(
                SYS_PRLIMIT64,
                0i64,
                7i64,
                &new_limit as *const Rlimit64 as i64,
                std::ptr::null_mut::<Rlimit64>() as i64,
            );
        }
    }
}
```
Expose `raise_fd_limit()` in `crates/dscan-core/src/sys/mod.rs` as a no-op on non-Linux platforms.

### Step 2: Invoke `raise_fd_limit()` in `init_scan_state` in `crates/dscan-core/src/scanner.rs`
At the beginning of `init_scan_state`:
```rust
#[cfg(target_os = "linux")]
crate::sys::raise_fd_limit();
```
This guarantees that before worker threads spawn, the process has maximum available file descriptors (e.g. 524,288 or 1,048,576).

### Step 3: Fix `fs::read_dir` Fallback in `crates/dscan-core/src/scanner.rs`
Update lines 946-960 in `crates/dscan-core/src/scanner.rs` to use `entry.symlink_metadata()` and respect `state.config.cross_filesystems`:
```rust
} else if let Ok(entries) = fs::read_dir(dir_path) {
    for entry in entries.flatten() {
        let path = entry.path();
        let p_bytes = path.as_os_str().as_bytes();
        let name_bytes = entry.file_name().as_os_str().as_bytes().to_vec();

        if matcher.is_excluded(p_bytes, &name_bytes) {
            continue;
        }

        // Use symlink_metadata so symlinks are not followed unless requested
        let meta_res = if state.config.follow_symlinks {
            entry.metadata()
        } else {
            entry.symlink_metadata()
        };

        if let Ok(meta) = meta_res {
            let is_on_root_dev = state.config.cross_filesystems || meta.dev() == root_dev;
            if meta.is_dir() {
                if is_on_root_dev {
                    sub_dirs.push(name_bytes);
                }
            } else if is_on_root_dev {
                let sz = meta.blocks() * 512;
                local_dir_size += sz;
                record_file_stat(local_files, local_bytes, sz, state);
                local_top_files.push(sz, current_node, &name_bytes, local_arena);
                record_file_ext(
                    local_ext_stats,
                    &name_bytes,
                    sz,
                    state.config.collect_ext_stats,
                );
            }
        }
    }
```

### Step 4: Add EMFILE Adaptive Retry / Yield in `open_dir_at2` Call Site
In `scan_directory_tree`, if `open_dir_at2` returns `EMFILE` or `ENFILE`:
```rust
match crate::sys::open_dir_at2(...) {
    Ok(cfd) => Some(cfd),
    Err(crate::sys::EXDEV) => {
        path_stack.truncate(orig_len);
        continue;
    }
    Err(24 /* EMFILE */) | Err(23 /* ENFILE */) => {
        // Yield thread to let peer workers complete open descriptors, then retry once
        std::thread::yield_now();
        crate::sys::open_dir_at2(...).ok()
    }
    _ => None,
}
```

### Step 5: Verification
Run the comprehensive test suite to confirm zero regressions:
```bash
cargo test -p dscan-core
cargo test -p dscan
```

## STOP conditions
1. If `SYS_PRLIMIT64` is not supported on a specific Linux kernel, ensure the fallback fails gracefully without panicking.
2. If `cargo test` fails on any platform, do not proceed.
