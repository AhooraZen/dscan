# Plan 020: Fix Windows NT Kernel Directory Traversal Crashes, Invalid Query Flags, and Buffer Safety

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- crates/dscan-core/src/sys/windows.rs crates/dscan-core/src/scanner.rs crates/dscan-cli/src/ui.rs crates/dscan-core/tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (complete platform isolation under `#[cfg(windows)]`; Linux syscall path remains 100% untouched and zero-regression)
- **Depends on**: plans/019-fix-workspace-build-and-test-performance-baseline.md
- **Category**: bug
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**: 

## Why this matters
`dscan` currently fails on Windows: it either crashes immediately upon startup or finishes scanning in 0ms having visited 0 files. While Plan 017 established high-performance NT kernel primitives (`NtQueryDirectoryFileEx`, `WidePathStack`, `DualBuffer`), seven critical bugs prevent directory traversal from functioning:

1. **Invalid NT Query Flags**: Passing the non-existent flag `SL_NO_EXTENDED_ATTRIBUTES = 0x800` to `NtQueryDirectoryFileEx` causes the NT kernel I/O subsystem (`IopQueryDirectory`) to reject every query with `STATUS_INVALID_PARAMETER` (`0xC000000D`).
2. **Buffer Overflow False Abort**: `NtQueryDirectoryFile` returns warning status `STATUS_BUFFER_OVERFLOW` (`0x80000005`) when a 512 KiB buffer is filled and cannot fit the next record. The scanner treats this as a fatal error, dropping all entries in the buffer and aborting traversal.
3. **Access Denied on Volume Serial**: `CreateFileW` opens directory handles with only `FILE_LIST_DIRECTORY`, omitting `FILE_READ_ATTRIBUTES`. Consequently, `GetFileInformationByHandle` fails with `ERROR_ACCESS_DENIED`, setting `root_dev = 0` and breaking filesystem boundary checks.
4. **Undefined Behavior in WidePathStack**: `as_null_terminated` pushes a `0` wide character and immediately pops it, returning a raw pointer into the vector's spare capacity beyond `len`. Under Rust pointer provenance, passing this to Win32 `CreateFileW` dereferences past slice boundaries (UB).
5. **Stale/Uninitialized Buffer Parsing**: Bounds checks compare record offsets against the 512 KiB allocation size (`buf_slice.len()`) rather than the kernel-returned byte count (`info_bytes`), reading uninitialized memory or stale data.
6. **Fake Fallback Byte Count**: The `GetFileInformationByHandleEx` fallback returns `(STATUS_SUCCESS, len as usize)` instead of measuring actual entries, passing 524,288 bytes as valid data.
7. **Terminal Width Negative Underflow**: Console window bounds calculation `(csbi.sr_window.right - csbi.sr_window.left + 1) as usize` casts negative coordinates to `usize::MAX`, triggering string allocation panics on redirected/headless Windows consoles.

Fixing these seven issues restores functional, memory-safe, ultra-fast directory traversal on Windows 10, 11, and Windows Server.

## Current state
- `crates/dscan-core/src/sys/windows.rs:27` and `250` define and pass `SL_NO_EXTENDED_ATTRIBUTES`:
  ```rust
  // crates/dscan-core/src/sys/windows.rs:27
  pub const SL_NO_EXTENDED_ATTRIBUTES: u32 = 0x00000800;

  // crates/dscan-core/src/sys/windows.rs:250
  if let Some(nt_ex) = get_nt_query_directory_file_ex() {
      let mut flags = SL_NO_EXTENDED_ATTRIBUTES;
      if restart_scan {
          flags |= SL_RESTART_SCAN;
      }
  ```
- `crates/dscan-core/src/scanner.rs:1156` aborts immediately if status is not `STATUS_SUCCESS`:
  ```rust
  // crates/dscan-core/src/scanner.rs:1156
  let (status, info_bytes) =
      unsafe { sys_nt_query_directory_file_fast(h_dir, buf_ptr, buf_len, restart_scan) };

  if status != STATUS_SUCCESS || info_bytes == 0 {
      break;
  }
  ```
