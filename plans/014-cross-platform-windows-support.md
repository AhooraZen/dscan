# Plan 014: Cross-Platform Windows Support: Ultra-Fast Directory Traversal via FILE_ID_BOTH_DIR_INFO, Native AllocationSize Accounting, and Zero-Regression Platform Facade
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 63f7cc5..HEAD -- src/sys.rs src/scanner.rs src/ui.rs src/cli.rs src/main.rs tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: LOW (complete platform isolation via `#[cfg(unix)]` and `#[cfg(windows)]`; Linux code path suffers zero regression)
- **Depends on**: plans/013-extreme-performance-architecture.md
- **Category**: migration
- **Planned at**: commit `63f7cc5`, 2026-10-05
- **Issue**: 

## Why this matters
Currently, `dscan` is hardcoded to Linux syscalls (`getdents64`, `statx`, `ioctl(TIOCGWINSZ)`, `std::os::unix`). It fails to compile on Windows (`cargo check --target x86_64-pc-windows-msvc` fails immediately).

Naïve cross-platform ports of disk space analyzers suffer severe performance collapse on Windows:
1. Using `std::fs::read_dir` followed by `std::fs::metadata()` issues two blocking Win32 I/O calls (`FindFirstFileExW` / `FindNextFileW` + `GetFileAttributesExW`) for every file entry, causing millions of kernel round-trips.
2. `std::fs::Metadata::len()` reports logical file length, completely failing to measure actual allocated disk space (ignoring NTFS cluster size rounding, sparse files, and NTFS volume compression).
3. Naive traversal follows Windows Directory Junctions (e.g. `C:\Users\Default\Application Data` pointing to `C:\Users\Default\AppData\Roaming`), creating infinite recursive traversal cycles and stack overflow panics.

This plan achieves high-performance native Windows support with zero external crate dependencies (pure Rust std + raw Win32 ABI externs) by:
- Querying directory entries in bulk via `GetFileInformationByHandleEx` with `FileIdBothDirectoryInfo` (`FILE_ID_BOTH_DIR_INFO`), filling 512 KiB page-aligned buffers with hundreds of entries in a single kernel transition.
- Reading `AllocationSize` directly from each `FILE_ID_BOTH_DIR_INFO` record in the batch buffer, obtaining exact cluster-allocated physical disk usage with **zero secondary stat syscalls**.
- Guarding against infinite recursive loops and junction point traps via `FILE_ATTRIBUTE_REPARSE_POINT` (0x400) inspection.
- Enforcing filesystem boundary containment using `ByHandleFileInformation.dwVolumeSerialNumber`.
- Detecting terminal columns via `GetConsoleScreenBufferInfo` and enabling ANSI color rendering via `ENABLE_VIRTUAL_TERMINAL_PROCESSING`.
- Partitioning `src/sys.rs` into a zero-cost facade (`src/sys/mod.rs`, `src/sys/linux.rs`, `src/sys/windows.rs`), guaranteeing that Linux retains 100% of its extreme-performance `statx` and `getdents64` engine without a single instruction of overhead.

## Current state
- `src/sys.rs:1-3` imports unix-specific traits and declares Linux-only types:
  ```rust
  use std::ffi::CString;
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;
  ```
- `src/scanner.rs:5-6` imports `std::os::unix::ffi::OsStrExt` and `std::os::unix::fs::MetadataExt`:
  ```rust
  use std::os::unix::ffi::OsStrExt;
  use std::os::unix::fs::MetadataExt;
  ```
- `src/scanner.rs:565-566` relies on Unix `MetadataExt::dev()` for boundary checks:
  ```rust
  let root_meta = root.metadata()?;
  let root_dev = root_meta.dev();
  ```
- `src/ui.rs:25-39` uses Linux `ioctl(1, TIOCGWINSZ, ...)`:
  ```rust
  pub fn get_terminal_width() -> usize {
      let mut ws = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
      let ret = unsafe { crate::sys::ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
      if ret == 0 && ws.ws_col > 10 { ws.ws_col as usize } else { 80 }
  }
  ```
