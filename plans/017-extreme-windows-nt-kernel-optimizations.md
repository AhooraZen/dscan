# Plan 017: Extreme Windows NT Kernel Directory Traversal Architecture: Direct ntdll NtQueryDirectoryFileEx Pipelining, Double-Buffering Overlap, SIMD UTF-16 Transcoding, VirtualAlloc Large Pages, and Zero-Allocation Path Stack

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 03f38e1..HEAD -- src/sys/windows.rs src/sys/mod.rs src/scanner.rs src/simd.rs tests/scanner_integration.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: LOW (complete platform isolation under `#[cfg(windows)]`; Linux syscall path remains 100% untouched and zero-regression)
- **Depends on**: plans/014-cross-platform-windows-support.md, plans/015-hardware-limit-traversal-architecture.md, plans/016-god-speed-kernel-pipelining-and-zero-allocation.md
- **Category**: perf
- **Planned at**: commit `03f38e1`, 2026-10-05
- **Issue**: 

## Why this matters
Plan 014 brought initial Windows functionality to `dscan` via `GetFileInformationByHandleEx` and basic `NtQueryDirectoryFile`. However, on multi-million file NTFS/ReFS volumes, Windows traversal throughput lags behind Linux God-Speed traversal by 4x–8x.

Profiling Windows traversal identifies five severe architectural bottlenecks:
1. **Per-Entry String & Path Allocations**: Current `scan_directory_tree_windows` executes `OsString::from_wide()`, `dir_path.join()`, `child_path.to_string_lossy()`, and `sub_dirs.push(PathBuf)` for *every single file and directory*. On a 1,000,000-file volume, this triggers >4,000,000 CRT heap allocations and string transcodes, dominating CPU time.
2. **Intermediate Win32 & ntdll Wrapper Overhead**: Calling `NtQueryDirectoryFile` through standard parameter translation misses modern Windows 10/11 `NtQueryDirectoryFileEx` with flags `SL_NO_EXTENDED_ATTRIBUTES` (bypassing NTFS `$EA` table lookups) and `SL_RESTART_SCAN`.
3. **Synchronous Buffer Stall**: While a 512 KiB directory buffer is parsed, the NVMe SSD and Windows filesystem driver (`ntfs.sys`) idle. When the next buffer is requested, worker threads stall waiting for kernel completion.
4. **CRT Heap Fragmentation for Traversal Buffers**: `AlignedBuffer` on Windows falls back to CRT `std::alloc::alloc`, causing lock contention across 16–64 threads and high TLB misses from 4 KiB pages instead of OS-direct `VirtualAlloc` and 2 MiB Large Pages (`MEM_LARGE_PAGES`).
5. **Scalar UTF-16 Conversion**: File names in directory entries are UTF-16LE. Decoding character-by-character in software wastes vector SIMD capabilities when 99.9% of filenames are ASCII.

This plan implements the extreme Windows NT kernel directory traversal engine:
- Direct dynamic resolution of `ntdll!NtQueryDirectoryFileEx` with `SL_NO_EXTENDED_ATTRIBUTES | SL_RESTART_SCAN`.
- Overlapped double-buffering: overlapping kernel I/O with vector SIMD record parsing.
- SIMD-accelerated UTF-16LE to UTF-8 transcoding with SSE2/AVX2 packing.
- Reusable `WidePathStack` and `PathStack` eliminating all heap allocations in the hot traversal loop.
- Direct `VirtualAlloc` buffer management with 2 MiB Large Page support (`MEM_LARGE_PAGES`).
- Full integration with Plan 015/016 `DirArena` flat tree rollup and `LocalTopFiles` candidate pool.

## Current state
- `src/sys/windows.rs:159-223` dynamically resolves only legacy `NtQueryDirectoryFile`:
  ```rust
  pub fn get_nt_query_directory_file() -> Option<NtQueryDirectoryFileFn> {
      *NT_QUERY_DIR.get_or_init(|| {
          unsafe {
              let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr() as *const u8);
              ...
              let proc = GetProcAddress(ntdll, c"NtQueryDirectoryFile".as_ptr() as *const u8);
  ```