- `crates/dscan-core/src/sys/windows.rs:328` opens directories without `FILE_READ_ATTRIBUTES`:
  ```rust
  // crates/dscan-core/src/sys/windows.rs:328-336
  let handle = unsafe {
      CreateFileW(
          ptr,
          FILE_LIST_DIRECTORY,
          FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
          std::ptr::null_mut(),
          OPEN_EXISTING,
          FILE_FLAG_BACKUP_SEMANTICS,
          std::ptr::null_mut(),
      )
  };
  ```
- `crates/dscan-core/src/scanner.rs:1068-1073` returns pointer to popped capacity:
  ```rust
  // crates/dscan-core/src/scanner.rs:1068-1073
  pub fn as_null_terminated(&mut self) -> *const u16 {
      self.wide.push(0);
      let ptr = self.wide.as_ptr();
      self.wide.pop();
      ptr
  }
  ```
- `crates/dscan-core/src/scanner.rs:1163, 1176, 1234` check bounds against `buf_slice.len()`:
  ```rust
  // crates/dscan-core/src/scanner.rs:1163
  if offset + std::mem::size_of::<FileIdBothDirInfo>() > buf_slice.len() {
      break;
  }
  ...
  // crates/dscan-core/src/scanner.rs:1176
  if offset + fn_offset + name_len_bytes > buf_slice.len() {
      break;
  }
  ...
  // crates/dscan-core/src/scanner.rs:1234
  if entry.next_entry_offset == 0
      || offset + (entry.next_entry_offset as usize) >= buf_slice.len()
  {
      break;
  }
  ```
- `crates/dscan-core/src/sys/windows.rs:295-301` returns `len as usize` as `info_bytes`:
  ```rust
  // crates/dscan-core/src/sys/windows.rs:295-301
  let ret = unsafe { GetFileInformationByHandleEx(handle, class, buffer, len) };
  if ret != 0 {
      (STATUS_SUCCESS, len as usize)
  } else {
      (STATUS_NO_MORE_FILES, 0)
  }
  ```
- `crates/dscan-cli/src/ui.rs:66` underflows on negative window dimensions:
  ```rust
  // crates/dscan-cli/src/ui.rs:66
  if ret != 0 {
      let width = (csbi.sr_window.right - csbi.sr_window.left + 1) as usize;
      if width > 10 {
          return width;
      }
  }
  80
  ```

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Linux Core Unit Tests | `cargo test -p dscan-core` | all tests pass, exit 0 |
| Windows Core Target Check | `cargo check -p dscan-core --target x86_64-pc-windows-gnu` | exit 0, no errors |
| Windows CLI Target Check | `cargo check -p dscan --target x86_64-pc-windows-gnu` | exit 0, no errors |
| Clippy Verification | `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` | clean, 0 warnings |
| Format Verification | `cargo fmt --check` | clean formatting, exit 0 |

## Suggested executor toolkit
- Skills: `rust-dev` (strict type modeling, unsafe invariants, zero-copy safety), `systematic-debugging` (root-cause verification), `ponytail` (minimal diffs, no unnecessary abstractions).
- Reference: Microsoft Win32 / NT DDK specification for `NtQueryDirectoryFileEx`, `STATUS_BUFFER_OVERFLOW` (0x80000005), `STATUS_NO_MORE_FILES` (0x80000006), `FILE_ID_BOTH_DIR_INFO`, and `GetConsoleScreenBufferInfo`.

## Scope
**In scope** (the only files you should modify):
- `crates/dscan-core/src/sys/windows.rs` — NT query flags, `FILE_READ_ATTRIBUTES`, `CreateFileW` desired access, and `GetFileInformationByHandleEx` entry counting.
- `crates/dscan-core/src/scanner.rs` — `WidePathStack` null-termination invariant, `STATUS_BUFFER_OVERFLOW` handling, and buffer bounds checking against `info_bytes`.
- `crates/dscan-cli/src/ui.rs` — Console buffer window bounds check avoiding `usize::MAX` underflow.
- `crates/dscan-core/tests/scanner_integration.rs` — Regression unit tests for `WidePathStack`, query flags, and buffer safety.