- `src/cli.rs:22-29` defines hardcoded Unix default excludes (`/proc`, `/sys`, `/dev`, `/run`, `/tmp`):
  ```rust
  let mut custom_excludes = vec![
      "/proc".to_string(),
      "/sys".to_string(),
      "/dev".to_string(),
      "/run".to_string(),
      "/tmp".to_string(),
      ".git".to_string(),
  ];
  ```
- `tests/scanner_integration.rs:3` imports `std::os::unix::fs::symlink` and runs `du -s -B1`.

### Repo Conventions
- **Zero External Dependencies**: No `windows`, `winapi`, or `windows-sys` crates. All Win32 ABI structs and functions must be declared directly via `extern "system"` and primitive standard types.
- **Idiomatic Rust 2024**: Strict borrow checker compliance, memory alignment safety using `read_unaligned`, and zero `unwrap()` panics in I/O loops.
- **Zero Regression**: Linux builds must compile identically and execute the existing assembly paths.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Linux Build | `cargo build --release` | exit 0 |
| Linux Test Suite | `cargo test` | exit 0, all 21+ tests pass |
| Lint Check | `cargo clippy --all-targets -- -D warnings` | exit 0, zero warnings |
| Format Check | `cargo fmt -- --check` | exit 0 |
| Windows Compilation (cross or native) | `cargo check --target x86_64-pc-windows-gnu` (when toolchain installed) | exit 0 |

## Suggested executor toolkit
- `rust-dev` skill for idiomatic Rust FFI, pointer safety, and conditional compilation.
- `ponytail` discipline for minimal, focused diffs without unnecessary abstractions.

## Scope
**In scope**:
- `src/sys.rs` -> convert into `src/sys/mod.rs`, `src/sys/linux.rs`, `src/sys/windows.rs`.
- `src/ui.rs`: Add Windows console width detection (`GetConsoleScreenBufferInfo`) under `#[cfg(windows)]`.
- `src/cli.rs`: Add Windows default system excludes (`$Recycle.Bin`, `System Volume Information`, etc.) under `#[cfg(windows)]`.
- `src/main.rs`: Enable ANSI escape codes on Windows via `enable_virtual_terminal_processing()`.
- `src/scanner.rs`: Implement native Windows traversal via `GetFileInformationByHandleEx(..., FileIdBothDirectoryInfo, ...)` while preserving Linux traversal untouched.
- `tests/scanner_integration.rs`: Guard Unix-specific tests (`symlink`, `du`) with `#[cfg(unix)]`.

**Out of scope**:
- Adding any third-party crate dependencies to `Cargo.toml`.
- Touching `src/work_stealing.rs` (Chase-Lev deque is platform-agnostic).
- Touching `src/format.rs` (byte unit formatting is pure math).

## Git workflow
- Branch: `advisor/014-windows-support`
- Commit per step with conventional commit messages (`feat: ...`, `refactor: ...`).

---

## Steps

### Step 1: Split `src/sys.rs` into Modular Facade (`src/sys/mod.rs` and `src/sys/linux.rs`)
Move existing Linux-specific ABI definitions into `src/sys/linux.rs` and create `src/sys/mod.rs` as the platform facade.

1. Move `/home/ahoura/dscan/src/sys.rs` to `/home/ahoura/dscan/src/sys/linux.rs`.
2. Create `/home/ahoura/dscan/src/sys/mod.rs`:
   ```rust
   #[cfg(unix)]
   mod linux;
   #[cfg(unix)]
   pub use linux::*;

   #[cfg(windows)]
   mod windows;
   #[cfg(windows)]
   pub use windows::*;
   ```
3. Create placeholder `/home/ahoura/dscan/src/sys/windows.rs` with `pub fn open_dir() {}` to satisfy conditional module discovery.

**Verify**: `cargo check && cargo test` → exit 0, all 21 tests pass on Linux.

---

### Step 2: Implement Zero-Dependency Win32 ABI Bindings in `src/sys/windows.rs`
Implement the raw Win32 structures, constants, and functions required for directory enumeration, volume tracking, and console buffer inspection in `src/sys/windows.rs`:

```rust
use std::ffi::c_void;
use std::path::Path;

pub type RawHandle = *mut c_void;
pub const INVALID_HANDLE_VALUE: RawHandle = -1isize as RawHandle;

pub const FILE_LIST_DIRECTORY: u32 = 0x0001;
pub const FILE_SHARE_READ: u32 = 0x00000001;
pub const FILE_SHARE_WRITE: u32 = 0x00000002;
pub const FILE_SHARE_DELETE: u32 = 0x00000004;
pub const OPEN_EXISTING: u32 = 3;
pub const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
pub const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;

pub const FILE_ID_BOTH_DIRECTORY_INFO: u32 = 0xa;
pub const FILE_ID_BOTH_DIRECTORY_RESTART_INFO: u32 = 0xb;

pub const ERROR_NO_MORE_FILES: u32 = 38;
pub const ERROR_MORE_DATA: u32 = 234;

pub const STD_OUTPUT_HANDLE: u32 = 0xfffffff5;
pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FileIdBothDirInfo {
    pub next_entry_offset: u32,
    pub file_index: u32,
    pub creation_time: i64,
    pub last_access_time: i64,
    pub last_write_time: i64,
    pub change_time: i64,
    pub end_of_file: i64,
    pub allocation_size: i64,
    pub file_attributes: u32,
    pub file_name_length: u32,
    pub ea_size: u32,
    pub short_name_length: i8,
    pub short_name: [u16; 12],
    pub file_id: i64,
    pub file_name: [u16; 1],
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct ByHandleFileInformation {
    pub dw_file_attributes: u32,
    pub ft_creation_time: [u32; 2],
    pub ft_last_access_time: [u32; 2],
    pub ft_last_write_time: [u32; 2],
    pub dw_volume_serial_number: u32,
    pub n_file_size_high: u32,
    pub n_file_size_low: u32,
    pub n_number_of_links: u32,
    pub n_file_index_high: u32,
    pub n_file_index_low: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SmallRect {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ConsoleScreenBufferInfo {
    pub dw_size: Coord,
    pub dw_cursor_position: Coord,
    pub w_attributes: u16,
    pub sr_window: SmallRect,
    pub dw_maximum_window_size: Coord,
}

unsafe extern "system" {
    pub fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: *mut c_void,
    ) -> RawHandle;

    pub fn CloseHandle(hObject: RawHandle) -> i32;

    pub fn GetFileInformationByHandleEx(
        hFile: RawHandle,
        FileInformationClass: u32,
        lpFileInformation: *mut c_void,
        dwBufferSize: u32,
    ) -> i32;

    pub fn GetFileInformationByHandle(
        hFile: RawHandle,
        lpFileInformation: *mut ByHandleFileInformation,
    ) -> i32;

    pub fn GetLastError() -> u32;

    pub fn GetStdHandle(nStdHandle: u32) -> RawHandle;

    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: RawHandle,
        lpConsoleScreenBufferInfo: *mut ConsoleScreenBufferInfo,
    ) -> i32;

    pub fn GetConsoleMode(hConsoleHandle: RawHandle, lpMode: *mut u32) -> i32;
    pub fn SetConsoleMode(hConsoleHandle: RawHandle, dwMode: u32) -> i32;
}

/// Open a directory handle for raw buffer enumeration using FILE_FLAG_BACKUP_SEMANTICS.
pub fn open_dir(path: &Path) -> Option<RawHandle> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    // SAFETY: wide is a valid null-terminated UTF-16 slice.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        None
    } else {
        Some(handle)
    }
}

/// Safely close a Win32 file handle.
pub fn close_handle(handle: RawHandle) {
    if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
        // SAFETY: handle is checked for non-null and INVALID_HANDLE_VALUE.
        unsafe { CloseHandle(handle) };
    }
}

/// Retrieve the volume serial number for filesystem boundary enforcement.
pub fn get_volume_serial_number(handle: RawHandle) -> Option<u64> {
    let mut info = ByHandleFileInformation::default();
    // SAFETY: info is a valid ByHandleFileInformation struct.
    let ret = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if ret != 0 {
        Some(info.dw_volume_serial_number as u64)
    } else {
        None
    }
}

/// Enable Windows Virtual Terminal Processing for neon ANSI terminal escape output.
pub fn enable_virtual_terminal_processing() {
    // SAFETY: GetStdHandle and console mode calls pass valid pointers and handles.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
            let mut mode = 0u32;
            if GetConsoleMode(handle, &mut mode) != 0 {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}
```