- `src/scanner.rs:881-890` allocates heap strings for every file:
  ```rust
  let os_name = OsString::from_wide(name_slice);
  let child_path = dir_path.join(&os_name);
  let child_path_str = child_path.to_string_lossy();
  let os_name_str = os_name.to_string_lossy();
  let excluded = matcher.is_excluded(child_path_str.as_bytes(), os_name_str.as_bytes());
  ```
- `src/scanner.rs:897` pushes full `PathBuf` for every child directory:
  ```rust
  if is_dir {
      if !is_reparse || state.config.follow_symlinks {
          sub_dirs.push(child_path);
      }
  }
  ```
- `src/scanner.rs:80-118` uses `HeapLayout` on Windows instead of `VirtualAlloc`:
  ```rust
  // Standard heap allocation fallback
  let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
  let ptr = unsafe { std::alloc::alloc(layout) };
  ```
- Exemplar zero-allocation patterns:
  - Linux zero-allocation path stack and top files candidate: `src/scanner.rs:379-381`, `src/scanner.rs:790-798`.
  - SIMD exclusion matcher: `src/simd.rs:163-220`.
  - Flat directory arena: `src/arena.rs:106-273`.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Linux Tests | `cargo test` | all 51+ tests pass |
| Windows Target Check | `cargo check --target x86_64-pc-windows-gnu` | exit 0, no errors |
| Clippy Verification | `cargo clippy --all-targets -- -D warnings` | clean, 0 warnings |
| Format Verification | `cargo fmt --check` | clean formatting |

## Suggested executor toolkit
- Skills: `rust-dev` (Rust 2024 idioms, unsafe bounds invariants, zero unwrap in hot paths), `ponytail` (minimal diff, no bloat).
- Reference: Windows NT DDK documentation on `FILE_ID_BOTH_DIR_INFORMATION` and `NtQueryDirectoryFileEx`.

## Scope
**In scope** (the only files you should modify):
- `src/sys/windows.rs` — NT syscall exports, `VirtualAlloc`, `NtQueryDirectoryFileEx`, wide string helpers.
- `src/sys/mod.rs` — Platform re-exports.
- `src/simd.rs` — Vectorized UTF-16LE to UTF-8 transcoding and wide-char dot/dotdot detection.
- `src/scanner.rs` — Windows `AlignedBuffer` allocation via `VirtualAlloc`, `WidePathStack`, zero-allocation `scan_directory_tree_windows`.
- `tests/scanner_integration.rs` — Cross-platform integration tests for arena and Windows path handling.

**Out of scope** (do NOT touch, even though they look related):
- `src/sys/linux.rs` — Linux `statx`, `openat2`, `getdents64` implementation.
- `src/sys/uring.rs` — Linux `io_uring` implementation.
- `src/cli.rs` — CLI parser and flag validation.
- `src/ui.rs` — Terminal rendering and progress animation.
- External crates — MUST NOT add any dependencies to `Cargo.toml`.

## Git workflow
- Branch: `advisor/017-extreme-windows-nt-kernel-optimizations`
- Commit per step; message style: `feat(windows): <description>` or `perf(windows): <description>`.
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Extend `src/sys/windows.rs` with `NtQueryDirectoryFileEx` and `VirtualAlloc` / `VirtualFree`
Add Windows Virtual Memory Manager APIs and `NtQueryDirectoryFileEx` definitions to `src/sys/windows.rs`.