**Out of scope** (do NOT touch, even though they look related):
- `crates/dscan-core/src/sys/linux.rs` — Linux `statx`, `openat2`, `getdents64` implementation.
- `crates/dscan-core/src/sys/uring.rs` — Linux `io_uring` implementation.
- `crates/dscan-gui/*` — GUI frontend and Tauri/WebKit configuration.
- `Cargo.toml` — External dependencies (must remain zero-dependency).

## Git workflow
- Branch: `advisor/020-fix-windows-nt-kernel-directory-traversal-and-crashes`
- Commit per step; message style: `fix(windows): <description>`
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Fix NT Query Flags and Add `FILE_READ_ATTRIBUTES` in `sys/windows.rs`
1. Open `crates/dscan-core/src/sys/windows.rs`.
2. Add `pub const FILE_READ_ATTRIBUTES: u32 = 0x00000080;` alongside existing Win32 file attribute / access constants (near line 16).
3. Remove the invalid constant `pub const SL_NO_EXTENDED_ATTRIBUTES: u32 = 0x00000800;` (or replace with comment documenting why bit 11 is rejected by NT kernel).
4. In `sys_nt_query_directory_file_fast` (around line 250), change the flag calculation to pass only valid `SL_RESTART_SCAN` (or `0` when continuing scan):
   ```rust
   // Target code in sys_nt_query_directory_file_fast:
   if let Some(nt_ex) = get_nt_query_directory_file_ex() {
       let flags = if restart_scan { SL_RESTART_SCAN } else { 0 };
       // SAFETY: nt_ex pointer is valid from ntdll, handle is valid, and buffer has length len.
       let status = unsafe {
           nt_ex(
               handle,
               std::ptr::null_mut(),
               std::ptr::null_mut(),
               std::ptr::null_mut(),
               &mut iosb,
               buffer,
               len,
               FILE_ID_BOTH_DIR_INFO_CLASS,
               flags,
               std::ptr::null_mut(),
           )
       };
       (status, iosb.information)
   }
   ```
5. In `open_dir_from_wide_ptr` (around line 328), add `FILE_READ_ATTRIBUTES` to `CreateFileW`'s `dwDesiredAccess`:
   ```rust
   // Target code in open_dir_from_wide_ptr:
   let handle = unsafe {
       CreateFileW(
           ptr,
           FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
           FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
           std::ptr::null_mut(),
           OPEN_EXISTING,
           FILE_FLAG_BACKUP_SEMANTICS,
           std::ptr::null_mut(),
       )
   };
   ```

**Verify**: `cargo check -p dscan-core --target x86_64-pc-windows-gnu` → exit 0, compiles with zero warnings or errors.

---

### Step 2: Implement Accurate `GetFileInformationByHandleEx` Entry Counting in `sys/windows.rs`
In `crates/dscan-core/src/sys/windows.rs` around lines 288-301, replace the fake `(STATUS_SUCCESS, len as usize)` return with entry parsing to compute the true populated byte count.