**Verify**: `cargo check && cargo test` → exit 0 on Linux.

---

### Step 3: Implement Windows Console Width Detection and ANSI Color Support
In `src/ui.rs`:
1. Guard `Winsize` and `TIOCGWINSZ` with `#[cfg(unix)]`.
2. Implement `get_terminal_width` for both Unix and Windows:
   ```rust
   #[cfg(unix)]
   pub fn get_terminal_width() -> usize {
       let mut ws = Winsize {
           ws_row: 0,
           ws_col: 0,
           ws_xpixel: 0,
           ws_ypixel: 0,
       };
       // SAFETY: ws is a valid Winsize struct passed to standard TIOCGWINSZ ioctl on stdout fd (1).
       let ret = unsafe { crate::sys::ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
       if ret == 0 && ws.ws_col > 10 {
           ws.ws_col as usize
       } else {
           80
       }
   }

   #[cfg(windows)]
   pub fn get_terminal_width() -> usize {
       use crate::sys::{
           ConsoleScreenBufferInfo, Coord, GetConsoleScreenBufferInfo, GetStdHandle,
           STD_OUTPUT_HANDLE, SmallRect,
       };
       let mut csbi = ConsoleScreenBufferInfo {
           dw_size: Coord { x: 0, y: 0 },
           dw_cursor_position: Coord { x: 0, y: 0 },
           w_attributes: 0,
           sr_window: SmallRect {
               left: 0,
               top: 0,
               right: 0,
               bottom: 0,
           },
           dw_maximum_window_size: Coord { x: 0, y: 0 },
       };
       // SAFETY: handle retrieved via standard GetStdHandle and passed to GetConsoleScreenBufferInfo.
       let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
       let ret = unsafe { GetConsoleScreenBufferInfo(handle, &mut csbi) };
       if ret != 0 {
           let width = (csbi.sr_window.right - csbi.sr_window.left + 1) as usize;
           if width > 10 {
               return width;
           }
       }
       80
   }
   ```
3. In `src/main.rs`:
   Call `dscan::sys::enable_virtual_terminal_processing()` at start of `main`:
   ```rust
   fn main() {
       #[cfg(windows)]
       dscan::sys::enable_virtual_terminal_processing();
   
       let options = match CliOptions::parse() {
           Some(opts) => opts,
           None => return,
       };
       // ...
   ```

**Verify**: `cargo check && cargo test` → exit 0 on Linux.

---

### Step 4: Implement Platform-Specific Default Excludes in `src/cli.rs`
Update `parse_from_args` in `src/cli.rs` to configure sensible OS-specific default excluded paths:
```rust
#[cfg(unix)]
let mut custom_excludes = vec![
    "/proc".to_string(),
    "/sys".to_string(),
    "/dev".to_string(),
    "/run".to_string(),
    "/tmp".to_string(),
    ".git".to_string(),
];

#[cfg(windows)]
let mut custom_excludes = vec![
    ".git".to_string(),
    "$Recycle.Bin".to_string(),
    "System Volume Information".to_string(),
    "pagefile.sys".to_string(),
    "hiberfil.sys".to_string(),
    "dumpstack.log.sys".to_string(),
];
```

**Verify**: `cargo test cli::tests::test_cli_defaults` → exit 0 on Linux.

---

### Step 5: Implement Native Windows Traversal in `src/scanner.rs`
1. Split root device acquisition in `run_scan`:
   ```rust
   #[cfg(unix)]
   let root_dev = {
       use std::os::unix::fs::MetadataExt;
       root_meta.dev()
   };

   #[cfg(windows)]
   let root_dev = {
       if let Some(h) = crate::sys::open_dir(root) {
           let dev = crate::sys::get_volume_serial_number(h).unwrap_or(0);
           crate::sys::close_handle(h);
           dev
       } else {
           0
       }
   };
   ```