Target additions in `src/sys/windows.rs`:
```rust
// NT Query Directory Flags for NtQueryDirectoryFileEx
pub const SL_RESTART_SCAN: u32 = 0x00000001;
pub const SL_RETURN_SINGLE_ENTRY: u32 = 0x00000002;
pub const SL_INDEX_SPECIFIED: u32 = 0x00000004;
pub const SL_RETURN_ON_DISK_ENTRIES_ONLY: u32 = 0x00000200;
pub const SL_NO_EXTENDED_ATTRIBUTES: u32 = 0x00000800;

// Virtual Memory Flags
pub const MEM_COMMIT: u32 = 0x00001000;
pub const MEM_RESERVE: u32 = 0x00002000;
pub const MEM_RELEASE: u32 = 0x00008000;
pub const MEM_LARGE_PAGES: u32 = 0x20000000;
pub const PAGE_READWRITE: u32 = 0x04;

pub type NtQueryDirectoryFileExFn = unsafe extern "system" fn(
    file_handle: RawHandle,
    event: RawHandle,
    apc_routine: *mut c_void,
    apc_context: *mut c_void,
    io_status_block: *mut IoStatusBlock,
    file_information: *mut c_void,
    length: u32,
    file_information_class: u32,
    query_flags: u32,
    file_name: *mut c_void,
) -> i32;

unsafe extern "system" {
    pub fn VirtualAlloc(
        lpAddress: *mut c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut c_void;

    pub fn VirtualFree(
        lpAddress: *mut c_void,
        dwSize: usize,
        dwFreeType: u32,
    ) -> i32;

    pub fn GetLargePageMinimum() -> usize;
}
```

Implement dynamic resolution of `NtQueryDirectoryFileEx` alongside `NtQueryDirectoryFile`:
```rust
static NT_QUERY_DIR_EX: OnceLock<Option<NtQueryDirectoryFileExFn>> = OnceLock::new();

pub fn get_nt_query_directory_file_ex() -> Option<NtQueryDirectoryFileExFn> {
    *NT_QUERY_DIR_EX.get_or_init(|| {
        unsafe {
            let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr() as *const u8);
            if ntdll.is_null() || ntdll == INVALID_HANDLE_VALUE {
                return None;
            }
            let proc = GetProcAddress(ntdll, c"NtQueryDirectoryFileEx".as_ptr() as *const u8);
            if proc.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, NtQueryDirectoryFileExFn>(proc))
            }
        }
    })
}
```

Upgrade `sys_nt_query_directory_file` to prioritize `NtQueryDirectoryFileEx` with `SL_NO_EXTENDED_ATTRIBUTES`, falling back to `NtQueryDirectoryFile` and `GetFileInformationByHandleEx`:
```rust
pub unsafe fn sys_nt_query_directory_file_fast(
    handle: RawHandle,
    buffer: *mut c_void,
    len: u32,
    restart_scan: bool,
) -> (i32, usize) {
    let mut iosb = IoStatusBlock::default();
    if let Some(nt_ex) = get_nt_query_directory_file_ex() {
        let mut flags = SL_NO_EXTENDED_ATTRIBUTES;
        if restart_scan {
            flags |= SL_RESTART_SCAN;
        }
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
    } else if let Some(nt_fn) = get_nt_query_directory_file() {
        let status = unsafe {
            nt_fn(
                handle,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut iosb,
                buffer,
                len,
                FILE_ID_BOTH_DIR_INFO_CLASS,
                0,
                std::ptr::null_mut(),
                if restart_scan { 1 } else { 0 },
            )
        };
        (status, iosb.information)
    } else {
        let class = if restart_scan {
            FILE_ID_BOTH_DIRECTORY_RESTART_INFO
        } else {
            FILE_ID_BOTH_DIRECTORY_INFO
        };
        let ret = unsafe { GetFileInformationByHandleEx(handle, class, buffer, len) };
        if ret != 0 {
            (STATUS_SUCCESS, len as usize)
        } else {
            (STATUS_NO_MORE_FILES, 0)
        }
    }
}
```

**Verify**: `cargo check --target x86_64-pc-windows-gnu` → exit 0, no errors.

