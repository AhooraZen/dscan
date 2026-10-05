# Plan 008: Fix Multi-Architecture ioctl and Syscall Numbers
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
- **Category**: architecture
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/ui.rs:31`, `get_terminal_width` invokes `crate::sys::syscall(16, 1, TIOCGWINSZ, ...)`. Syscall 16 is only `ioctl` on x86_64 Linux. On AArch64 (ARM64) and RISC-V 64, syscall 16 is `restart_syscall`, while `ioctl` is syscall 29. On ARM32, `ioctl` is syscall 54. On ARM64 Linux and Android Termux, this causes terminal width detection to fail and default to 80 columns. Furthermore, `src/sys.rs:14-15` falls back to `SYS_GETDENTS64 = 217` on all non-x86/arm targets, which fails on RISC-V and LoongArch where `getdents64` is syscall 61.

## Current state
In `src/ui.rs:31`:
```rust
    let ret = unsafe { crate::sys::syscall(16, 1, TIOCGWINSZ, &mut ws as *mut Winsize as i64) };
```
In `src/sys.rs:5-16`:
```rust
#[cfg(target_arch = "x86_64")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(target_arch = "aarch64")]
pub const SYS_GETDENTS64: i64 = 61;

#[cfg(target_arch = "arm")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm")))]
pub const SYS_GETDENTS64: i64 = 217;
```

## Solution
1. In `src/sys.rs`, bind standard POSIX/libc `ioctl` extern `pub fn ioctl(fd: i32, request: u64, ...) -> i32;` directly, or define architecture-specific `SYS_IOCTL`. Using the C library `ioctl` function is portable across all Linux CPU architectures (x86_64, aarch64, arm, riscv64, loongarch64).
2. Update `SYS_GETDENTS64` to recognize `riscv64` and `loongarch64` (syscall 61) and `i686` (syscall 220).

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test` | exit 0, all pass |

## Scope
**In scope**:
- `src/sys.rs` (declare `ioctl` extern and architecture constants)
- `src/ui.rs` (`get_terminal_width` using `ioctl`)

**Out of scope**:
- Traversal logic, scanner queue

## Git workflow
- Branch: `advisor/008-fix-multi-arch-syscalls`
- Commit message: `fix(sys): use portable ioctl binding and correct multi-arch syscall numbers`

## Steps

### Step 1: Add `ioctl` declaration to `src/sys.rs`
In `src/sys.rs`, add `ioctl` to `unsafe extern "C"`:
```rust
unsafe extern "C" {
    pub fn syscall(number: i64, ...) -> i64;
    pub fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    pub fn close(fd: i32) -> i32;
    pub fn ioctl(fd: i32, request: u64, ...) -> i32;
}
```

Update `SYS_GETDENTS64` mappings:
```rust
#[cfg(target_arch = "x86_64")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "loongarch64"))]
pub const SYS_GETDENTS64: i64 = 61;

#[cfg(target_arch = "arm")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(target_arch = "x86")]
pub const SYS_GETDENTS64: i64 = 220;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "arm",
    target_arch = "x86"
)))]
pub const SYS_GETDENTS64: i64 = 217;
```

### Step 2: Update `get_terminal_width` in `src/ui.rs`
Call `crate::sys::ioctl` instead of raw `syscall(16, ...)`:
```rust
pub fn get_terminal_width() -> usize {
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ret = unsafe { crate::sys::ioctl(1, TIOCGWINSZ as u64, &mut ws as *mut Winsize) };
    if ret == 0 && ws.ws_col > 10 {
        ws.ws_col as usize
    } else {
        80
    }
}
```

### Step 3: Verify
Run `cargo build` and `cargo test`.
Verify that `get_terminal_width()` returns a sensible terminal width (> 0) on the current host.

**Verify**: `cargo test` → all pass.

## Test plan
- Verify `get_terminal_width()` succeeds without error.
- Verify compilation succeeds on host architecture.
- Verification: `cargo test`

## Done criteria
- [ ] `src/ui.rs` no longer uses hardcoded syscall 16
- [ ] `SYS_GETDENTS64` covers aarch64, riscv64, arm, x86_64, and x86
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 008 updated to DONE

## STOP conditions
- If linking `ioctl` fails on Linux targets. Standard libc provides `ioctl`.

## Maintenance notes
- Using the libc `ioctl` symbol is safe because `dscan` already links libc for `open` and `close`.