2. Split `scan_directory_tree` into platform branches or conditional helper functions.
   On Linux: Retain the exact current `scan_directory_tree` (`getdents64` + `statx`).
   On Windows: Implement `scan_directory_tree_windows`:
   ```rust
   #[cfg(windows)]
   fn scan_directory_tree_windows(
       dir_path: &Path,
       state: &Arc<GlobalState>,
       worker: &Worker<PathBuf>,
       buffer: &mut AlignedBuffer,
       local_dirs: &mut HashMap<PathBuf, u64>,
       local_top_files: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
       local_files: &mut u64,
       local_bytes: &mut u64,
   ) {
       use std::ffi::OsString;
       use std::os::windows::ffi::OsStringExt;
       use crate::sys::*;

       let mut local_dir_size: u64 = 0;
       let mut sub_dirs = Vec::new();

       let root_dev = state.config.root_dev;
       let top_limit = state.config.top_limit;
       let excludes = &state.config.excludes;

       if let Some(h_dir) = open_dir(dir_path) {
           if !state.config.cross_filesystems {
               if let Some(vol) = get_volume_serial_number(h_dir) {
                   if vol != root_dev {
                       close_handle(h_dir);
                       return;
                   }
               }
           }

           let buf_slice = buffer.as_mut_slice();
           let buf_ptr = buf_slice.as_mut_ptr() as *mut std::ffi::c_void;
           let buf_len = buf_slice.len() as u32;

           loop {
               // SAFETY: h_dir is a valid directory handle opened with FILE_FLAG_BACKUP_SEMANTICS,
               // buf_ptr is a valid aligned buffer pointer.
               let ret = unsafe {
                   GetFileInformationByHandleEx(h_dir, FILE_ID_BOTH_DIRECTORY_INFO, buf_ptr, buf_len)
               };

               if ret == 0 {
                   break;
               }

               let mut offset = 0usize;
               loop {
                   let entry_ptr = unsafe { (buf_ptr as *const u8).add(offset) as *const FileIdBothDirInfo };
                   // SAFETY: read_unaligned prevents unaligned memory faults from misaligned filesystem records.
                   let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };

                   let name_len_bytes = entry.file_name_length as usize;
                   let name_len_wchars = name_len_bytes / std::mem::size_of::<u16>();

                   let file_name_ptr = unsafe {
                       let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);
                       (entry_ptr as *const u8).add(fn_offset) as *const u16
                   };

                   let name_slice = unsafe {
                       std::slice::from_raw_parts(file_name_ptr, name_len_wchars)
                   };

                   let is_dot = name_slice == [b'.' as u16];
                   let is_dotdot = name_slice == [b'.' as u16, b'.' as u16];

                   if !is_dot && !is_dotdot {
                       let os_name = OsString::from_wide(name_slice);
                       let child_path = dir_path.join(&os_name);
                       let child_path_str = child_path.to_string_lossy();

                       let mut excluded = false;
                       for exc in excludes {
                           let exc_str = String::from_utf8_lossy(exc);
                           if child_path_str.ends_with(exc_str.as_ref()) || os_name == exc_str.as_ref() {
                               excluded = true;
                               break;
                           }
                       }

                       if !excluded {
                           let attrs = entry.file_attributes;
                           let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                           let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                           if is_dir {
                               if !is_reparse || state.config.follow_symlinks {
                                   sub_dirs.push(child_path);
                               }
                           } else {
                               // Regular file: AllocationSize gives actual disk cluster allocation!
                               let sz = if entry.allocation_size > 0 {
                                   entry.allocation_size as u64
                               } else {
                                   entry.end_of_file.max(0) as u64
                               };

                               local_dir_size += sz;
                               *local_files += 1;
                               *local_bytes += sz;

                               if *local_files >= 1024 {
                                   state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                                   state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                                   *local_bytes = 0;
                                   *local_files = 0;
                               }

                               push_top_file(local_top_files, top_limit, sz, || child_path);
                           }
                       }
                   }

                   if entry.next_entry_offset == 0 {
                       break;
                   }
                   offset += entry.next_entry_offset as usize;
               }
           }

           close_handle(h_dir);
       } else if let Ok(entries) = fs::read_dir(dir_path) {
           for entry in entries.flatten() {
               let path = entry.path();
               let p_bytes = path.to_string_lossy().as_bytes().to_vec();
               let name_bytes = entry.file_name().to_string_lossy().as_bytes().to_vec();

               if is_excluded_dir(&p_bytes, &name_bytes, excludes) {
                   continue;
               }

               if let Ok(meta) = entry.metadata() {
                   if meta.is_dir() {
                       sub_dirs.push(path);
                   } else {
                       let sz = meta.len();
                       local_dir_size += sz;
                       *local_files += 1;
                       *local_bytes += sz;

                       if *local_files >= 1024 {
                           state.total_bytes.fetch_add(*local_bytes, Ordering::Relaxed);
                           state.total_files.fetch_add(*local_files, Ordering::Relaxed);
                           *local_bytes = 0;
                           *local_files = 0;
                       }

                       push_top_file(local_top_files, top_limit, sz, || path);
                   }
               }
           }
       }

       *local_dirs.entry(dir_path.to_path_buf()).or_insert(0) += local_dir_size;

       if sub_dirs.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads {
           let half = sub_dirs.split_off(sub_dirs.len() / 2);
           for d in half {
               worker.push(d);
           }
           state.cvar.notify_all();
       }

       for sub in sub_dirs {
           scan_directory_tree_windows(
               &sub,
               state,
               worker,
               buffer,
               local_dirs,
               local_top_files,
               local_files,
               local_bytes,
           );
       }
   }
   ```