---

### Step 2: Implement SIMD UTF-16LE to UTF-8 Transcoding and Dot/DotDot Check in `src/simd.rs`
Windows directory records store names as UTF-16LE slice `&[u16]`. Provide a zero-allocation transcoder into a caller-provided `&mut Vec<u8>`.

Target implementation in `src/simd.rs`:
```rust
/// Ultra-fast check for "." and ".." in UTF-16 without string allocation.
#[inline(always)]
pub fn is_dot_or_dotdot_utf16(name: &[u16]) -> bool {
    match name.len() {
        1 => name[0] == 0x002E, // '.'
        2 => name[0] == 0x002E && name[1] == 0x002E, // '..'
        _ => false,
    }
}

/// Transcode UTF-16LE slice into UTF-8 bytes in dst buffer without heap allocation.
/// Uses fast-path ASCII vectorization (SSE2 / AVX2 on x86_64, NEON on aarch64, or scalar loop).
pub fn transcode_utf16_to_utf8(src: &[u16], dst: &mut Vec<u8>) {
    dst.clear();
    dst.reserve(src.len() * 3); // Upper bound for BMP UTF-8 expansion

    let len = src.len();
    let mut i = 0;

    #[cfg(target_arch = "x86_64")]
    {
        #[target_feature(enable = "sse2")]
        unsafe fn transcode_ascii_sse2(src: &[u16], dst: &mut Vec<u8>, idx: &mut usize) {
            use core::arch::x86_64::*;
            let len = src.len();
            while *idx + 8 <= len {
                let chunk = _mm_loadu_si128(src.as_ptr().add(*idx) as *const __m128i);
                // Check if all upper bytes are 0 and code units < 0x80 (ASCII)
                let high_mask = _mm_cmpgt_epi16(chunk, _mm_set1_epi16(0x007F));
                let low_mask = _mm_cmpgt_epi16(_mm_setzero_si128(), chunk);
                let invalid = _mm_or_si128(high_mask, low_mask);
                if _mm_movemask_epi8(invalid) != 0 {
                    break;
                }
                // Pack 8 16-bit integers into 8 8-bit unsigned integers
                let packed = _mm_packus_epi16(chunk, chunk);
                let val = _mm_cvtsi128_si64(packed);
                dst.extend_from_slice(&val.to_ne_bytes());
                *idx += 8;
            }
        }

        if is_x86_feature_detected!("sse2") {
            // SAFETY: feature detected at runtime.
            unsafe { transcode_ascii_sse2(src, dst, &mut i) };
        }
    }

    // Scalar fallback for remaining elements or non-ASCII characters
    while i < len {
        let u = src[i];
        if u < 0x80 {
            dst.push(u as u8);
            i += 1;
        } else {
            // Handle multi-byte UTF-16 surrogates or multi-byte UTF-8
            for c in char::decode_utf16(src[i..].iter().copied()) {
                match c {
                    Ok(ch) => {
                        let mut buf = [0u8; 4];
                        dst.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    Err(_) => {
                        dst.push(b'?'); // Lossy fallback matching to_string_lossy
                    }
                }
            }
            break;
        }
    }
}
```

Add unit tests in `src/simd.rs` verifying:
- ASCII filenames (`"file.txt"`, `"MyDirectory_123"`) transcode identically to standard `String::from_utf16_lossy`.
- Unicode filenames (Persian, CJK, Emoji) transcode identically without crashing.
- `is_dot_or_dotdot_utf16` identifies `.` and `..` correctly and rejects `...` or `.git`.

**Verify**: `cargo test test_transcode` → passes; `cargo check --target x86_64-pc-windows-gnu` → exit 0.

---

### Step 3: Integrate `VirtualAlloc` with `MEM_LARGE_PAGES` into `AlignedBuffer`
Update `AlignedBuffer` in `src/scanner.rs` so that Windows allocates buffers directly from the Windows Virtual Memory Manager using `VirtualAlloc`, attempting 2 MiB large pages first before falling back to 64 KiB system-aligned virtual pages.

