# Plan 006: Fix Memory Safety and Alignment in getdents64 Parser
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
- **Category**: security
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/scanner.rs:113-119`, raw directory buffers from the Linux `getdents64` syscall are parsed with unsafe direct pointer casts and unbounded C-string reads:
1. `*dirent_ptr` directly dereferences an unaligned 8-byte aligned struct (`LinuxDirent64`), which causes undefined behavior in Rust and potential `SIGBUS` crashes on strict-alignment architectures (ARM, RISC-V).
2. `reclen` is not bounds-checked: a corrupted or zero `reclen` causes an infinite loop or out-of-bounds heap memory access.
3. `CStr::from_ptr(name_ptr)` scans forward indefinitely until it finds a `\0` byte with no upper bound check against the buffer or record size, risking heap memory over-reads.

## Current state
In `src/scanner.rs:112-122`:
```rust
            while pos < nread {
                let dirent_ptr = unsafe { buffer.as_ptr().add(pos) as *const LinuxDirent64 };
                let dirent = unsafe { *dirent_ptr };
                let reclen = dirent.d_reclen as usize;

                let name_ptr = unsafe { buffer.as_ptr().add(pos + 19) as *const std::ffi::c_char };
                let name_cstr = unsafe { CStr::from_ptr(name_ptr) };
                let name_bytes = name_cstr.to_bytes();

                pos += reclen;
```

## Solution
1. Use `std::ptr::read_unaligned` to safely read `LinuxDirent64` regardless of byte alignment.
2. Validate record length before advancing: `if reclen < 19 || pos + reclen > nread { break; }`.
3. Slice the record's name window `&buffer[pos + 19..pos + reclen]` and use `CStr::from_bytes_until_nul` to ensure string parsing never reads beyond the current directory entry.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/scanner.rs` (buffer parsing loop inside `scan_directory_tree`)

**Out of scope**:
- `src/sys.rs` (struct definition stays unchanged)
- Argument parsing, UI formatting

## Git workflow
- Branch: `advisor/006-fix-getdents-safety`
- Commit message: `fix(scanner): use unaligned reads and bounded slice parsing for getdents64 buffer`

## Steps

### Step 1: Update buffer parsing in `src/scanner.rs`
Replace lines 112-122 in `scan_directory_tree` with:
```rust
            while pos < nread {
                if pos + std::mem::size_of::<LinuxDirent64>() > nread {
                    break;
                }

                let dirent_ptr = unsafe { buffer.as_ptr().add(pos) as *const LinuxDirent64 };
                let dirent = unsafe { std::ptr::read_unaligned(dirent_ptr) };
                let reclen = dirent.d_reclen as usize;

                if reclen < 19 || pos + reclen > nread {
                    break;
                }

                let name_start = pos + 19;
                let name_record_slice = &buffer[name_start..pos + reclen];
                let name_cstr = match std::ffi::CStr::from_bytes_until_nul(name_record_slice) {
                    Ok(c) => c,
                    Err(_) => {
                        pos += reclen;
                        continue;
                    }
                };
                let name_bytes = name_cstr.to_bytes();

                pos += reclen;
```

### Step 2: Verify with tests and clippy
Run test suite and verify no regressions in directory traversal.

**Verify**: `cargo test` → all pass.

## Test plan
- Run integration tests under `tests/scanner_integration.rs`.
- Verify `getdents64` correctly parses normal files, hidden files, and empty directories.
- Verification: `cargo test`

## Done criteria
- [ ] No direct dereference of unaligned pointers in `src/scanner.rs`
- [ ] `reclen == 0` or out-of-bounds `reclen` safely breaks loop without crashing
- [ ] Filename parsing bounded by `pos + reclen`
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 006 updated to DONE

## STOP conditions
- If `CStr::from_bytes_until_nul` fails to parse valid Linux kernel dirent names.

## Maintenance notes
- Offset 19 is the exact byte offset of `d_name` in Linux `struct dirent64` (`u64 d_ino` [8] + `i64 d_off` [8] + `u16 d_reclen` [2] + `u8 d_type` [1] = 19 bytes).