3. In `worker_loop`, invoke `scan_directory_tree` on `#[cfg(unix)]` and `scan_directory_tree_windows` on `#[cfg(windows)]`.

**Verify**: `cargo check && cargo test` → exit 0 on Linux.

---

### Step 6: Make Integration Tests Cross-Platform in `tests/scanner_integration.rs`
1. Add `#[cfg(unix)]` to tests that depend on Linux `du` and POSIX symlinks:
   - `test_synthetic_tree_matches_du`
   - `test_symlink_directory_traversal`
2. Keep `test_multi_threaded_scalability` active for all platforms.
3. Add cross-platform synthetic tree test verifying hierarchical directory rollup on both Linux and Windows.

**Verify**: `cargo test` → exit 0, all integration tests pass on Linux.

---

## Test plan
- Verify on Linux:
  - `cargo test --all-targets`: All existing tests (`chase_lev_tests`, `scanner_integration`, unit tests in `cli`, `scanner`, `sys`, `ui`, `work_stealing`) must pass without regression.
  - `cargo clippy --all-targets -- -D warnings`: Must produce zero warnings.
  - `cargo fmt -- --check`: Formatted according to repo standards.
- Verify for Windows:
  - If Windows target is installed: `cargo check --target x86_64-pc-windows-gnu` or `cargo check --target x86_64-pc-windows-msvc`.
  - Ensure zero unresolved symbols or platform-gated compilation errors.

## Done criteria
- [ ] `src/sys.rs` converted to `src/sys/mod.rs`, `src/sys/linux.rs`, `src/sys/windows.rs`.
- [ ] `src/sys/windows.rs` contains zero external crate dependencies (pure Win32 ABI bindings).
- [ ] `cargo check` and `cargo test` pass cleanly on Linux.
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0 with zero warnings.
- [ ] `cargo fmt -- --check` exits 0.
- [ ] No files outside the in-scope list are modified (`git status`).
- [ ] `plans/README.md` status row updated.

## STOP conditions
Stop and report back (do not improvise) if:
- Codebase files in `src/sys.rs`, `src/scanner.rs`, `src/ui.rs`, or `src/cli.rs` have drifted from the excerpts in "Current state".
- Attempting to support Windows requires adding external crates to `Cargo.toml`.
- Verification of any step fails twice after reasonable adjustments.

## Maintenance notes
- Windows `FILE_ID_BOTH_DIR_INFO` returns variable-length records aligned by `NextEntryOffset`. Always access via `read_unaligned` to prevent crashes on non-standard Windows filesystem drivers (FAT32/exFAT).
- Reparse points (`FILE_ATTRIBUTE_REPARSE_POINT`) must never be recursively followed unless `--follow-symlinks` is explicitly set, preventing junction loop traps.