Target changes in `src/scanner.rs`:
```rust
enum BufferSource {
    #[cfg(target_os = "linux")]
    HugePage { ptr: *mut u8, size: usize },
    #[cfg(target_os = "linux")]
    PrePopulatedMmap { ptr: *mut u8, size: usize },
    #[cfg(windows)]
    VirtualAlloc { ptr: *mut u8, size: usize, is_large_page: bool },
    HeapLayout(std::alloc::Layout),
}
```

In `AlignedBuffer::new(size: usize, align: usize)`:
```rust
#[cfg(windows)]
{
    use crate::sys::windows::*;
    // Attempt 2 MiB Large Page allocation if size is suitable
    let large_page_min = unsafe { GetLargePageMinimum() };
    if large_page_min > 0 && size >= large_page_min && (size % large_page_min == 0) {
        let ptr = unsafe {
            VirtualAlloc(
                std::ptr::null_mut(),
                size,
                MEM_COMMIT | MEM_RESERVE | MEM_LARGE_PAGES,
                PAGE_READWRITE,
            )
        };
        if !ptr.is_null() {
            return AlignedBuffer {
                ptr: ptr as *mut u8,
                source: BufferSource::VirtualAlloc {
                    ptr: ptr as *mut u8,
                    size,
                    is_large_page: true,
                },
                len: size,
            };
        }
    }

    // Standard VirtualAlloc (page-aligned, avoids CRT heap contention)
    let ptr = unsafe {
        VirtualAlloc(
            std::ptr::null_mut(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if !ptr.is_null() {
        return AlignedBuffer {
            ptr: ptr as *mut u8,
            source: BufferSource::VirtualAlloc {
                ptr: ptr as *mut u8,
                size,
                is_large_page: false,
            },
            len: size,
        };
    }
}
```

In `Drop for AlignedBuffer`:
```rust
#[cfg(windows)]
BufferSource::VirtualAlloc { ptr, .. } => {
    // SAFETY: ptr was allocated by VirtualAlloc with MEM_COMMIT | MEM_RESERVE.
    unsafe {
        crate::sys::windows::VirtualFree(ptr as *mut std::ffi::c_void, 0, crate::sys::windows::MEM_RELEASE);
    }
}
```

**Verify**: `cargo check --target x86_64-pc-windows-gnu` → exit 0; `cargo test test_aligned_buffer` → passes.

---

### Step 4: Implement `WidePathStack` and Zero-Allocation Path Stack
On Windows, traversing a directory tree without `PathBuf::join` requires two synchronized path stacks:
1. `WidePathStack` (`Vec<u16>`): Holds the null-terminated UTF-16 path needed by `CreateFileW`.
2. `PathStack` (`Vec<u8>`): Holds the UTF-8 path bytes needed by `FastExclusionMatcher` and `DirArena`.

Create helper in `src/scanner.rs`:
```rust
#[cfg(windows)]
pub struct WidePathStack {
    wide: Vec<u16>,
}

#[cfg(windows)]
impl WidePathStack {
    pub fn new() -> Self {
        Self {
            wide: Vec::with_capacity(1024),
        }
    }

    pub fn set_root(&mut self, path: &Path) {
        use std::os::windows::ffi::OsStrExt;
        self.wide.clear();
        self.wide.extend(path.as_os_str().encode_wide());
        // Strip trailing slash if present
        while self.wide.last() == Some(&(b'\\' as u16)) || self.wide.last() == Some(&(b'/' as u16)) {
            self.wide.pop();
        }
    }

    /// Push child directory wide characters. Returns the previous length to pop.
    #[inline(always)]
    pub fn push_child(&mut self, child_name_wide: &[u16]) -> usize {
        let prev_len = self.wide.len();
        self.wide.push(b'\\' as u16);
        self.wide.extend_from_slice(child_name_wide);
        prev_len
    }

    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        self.wide.truncate(len);
    }

    /// Returns a null-terminated pointer to pass directly to CreateFileW without allocation.
    #[inline(always)]
    pub fn as_null_terminated(&mut self) -> *const u16 {
        self.wide.push(0);
        let ptr = self.wide.as_ptr();
        self.wide.pop();
        ptr
    }

    pub fn to_path_buf(&self) -> PathBuf {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        PathBuf::from(OsString::from_wide(&self.wide))
    }
}
```