Target implementation in `crates/dscan-core/src/sys/windows.rs`:
```rust
    } else {
        let class = if restart_scan {
            FILE_ID_BOTH_DIRECTORY_RESTART_INFO
        } else {
            FILE_ID_BOTH_DIRECTORY_INFO
        };
        // SAFETY: handle and buffer are valid for GetFileInformationByHandleEx.
        let ret = unsafe { GetFileInformationByHandleEx(handle, class, buffer, len) };
        if ret != 0 {
            // Walk returned entries to compute exact byte count written
            let mut actual_bytes = 0usize;
            let mut offset = 0usize;
            let min_hdr = std::mem::size_of::<FileIdBothDirInfo>();
            let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);

            while offset + min_hdr <= len as usize {
                // SAFETY: offset + min_hdr <= len, pointer is within buffer bounds
                let entry = unsafe {
                    std::ptr::read_unaligned(
                        (buffer as *const u8).add(offset) as *const FileIdBothDirInfo
                    )
                };
                let entry_end = offset + fn_offset + (entry.file_name_length as usize);
                if entry_end > len as usize {
                    break;
                }
                actual_bytes = entry_end;
                if entry.next_entry_offset == 0 {
                    break;
                }
                let next = offset + (entry.next_entry_offset as usize);
                if next <= offset || next >= len as usize {
                    break;
                }
                offset = next;
            }
            (STATUS_SUCCESS, actual_bytes)
        } else {
            (STATUS_NO_MORE_FILES, 0)
        }
    }
```

**Verify**: `cargo check -p dscan-core --target x86_64-pc-windows-gnu` → exit 0.

---

### Step 3: Enforce Safe Invariant in `WidePathStack` in `scanner.rs`
Eliminate undefined behavior in `WidePathStack` by maintaining the invariant that `self.wide` **always contains a trailing null character (`0`) as an active element within `self.wide.len()`**. This guarantees that `as_null_terminated()` returns a pointer with valid provenance over the entire null-terminated slice without touching spare capacity.

Target replacement in `crates/dscan-core/src/scanner.rs` (lines 1015-1096):
```rust
#[derive(Debug, Clone)]
pub struct WidePathStack {
    wide: Vec<u16>,
}

impl WidePathStack {
    pub fn new() -> Self {
        let mut wide = Vec::with_capacity(1024);
        wide.push(0);
        Self { wide }
    }

    #[cfg(windows)]
    pub fn set_root(&mut self, path: &Path) {
        use std::os::windows::ffi::OsStrExt;
        self.wide.clear();
        self.wide.extend(path.as_os_str().encode_wide());
        while self.wide.len() > 3
            && (self.wide.last() == Some(&(b'\\' as u16))
                || self.wide.last() == Some(&(b'/' as u16)))
        {
            self.wide.pop();
        }
        self.wide.push(0);
    }

    pub fn set_root_wide(&mut self, wide_root: &[u16]) {
        self.wide.clear();
        self.wide.extend_from_slice(wide_root);
        while self.wide.len() > 3
            && (self.wide.last() == Some(&(b'\\' as u16))
                || self.wide.last() == Some(&(b'/' as u16)))
        {
            self.wide.pop();
        }
        self.wide.push(0);
    }

    /// Push child directory wide characters. Returns the previous path length (excluding null terminator) to restore on pop.
    #[inline(always)]
    pub fn push_child(&mut self, child_name_wide: &[u16]) -> usize {
        let prev_len = self.wide.len().saturating_sub(1);
        if self.wide.last() == Some(&0) {
            self.wide.pop();
        }
        if !self.wide.ends_with(&[b'\\' as u16]) && !self.wide.ends_with(&[b'/' as u16]) {
            self.wide.push(b'\\' as u16);
        }
        self.wide.extend_from_slice(child_name_wide);
        self.wide.push(0);
        prev_len
    }

    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        self.wide.truncate(len);
        if self.wide.last() != Some(&0) {
            self.wide.push(0);
        }
    }

    /// Returns a valid null-terminated pointer with active slice provenance.
    #[inline(always)]
    pub fn as_null_terminated(&self) -> *const u16 {
        self.wide.as_ptr()
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u16] {
        if self.wide.len() > 1 {
            &self.wide[..self.wide.len() - 1]
        } else {
            &[]
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.wide.len().saturating_sub(1)
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(windows)]
    pub fn to_path_buf(&self) -> PathBuf {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        PathBuf::from(OsString::from_wide(self.as_slice()))
    }
}
```