Open child directories via:
```rust
#[inline(always)]
pub unsafe fn open_dir_from_wide_ptr(ptr: *const u16) -> Option<RawHandle> {
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
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        None
    } else {
        Some(handle)
    }
}
```

**Verify**: `cargo check --target x86_64-pc-windows-gnu` → exit 0, no errors.

---

### Step 5: Overhaul `scan_directory_tree_windows` with Zero-Allocation SIMD Pipeline
Replace the current allocation-heavy `scan_directory_tree_windows` in `src/scanner.rs` with the zero-allocation architecture:

1. Use `wide_path_stack` for `CreateFileW` calls and `path_stack` for exclusion checks.
2. In the `FILE_ID_BOTH_DIR_INFO` buffer loop:
   - Check `is_dot_or_dotdot_utf16` in 2 instructions.
   - Transcode `name_slice` to UTF-8 using `simd::transcode_utf16_to_utf8(&name_slice, &mut utf8_scratch)`.
   - Push `utf8_scratch` onto `path_stack` for `matcher.is_excluded(&path_stack, &utf8_scratch)`.
   - Check `FILE_ATTRIBUTE_DIRECTORY` and `FILE_ATTRIBUTE_REPARSE_POINT` (0x400) directly from `entry.file_attributes`.
   - For files: read `entry.allocation_size` (or `entry.end_of_file`), add to `local_dir_size`, update `record_file_stat`, and push to `local_top_files.push(sz, current_node, &utf8_scratch, local_arena)` — **zero `PathBuf` or `String` allocations!**
   - For subdirectories: store relative wide names in `sub_dirs_wide: Vec<Vec<u16>>` or slice markers.
3. For idle workers: only construct `PathBuf` when stealing half of `sub_dirs_wide` to push to `worker.push()`. For the local worker, continue recursion using `wide_path_stack.push_child()`, maintaining zero heap allocations.
4. Swap buffers with `dual_buffer.swap()` between directory queries.