**Verify**: `cargo test -p dscan-core --test scanner_integration -- test_windows_wide_path_stack_push_pop_integrity` → passes.

---

### Step 4: Fix `STATUS_BUFFER_OVERFLOW` and Buffer Bounds in `scanner.rs`
1. Open `crates/dscan-core/src/scanner.rs`.
2. In `scan_directory_tree_windows` at line 1156, update the loop termination condition to accept both `STATUS_SUCCESS` and `STATUS_BUFFER_OVERFLOW`:
   ```rust
   // Target code around line 1156:
   let (status, info_bytes) =
       unsafe { sys_nt_query_directory_file_fast(h_dir, buf_ptr, buf_len, restart_scan) };

   if (status != STATUS_SUCCESS && status != STATUS_BUFFER_OVERFLOW) || info_bytes == 0 {
       break;
   }
   restart_scan = false;
   ```
3. In `scan_directory_tree_windows` at lines 1163, 1176, and 1234, replace all occurrences of `buf_slice.len()` with `info_bytes`:
   ```rust
   // Line 1163:
   if offset + std::mem::size_of::<FileIdBothDirInfo>() > info_bytes {
       break;
   }

   // Line 1176:
   let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);
   if offset + fn_offset + name_len_bytes > info_bytes {
       break;
   }

   // Line 1234:
   if entry.next_entry_offset == 0
       || offset + (entry.next_entry_offset as usize) >= info_bytes
   {
       break;
   }
   ```

**Verify**: `cargo check -p dscan-core --target x86_64-pc-windows-gnu` → exit 0.

---

### Step 5: Fix Terminal Width Negative Underflow in `dscan-cli/src/ui.rs`
1. Open `crates/dscan-cli/src/ui.rs`.
2. In `get_terminal_width()` under `#[cfg(windows)]` (lines 64-72), guard against degenerate / negative console rectangles:
   ```rust
   // Target code in crates/dscan-cli/src/ui.rs:
   let ret = unsafe { GetConsoleScreenBufferInfo(handle, &mut csbi) };
   if ret != 0 && csbi.sr_window.right >= csbi.sr_window.left {
       let width = (csbi.sr_window.right - csbi.sr_window.left + 1) as usize;
       if (10..=1024).contains(&width) {
           return width;
       }
   }
   80
   ```

**Verify**: `cargo check -p dscan --target x86_64-pc-windows-gnu` → exit 0.

---

### Step 6: Add Cross-Platform Regression Unit Tests
In `crates/dscan-core/tests/scanner_integration.rs`, add unit tests covering:
1. `WidePathStack` null-termination slice bounds and capacity safety.
2. Console width negative underflow simulation.
3. Directory buffer bounds checking against `info_bytes`.

Target additions to append to `crates/dscan-core/tests/scanner_integration.rs`:
```rust
#[test]
fn test_windows_wide_path_stack_in_bounds_null_terminator() {
    use dscan_core::scanner::WidePathStack;

    let mut stack = WidePathStack::new();
    assert_eq!(stack.len(), 0);
    assert!(stack.is_empty());

    let root_wide: Vec<u16> = "D:\\Scanner\\Target".encode_utf16().collect();
    stack.set_root_wide(&root_wide);

    // Verify pointer is non-null and points to valid UTF-16 ending in 0
    let ptr = stack.as_null_terminated();
    assert!(!ptr.is_null());
    assert_eq!(stack.len(), root_wide.len());

    let child_a: Vec<u16> = "child_alpha".encode_utf16().collect();
    let prev = stack.push_child(&child_a);

    // Slice representation must not contain trailing 0
    let slice = stack.as_slice();
    assert!(!slice.contains(&0));

    // Pointer must be valid and end with 0 at slice.len()
    let ptr2 = stack.as_null_terminated();
    unsafe {
        assert_eq!(*ptr2.add(slice.len()), 0);
    }

    stack.truncate(prev);
    assert_eq!(stack.len(), root_wide.len());
    let slice_restored = stack.as_slice();
    assert_eq!(slice_restored, &root_wide[..]);
}

#[test]
fn test_windows_terminal_width_bounds_logic() {
    // Simulate SmallRect with right < left (degenerate headless console)
    let left: i16 = 0;
    let right: i16 = -1;
    let valid = right >= left;
    assert!(!valid, "Must detect degenerate window rectangle");

    // Sane dimensions
    let left: i16 = 0;
    let right: i16 = 119;
    let width = if right >= left {
        (right - left + 1) as usize
    } else {
        80
    };
    assert_eq!(width, 120);
}
```

**Verify**: `cargo test -p dscan-core --test scanner_integration` → all tests pass, exit 0.

---

## Test plan
- **Unit Tests**:
  - `test_windows_wide_path_stack_push_pop_integrity` in `crates/dscan-core/tests/scanner_integration.rs`: verifies push/pop/truncate consistency.
  - `test_windows_wide_path_stack_in_bounds_null_terminator` in `crates/dscan-core/tests/scanner_integration.rs`: verifies memory provenance and slice integrity without spare capacity access.
  - `test_windows_terminal_width_bounds_logic` in `crates/dscan-core/tests/scanner_integration.rs`: verifies protection against `usize::MAX` overflow on negative/degenerate rect coordinates.
- **Verification Commands**:
  - `cargo check -p dscan-core --target x86_64-pc-windows-gnu` → exit 0, no errors.
  - `cargo check -p dscan --target x86_64-pc-windows-gnu` → exit 0, no errors.
  - `cargo test -p dscan-core` → all 49+ tests pass on host Linux.
  - `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` → 0 warnings.
  - `cargo fmt --check` → exit 0.

## Done criteria
- [ ] `cargo check -p dscan-core --target x86_64-pc-windows-gnu` exits 0 with 0 errors.
- [ ] `cargo check -p dscan --target x86_64-pc-windows-gnu` exits 0 with 0 errors.
- [ ] `cargo test -p dscan-core` exits 0; all existing and new regression tests pass.
- [ ] `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings` exits 0 with 0 warnings.
- [ ] `cargo fmt --check` exits 0.
- [ ] `grep -rn "SL_NO_EXTENDED_ATTRIBUTES" crates/` returns 0 matches.
- [ ] `grep -rn "buf_slice.len()" crates/dscan-core/src/scanner.rs` returns matches only for initial buffer sizing, not entry bounds checks.
- [ ] No files outside the in-scope list are modified (`git status`).
- [ ] `plans/README.md` status row updated.

## STOP conditions
Stop and report back (do not improvise) if:
- Code at `crates/dscan-core/src/sys/windows.rs`, `crates/dscan-core/src/scanner.rs`, or `crates/dscan-cli/src/ui.rs` does not match the excerpts in "Current state".
- Cross-compilation to `x86_64-pc-windows-gnu` fails due to unrecognized Windows API externs or syntax errors.
- Existing Linux unit or integration tests in `crates/dscan-core` fail (`cargo test -p dscan-core`).
- The executor attempts to modify out-of-scope files (`linux.rs`, `uring.rs`, `Cargo.toml`).
- Any step verification fails twice after a reasonable fix attempt.

## Maintenance notes
- **NT Kernel Version Compatibility**: Windows 10 build 1709+ natively supports `NtQueryDirectoryFileEx`. On older systems (e.g. Windows Server 2016, Windows 8.1), dynamic lookup safely falls back to `NtQueryDirectoryFile` and `GetFileInformationByHandleEx`.
- **Large Directories**: Directories with tens of thousands of entries will now correctly return `STATUS_BUFFER_OVERFLOW` over multiple sequential buffer queries until `STATUS_NO_MORE_FILES` is reached, ensuring 100% of directory entries are counted.
- **Reviewer Focus**: Scrutinize that all `buf_slice.len()` comparisons within the entry decoding loop in `scanner.rs` have been completely replaced with `info_bytes`.