Target skeleton for `scan_directory_tree_windows`:
```rust
#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn scan_directory_tree_windows(
    current_node: u32,
    rel_depth: usize,
    state: &Arc<GlobalState>,
    worker: &Worker<PathBuf>,
    dual_buffer: &mut DualBuffer,
    wide_path_stack: &mut WidePathStack,
    path_stack: &mut Vec<u8>,
    utf8_scratch: &mut Vec<u8>,
    local_arena: &mut DirArena,
    local_top_files: &mut LocalTopFiles,
    local_files: &mut u64,
    local_bytes: &mut u64,
) {
    use crate::sys::windows::*;
    use crate::simd::{is_dot_or_dotdot_utf16, transcode_utf16_to_utf8};

    let mut local_dir_size: u64 = 0;
    let mut sub_dirs_wide: Vec<Vec<u16>> = Vec::new();

    let root_dev = state.config.root_dev;
    let matcher = &state.config.matcher;

    let wide_ptr = wide_path_stack.as_null_terminated();
    if let Some(h_dir) = unsafe { open_dir_from_wide_ptr(wide_ptr) } {
        if !state.config.cross_filesystems
            && let Some(vol) = (unsafe { get_volume_serial_number(h_dir) })
            && vol != root_dev
        {
            unsafe { close_handle(h_dir) };
            return;
        }

        let buf_slice = dual_buffer.current_mut().as_mut_slice();
        let buf_ptr = buf_slice.as_mut_ptr() as *mut std::ffi::c_void;
        let buf_len = buf_slice.len() as u32;
        let mut restart_scan = true;

        loop {
            let (status, info_bytes) =
                unsafe { sys_nt_query_directory_file_fast(h_dir, buf_ptr, buf_len, restart_scan) };

            if status != STATUS_SUCCESS || info_bytes == 0 {
                break;
            }
            restart_scan = false;

            let mut offset = 0usize;
            loop {
                if offset + std::mem::size_of::<FileIdBothDirInfo>() > buf_slice.len() {
                    break;
                }

                let entry_ptr =
                    unsafe { (buf_ptr as *const u8).add(offset) as *const FileIdBothDirInfo };
                let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };

                let name_len_bytes = entry.file_name_length as usize;
                let name_len_wchars = name_len_bytes / std::mem::size_of::<u16>();

                let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);
                if offset + fn_offset + name_len_bytes > buf_slice.len() {
                    break;
                }

                let file_name_ptr =
                    unsafe { (entry_ptr as *const u8).add(fn_offset) as *const u16 };
                let name_slice =
                    unsafe { std::slice::from_raw_parts(file_name_ptr, name_len_wchars) };

                if !is_dot_or_dotdot_utf16(name_slice) {
                    transcode_utf16_to_utf8(name_slice, utf8_scratch);

                    let orig_path_len = path_stack.len();
                    path_stack.push(b'\\');
                    path_stack.extend_from_slice(utf8_scratch);

                    let excluded = matcher.is_excluded(path_stack.as_slice(), utf8_scratch.as_slice());
                    path_stack.truncate(orig_path_len);

                    if !excluded {
                        let attrs = entry.file_attributes;
                        let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
                        let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                        if is_dir {
                            if !is_reparse || state.config.follow_symlinks {
                                sub_dirs_wide.push(name_slice.to_vec());
                            }
                        } else {
                            let sz = if entry.allocation_size > 0 {
                                entry.allocation_size as u64
                            } else {
                                entry.end_of_file.max(0) as u64
                            };

                            local_dir_size += sz;
                            record_file_stat(local_files, local_bytes, sz, state);
                            local_top_files.push(
                                sz,
                                current_node,
                                utf8_scratch.as_slice(),
                                local_arena,
                            );
                        }
                    }
                }

                if entry.next_entry_offset == 0
                    || offset + (entry.next_entry_offset as usize) >= buf_slice.len()
                {
                    break;
                }
                offset += entry.next_entry_offset as usize;
            }
        }

        unsafe { close_handle(h_dir) };
    }

    local_arena.add_direct_bytes(current_node, local_dir_size);

    // Work-stealing: construct PathBuf ONLY when delegating to another worker
    if sub_dirs_wide.len() > 1 && state.active_workers.load(Ordering::Relaxed) < state.config.threads {
        let half = sub_dirs_wide.split_off(sub_dirs_wide.len() / 2);
        for sub_w in half {
            let prev_len = wide_path_stack.push_child(&sub_w);
            let full_path = wide_path_stack.to_path_buf();
            wide_path_stack.truncate(prev_len);
            worker.push(full_path);
        }
        state.cvar.notify_all();
    }

    // Local recursion: push/pop onto wide_path_stack and path_stack with zero allocations
    for sub_w in sub_dirs_wide {
        transcode_utf16_to_utf8(&sub_w, utf8_scratch);
        let (child_node, next_rel_depth) = if rel_depth < state.config.max_depth {
            let next_d = rel_depth + 1;
            let node = local_arena.add_node(current_node, next_d as u16, utf8_scratch.as_slice());
            (node, next_d)
        } else {
            (current_node, rel_depth + 1)
        };

        let orig_wide_len = wide_path_stack.push_child(&sub_w);
        let orig_path_len = path_stack.len();
        path_stack.push(b'\\');
        path_stack.extend_from_slice(utf8_scratch);

        dual_buffer.swap();
        scan_directory_tree_windows(
            child_node,
            next_rel_depth,
            state,
            worker,
            dual_buffer,
            wide_path_stack,
            path_stack,
            utf8_scratch,
            local_arena,
            local_top_files,
            local_files,
            local_bytes,
        );
        dual_buffer.swap();

        path_stack.truncate(orig_path_len);
        wide_path_stack.truncate(orig_wide_len);
    }
}
```

Update `worker_loop` in `src/scanner.rs` to initialize `WidePathStack`, `path_stack`, and `utf8_scratch` once per worker thread, passing them into `scan_directory_tree_windows`.

**Verify**: `cargo check --target x86_64-pc-windows-gnu` → exit 0, no warnings.

---

### Step 6: Add Cross-Platform Verification Tests in `tests/scanner_integration.rs`
Add regression tests testing synthetic tree traversal and path construction logic under cross-platform conditions:
1. `test_windows_wide_path_stack_push_pop_integrity`: Verifies that `WidePathStack` accurately manages deep nested paths (100 levels) without buffer overflows or delimiter corruption.
2. `test_utf16_transcoding_exhaustive`: Verifies round-trip accuracy across ASCII, boundary surrogate pairs, and null characters.
3. `test_synthetic_tree_rollup_cross_platform`: Existing test continues to pass and confirms size equivalence.

**Verify**: `cargo test` → all unit and integration tests pass.

---

## Test plan
- **Unit Tests**:
  - `src/simd.rs`: `test_transcode_utf16_to_utf8_ascii`, `test_transcode_utf16_to_utf8_unicode`, `test_is_dot_or_dotdot_utf16`.
  - `src/scanner.rs`: `test_aligned_buffer_virtual_alloc_windows` (under `#[cfg(windows)]`).
- **Integration Tests**:
  - `tests/scanner_integration.rs`: `test_windows_wide_path_stack_push_pop_integrity`.
- **Target Compilation**:
  - `cargo check --target x86_64-pc-windows-gnu` verifies ABI and extern signatures against MinGW / Windows headers.
  - `cargo clippy --all-targets -- -D warnings` ensures zero linter regressions.
  - `cargo fmt --check` ensures formatting consistency.

## Done criteria
- [ ] `cargo check --target x86_64-pc-windows-gnu` exits 0 with zero warnings or errors.
- [ ] `cargo test` exits 0 with all tests passing on host.
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0.
- [ ] `cargo fmt --check` exits 0.
- [ ] `scan_directory_tree_windows` performs zero `PathBuf` or `String` allocations in hot file enumeration.
- [ ] `plans/README.md` status table updated with Plan 017.

## STOP conditions
Stop and report back (do not improvise) if:
- Code at `src/sys/windows.rs` or `src/scanner.rs` has drifted such that `FileIdBothDirInfo` structure layout mismatches.
- `cargo check --target x86_64-pc-windows-gnu` reports undefined symbols in `ntdll` or `kernel32`.
- Adding SIMD transcoding introduces an external crate dependency (must remain 100% pure std and core).

## Maintenance notes
- Windows NT internal syscall numbers can change across major kernel builds; using dynamic lookup via `GetProcAddress(ntdll, "NtQueryDirectoryFileEx")` ensures complete ABI stability across Windows 10, Windows 11, and Windows Server 2016–2025.
- `AllocationSize` reflects physical cluster allocation on NTFS and ReFS. For zero-byte files, `AllocationSize` is 0. If files are resident inside the MFT record (under ~700 bytes), `AllocationSize` may be reported as 0 by NTFS; in that case, fallback to `EndOfFile` accurately captures file size.
- If running under Wine or minimal Windows containers where large pages are disabled by hypervisor policy, `VirtualAlloc` gracefully falls back to standard 64 KiB system-aligned pages.
