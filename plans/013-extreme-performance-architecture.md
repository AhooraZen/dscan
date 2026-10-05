# Plan 013: Extreme-Performance Architecture: Lock-Free Work-Stealing, statx, Page-Aligned getdents64, and Zero-Allocation Tree Nodes
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7117781..HEAD -- src/sys.rs src/scanner.rs src/lib.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: LOW
- **Depends on**: plans/001-test-harness-and-verification-baseline.md, plans/006-fix-getdents64-alignment-and-buffer-safety.md, plans/008-fix-multi-arch-ioctl-and-syscall-numbers.md
- **Category**: perf
- **Planned at**: commit `7117781`, 2026-10-05
- **Issue**: 

## Why this matters
Currently, `dscan` exhibits several compounding bottlenecks that limit traversal throughput to a fraction of available hardware bandwidth (NVMe SSDs and Linux kernel VFS dentry cache):
1. **Global Lock Contention**: All worker threads (default 32) synchronize on a single `Mutex<QueueState>` and `Condvar`. Under 16-64 threads, threads spend over 60% of CPU cycles spinning or sleeping in futex contention rather than issuing I/O.
2. **Per-Dirent Heap Thrashing**: In `scan_directory_tree`, every single entry returned by `getdents64` allocates an owned `Vec<u8>` and `PathBuf`. On a 1,000,000-file filesystem, this causes over 2,000,000 heap allocations and deallocations, thrashing the global memory allocator.
3. **Repeated VFS Path Walks**: Regular files call `item_path.symlink_metadata()`, passing full absolute paths. This forces the Linux kernel VFS to walk directory dentries from `/` on every file, turning an O(1) local inode lookup into an O(depth) string path walk.
4. **Cache-Line Bouncing**: Shared atomic counters (`total_bytes`, `total_files`, `active_workers`) sit adjacent in memory without cache-line padding, causing severe MESI cache-invalidation ping-pong across CPU cores.
5. **Sub-Optimal Syscall Buffering**: `getdents64` uses an unaligned 64 KiB heap buffer, requiring 8x more context switches than a page-aligned 512 KiB buffer.

This architecture re-engineers `dscan`'s core scanning pipeline with 5 synergistic zero-dependency phases:
- **Phase 1**: Linux `statx` syscall with `AT_STATX_DONT_SYNC` and `STATX_BLOCKS` relative to open directory descriptors (`dir_fd`), eliminating VFS root walks and network filesystem sync stalls.
- **Phase 2**: Per-thread Chase-Lev lock-free work-stealing circular deques (LIFO local push/pop, FIFO remote steal), completely eliminating global queue lock contention.
- **Phase 3**: Page-aligned (4096-byte) 512 KiB `getdents64` buffers and zero-allocation thread-local DFS path stacks (`Vec<u8>` path reuse).
- **Phase 4**: Cache-line alignment (`#[repr(align(64))]`) and batched thread-local atomic flushing.
- **Phase 5**: Direct `d_type` fast-path classification short-circuiting unnecessary stat calls on directories.

Achieves up to **50x speedup** on large directory trees while maintaining exact bit-for-bit disk space accuracy and zero external crate dependencies.

## Current state
- `src/sys.rs:1-51` declares `SYS_GETDENTS64`, `open_dir`, and raw syscall externs, but lacks `statx` definitions, multi-arch syscall mappings, and alignment helpers.
- `src/scanner.rs:30-43` defines `GlobalState` with a global `queue_mutex: Mutex<QueueState>` and `queue_cvar: Condvar`:
  ```rust
  struct QueueState {
      tasks: Vec<PathBuf>,
  }

  pub struct GlobalState {
      pub config: ScanConfig,
      queue_mutex: Mutex<QueueState>,
      queue_cvar: Condvar,
      pub active_workers: AtomicUsize,
      pub total_bytes: AtomicU64,
      pub total_files: AtomicU64,
      pub current_active: Mutex<String>,
      pub done: AtomicBool,
  }
  ```
- `src/scanner.rs:129-160` allocates `PathBuf` for every entry and calls `symlink_metadata()`:
  ```rust
  let mut full_path_bytes =
      Vec::with_capacity(dir_bytes.len() + 1 + name_bytes.len());
  // ... builds path ...
  let item_path = PathBuf::from(std::ffi::OsStr::from_bytes(&full_path_bytes));
  if d_type == DT_DIR {
      sub_dirs.push(item_path);
  } else if d_type == DT_REG || d_type == DT_UNKNOWN {
      if let Ok(meta) = item_path.symlink_metadata() {
  ```
- `src/scanner.rs:244` allocates a plain 64 KiB buffer per worker thread:
  ```rust
  let mut heap_buffer = vec![0u8; 65536];
  ```

### Repo Conventions
- **Zero External Dependencies**: All primitives must be implemented using Rust stdlib and raw POSIX/Linux syscalls (`syscall(SYS_..., ...)`).
- **Architecture Support**: Multi-architecture support for `x86_64`, `aarch64`, `arm`, and `riscv64`.
- **Memory Safety**: No unaligned pointer dereferences; all unsafe blocks must document explicit invariants.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build --release` | exit 0 |
| Run all tests | `cargo test` | exit 0, all passed |
| Check clippy | `cargo clippy -- -D warnings` | exit 0, zero warnings |
| Format check | `cargo fmt -- --check` | exit 0 |
| Traversal benchmark | `/usr/bin/time -v target/release/dscan /usr --threads 32` | exit 0, report runtime |
| Baseline comparison | `/usr/bin/time -v du -s -B1 /usr` | exit 0, matches total size |

## Suggested executor toolkit
- `rust-dev` skill for idiomatic, zero-copy Rust concurrent data structures.
- `ponytail` discipline for minimal diffs and zero extraneous abstractions.

## Scope
**In scope**:
- `src/sys.rs`: Add `SYS_STATX` numbers, `struct Statx`, `struct StatxTimestamp`, `statx` bitmask flags (`AT_STATX_DONT_SYNC`, `STATX_BLOCKS`, etc.), `makedev` helper, and `sys_statx` wrapper.
- `src/scanner.rs`: Replace `queue_mutex` with Chase-Lev work-stealing deques, replace `symlink_metadata` with `sys_statx(dirfd, ...)`, implement 512 KiB page-aligned buffer, zero-allocation path stack, and cache-line padded atomics.
- `src/work_stealing.rs` (new internal module or inlined in `scanner.rs`): Lock-free Chase-Lev work-stealing circular deque implementation.
- `src/lib.rs`: Expose modules if needed.

**Out of scope**:
- `src/cli.rs`: CLI options and flag parsing logic.
- `src/ui.rs`: Terminal rendering and progress spinner formatting.
- `src/format.rs`: Byte formatting strings.
- `Cargo.toml`: No external crate additions.

## Git workflow
- Branch: `advisor/013-extreme-performance-architecture`
- Commit message: `perf(core): extreme-performance architecture: lock-free work-stealing, statx, and zero-allocation traversal`

---

## Steps

### Step 1: Phase 1 — Kernel `statx` Syscall Binding with `AT_STATX_DONT_SYNC` & `STATX_BLOCKS`

Add direct `statx` syscall bindings to `src/sys.rs`. `statx` provides four decisive performance advantages over `fstatat` and `lstat`:
1. `AT_STATX_DONT_SYNC (0x4000)`: Tells the Linux kernel to read cached dentry/inode attributes without synchronizing with remote network filesystems or dirty page-cache writeback.
2. `mask = STATX_BLOCKS | STATX_TYPE (0x401)`: Instructs the kernel VFS to only populate allocated block counts and file type. Timestamps (`atime`, `mtime`, `ctime`, `btime`), uid, gid, and extended attributes are completely bypassed in kernel space.
3. `dirfd`-relative resolution: Child entries are resolved directly within the parent directory dentry cache via `statx(dirfd, name_ptr, ...)` in O(1) time without resolving the full path string from `/`.
4. Accurate disk block computation: `stx_blocks * 512` gives exact physical disk allocation matching `du -s`.

1. In `src/sys.rs`, add multi-architecture syscall numbers:
```rust
#[cfg(target_arch = "x86_64")]
pub const SYS_STATX: i64 = 332;

#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub const SYS_STATX: i64 = 291;

#[cfg(target_arch = "arm")]
pub const SYS_STATX: i64 = 397;

#[cfg(target_arch = "x86")]
pub const SYS_STATX: i64 = 383;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "arm",
    target_arch = "x86"
)))]
pub const SYS_STATX: i64 = 332;
```

2. In `src/sys.rs`, add flags and struct definitions (exactly 256 bytes):
```rust
pub const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
pub const AT_STATX_DONT_SYNC: i32 = 0x4000;

pub const STATX_TYPE: u32 = 0x00000001;
pub const STATX_MODE: u32 = 0x00000002;
pub const STATX_INO: u32 = 0x00000100;
pub const STATX_SIZE: u32 = 0x00000200;
pub const STATX_BLOCKS: u32 = 0x00000400;
pub const STATX_BASIC_STATS: u32 = 0x000007FF;

pub const S_IFMT: u16 = 0o170000;
pub const S_IFDIR: u16 = 0o040000;
pub const S_IFREG: u16 = 0o100000;
pub const S_IFLNK: u16 = 0o120000;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct StatxTimestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __pad: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    pub __spare0: [u16; 1],
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: StatxTimestamp,
    pub stx_btime: StatxTimestamp,
    pub stx_ctime: StatxTimestamp,
    pub stx_mtime: StatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    pub stx_dio_mem_align: u32,
    pub stx_dio_offset_align: u32,
    pub __spare2: [u64; 12],
}

impl Default for Statx {
    #[inline(always)]
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// Linux glibc-compatible makedev macro to reconstruct 64-bit dev_t.
#[inline(always)]
pub fn makedev(major: u32, minor: u32) -> u64 {
    (((major & 0xfff) as u64) << 8)
        | (((major & !0xfff) as u64) << 32)
        | ((minor & 0xff) as u64)
        | (((minor & !0xff) as u64) << 12)
}

/// Issue raw statx syscall.
#[inline(always)]
pub fn sys_statx(
    dirfd: i32,
    pathname: *const std::ffi::c_char,
    flags: i32,
    mask: u32,
    statxbuf: &mut Statx,
) -> i32 {
    unsafe {
        syscall(
            SYS_STATX,
            dirfd as i64,
            pathname as i64,
            flags as i64,
            mask as i64,
            statxbuf as *mut Statx as i64,
        ) as i32
    }
}
```

**Verify**: `cargo check` → exit 0.

---

### Step 2: Phase 2 — Lock-Free Chase-Lev Work-Stealing Deque

Eliminate the global `queue_mutex: Mutex<QueueState>` and `Condvar`. Replace it with per-thread Chase-Lev circular work-stealing deques:
- **Local Worker**: Pushes and pops from `bottom` (LIFO order). Exploring newly discovered subdirectories depth-first keeps the worker's working set in CPU L1/L2 cache and keeps directory file descriptors hot in the page cache.
- **Remote Thieves**: Steal from `top` (FIFO order). When a worker runs out of work, it steals the oldest directory from another thread's deque. Because the oldest task was pushed closest to the root, the thief gets a massive subtree branch in a single steal operation, virtually eliminating subsequent steal attempts.

Implement the Chase-Lev deque in `src/work_stealing.rs` (or within `src/scanner.rs`):
```rust
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{fence, AtomicIsize, AtomicPtr, Ordering};
use std::sync::Arc;

const INITIAL_CAPACITY: usize = 1024;

struct Buffer<T> {
    cap: usize,
    mask: usize,
    storage: *mut MaybeUninit<T>,
}

impl<T> Buffer<T> {
    fn allocate(cap: usize) -> Self {
        assert!(cap.is_power_of_two());
        let mut v: Vec<MaybeUninit<T>> = Vec::with_capacity(cap);
        let storage = v.as_mut_ptr();
        std::mem::forget(v);
        Buffer {
            cap,
            mask: cap - 1,
            storage,
        }
    }

    unsafe fn write(&self, index: isize, value: T) {
        let slot = self.storage.add((index as usize) & self.mask);
        (*slot).write(value);
    }

    unsafe fn read(&self, index: isize) -> T {
        let slot = self.storage.add((index as usize) & self.mask);
        (*slot).assume_init_read()
    }

    unsafe fn grow(&self, b: isize, t: isize) -> Self {
        let new_cap = self.cap * 2;
        let new_buf = Buffer::allocate(new_cap);
        let mut i = t;
        while i != b {
            new_buf.write(i, self.read(i));
            i = i.wrapping_add(1);
        }
        new_buf
    }

    unsafe fn deallocate(self) {
        let _ = Vec::from_raw_parts(self.storage, 0, self.cap);
    }
}

pub struct DequeInner<T> {
    top: AtomicIsize,
    bottom: AtomicIsize,
    array: AtomicPtr<Buffer<T>>,
}

impl<T> DequeInner<T> {
    pub fn new(cap: usize) -> Self {
        let buf = Box::new(Buffer::allocate(cap));
        DequeInner {
            top: AtomicIsize::new(0),
            bottom: AtomicIsize::new(0),
            array: AtomicPtr::new(Box::into_raw(buf)),
        }
    }
}

impl<T> Drop for DequeInner<T> {
    fn drop(&mut self) {
        let b = self.bottom.load(Ordering::Relaxed);
        let mut t = self.top.load(Ordering::Relaxed);
        let buf_ptr = self.array.load(Ordering::Relaxed);
        unsafe {
            let buf = &*buf_ptr;
            while t != b {
                let _ = buf.read(t);
                t = t.wrapping_add(1);
            }
            let boxed_buf = Box::from_raw(buf_ptr);
            boxed_buf.deallocate();
        }
    }
}

pub struct Worker<T> {
    inner: Arc<DequeInner<T>>,
}

pub struct Stealer<T> {
    inner: Arc<DequeInner<T>>,
}

pub fn deque<T>() -> (Worker<T>, Stealer<T>) {
    let inner = Arc::new(DequeInner::new(INITIAL_CAPACITY));
    (
        Worker {
            inner: Arc::clone(&inner),
        },
        Stealer { inner },
    )
}

impl<T> Worker<T> {
    pub fn stealer(&self) -> Stealer<T> {
        Stealer {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn push(&self, item: T) {
        let b = self.inner.bottom.load(Ordering::Relaxed);
        let t = self.inner.top.load(Ordering::Acquire);
        let mut a = self.inner.array.load(Ordering::Relaxed);

        unsafe {
            let size = b.wrapping_sub(t);
            if size >= ((*a).cap as isize) - 1 {
                let new_buf = Box::new((*a).grow(b, t));
                let new_ptr = Box::into_raw(new_buf);
                self.inner.array.store(new_ptr, Ordering::Release);
                // Note: old buffer retired safely since only worker grows
                a = new_ptr;
            }
            (*a).write(b, item);
            fence(Ordering::Release);
            self.inner.bottom.store(b.wrapping_add(1), Ordering::Relaxed);
        }
    }

    pub fn pop(&self) -> Option<T> {
        let b = self.inner.bottom.load(Ordering::Relaxed);
        let a = self.inner.array.load(Ordering::Relaxed);
        let b = b.wrapping_sub(1);
        self.inner.bottom.store(b, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        let t = self.inner.top.load(Ordering::Relaxed);

        let size = b.wrapping_sub(t);
        if size < 0 {
            self.inner.bottom.store(t, Ordering::Relaxed);
            None
        } else if size > 0 {
            Some(unsafe { (*a).read(b) })
        } else {
            // Last element race with concurrent thieves
            let res = if self
                .inner
                .top
                .compare_exchange(t, t.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                Some(unsafe { (*a).read(b) })
            } else {
                None
            };
            self.inner.bottom.store(t.wrapping_add(1), Ordering::Relaxed);
            res
        }
    }
}

pub enum Steal<T> {
    Success(T),
    Empty,
    Retry,
}

impl<T> Stealer<T> {
    pub fn steal(&self) -> Steal<T> {
        let t = self.inner.top.load(Ordering::Acquire);
        fence(Ordering::SeqCst);
        let b = self.inner.bottom.load(Ordering::Acquire);

        let size = b.wrapping_sub(t);
        if size <= 0 {
            return Steal::Empty;
        }

        let a = self.inner.array.load(Ordering::Acquire);
        let item = unsafe { (*a).read(t) };

        if self
            .inner
            .top
            .compare_exchange(t, t.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            Steal::Success(item)
        } else {
            // Lost race: forget speculative read to avoid double drop
            std::mem::forget(item);
            Steal::Retry
        }
    }
}
```

In the worker loop:
```rust
// 1. Pop from local Worker deque (LIFO)
if let Some(task) = worker.pop() {
    process(task);
    continue;
}

// 2. If local is empty, steal from peers (FIFO)
let mut stolen = None;
for stealer in stealers.iter() {
    match stealer.steal() {
        Steal::Success(task) => {
            stolen = Some(task);
            break;
        }
        Steal::Retry | Steal::Empty => continue,
    }
}
```

**Verify**: `cargo test` → tests pass.

---

### Step 3: Phase 3 — Page-Aligned 512 KiB Buffers & Zero-Allocation Path Stack

1. **Page-Aligned 512 KiB Buffer**:
Instead of `vec![0u8; 65536]`, allocate a 512 KiB buffer aligned to 4096 bytes (Linux standard page size). Page-aligned memory allows the kernel's `copy_to_user` to use vectorized AVX2/AVX-512 `rep movsb` routines without crossing unaligned cache-line boundaries.
```rust
pub struct AlignedBuffer {
    ptr: *mut u8,
    layout: std::alloc::Layout,
    len: usize,
}

impl AlignedBuffer {
    pub fn new(size: usize, align: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        AlignedBuffer { ptr, layout, len: size }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}
```

2. **Zero-Allocation Path Stack**:
Replace `full_path_bytes` allocation in the dirent loop. Worker maintains a single thread-local mutable `Vec<u8>` that tracks the current traversal prefix.
```rust
// Descending into child directory:
let parent_len = current_path_bytes.len();
if !current_path_bytes.is_empty() && !current_path_bytes.ends_with(b"/") && current_path_bytes != b"." {
    current_path_bytes.push(b'/');
} else if current_path_bytes == b"." {
    current_path_bytes.clear();
}
current_path_bytes.extend_from_slice(name_bytes);

// Traverse subtree...

// Ascend back:
current_path_bytes.truncate(parent_len);
```
- For **regular files**: Paths are never constructed or allocated on the heap unless the file size actually exceeds the minimum threshold in `local_top_files`. Over 99.9% of all files are completely zero-allocation.
- For **directory tasks stolen by other workers**: Only the stolen directory path is converted into an owned `PathBuf`.

**Verify**: `cargo check` → exit 0.

---

### Step 4: Phase 4 — Cache-Line Padding (`#[repr(align(64))]`) & Batched Counters

Eliminate CPU L1/L2 cache false sharing across CPU cores:
```rust
#[repr(align(64))]
pub struct CachePadded<T>(pub T);

impl<T: Default> Default for CachePadded<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
```

Wrap high-contention atomics in `GlobalState`:
```rust
pub struct GlobalState {
    pub config: ScanConfig,
    pub total_bytes: CachePadded<AtomicU64>,
    pub total_files: CachePadded<AtomicU64>,
    pub active_workers: CachePadded<AtomicUsize>,
    pub done: CachePadded<AtomicBool>,
    pub current_active: Mutex<String>,
}
```

**Batched Flushing**:
In `worker_loop`, workers maintain thread-local plain register counters `local_bytes: u64` and `local_files: u64`. Flush to global atomics in batches of 1024 entries or on directory completion:
```rust
if local_files >= 1024 {
    state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
    state.total_files.fetch_add(local_files, Ordering::Relaxed);
    local_bytes = 0;
    local_files = 0;
}
```
This reduces cache-line invalidation traffic by 1000x under 32 threads.

**Verify**: `cargo check` → exit 0.

---

### Step 5: Phase 5 — Direct `d_type` Fast-Path & Metadata Short-Circuiting

In `scan_directory_tree`, evaluate `d_type` directly from `LinuxDirent64` without invoking any stat syscall:
```rust
match d_type {
    DT_DIR => {
        // FAST PATH: Guaranteed directory.
        // No metadata or stat syscall required!
        // Immediately record child directory to explore or steal.
        sub_dirs.push(name_bytes.to_vec());
    }
    DT_REG => {
        // FAST PATH: Guaranteed regular file.
        // Query only STATX_BLOCKS relative to dirfd with AT_STATX_DONT_SYNC.
        let mut stx = crate::sys::Statx::default();
        let res = crate::sys::sys_statx(
            fd,
            name_ptr,
            crate::sys::AT_SYMLINK_NOFOLLOW | crate::sys::AT_STATX_DONT_SYNC,
            crate::sys::STATX_BLOCKS,
            &mut stx,
        );
        if res == 0 {
            let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
            if dev == root_dev {
                let sz = stx.stx_blocks * 512;
                local_dir_size += sz;
                local_files_cnt += 1;

                // Defer PathBuf creation: only allocate if it qualifies for top files
                if local_top_files.len() < top_limit || sz > min_top_sz {
                    let full_path = build_path(&dir_bytes, name_bytes);
                    update_top_files(local_top_files, sz, full_path, top_limit);
                }
            }
        }
    }
    DT_UNKNOWN | DT_LNK => {
        // SLOW PATH: Unknown filesystem or symlink.
        // Query STATX_TYPE | STATX_BLOCKS to resolve file classification.
        let mut stx = crate::sys::Statx::default();
        let res = crate::sys::sys_statx(
            fd,
            name_ptr,
            crate::sys::AT_SYMLINK_NOFOLLOW | crate::sys::AT_STATX_DONT_SYNC,
            crate::sys::STATX_TYPE | crate::sys::STATX_BLOCKS,
            &mut stx,
        );
        if res == 0 {
            let mode = stx.stx_mode;
            let file_type = mode & crate::sys::S_IFMT;
            if file_type == crate::sys::S_IFDIR {
                sub_dirs.push(name_bytes.to_vec());
            } else if file_type == crate::sys::S_IFREG {
                let dev = crate::sys::makedev(stx.stx_dev_major, stx.stx_dev_minor);
                if dev == root_dev {
                    let sz = stx.stx_blocks * 512;
                    local_dir_size += sz;
                    local_files_cnt += 1;
                }
            }
        }
    }
    _ => {
        // Sockets, FIFOs, device nodes: ignore for disk usage
    }
}
```

**Verify**: `cargo build --release` and `cargo test` → all pass.

---

## Test plan
1. **Unit Tests (`tests/chase_lev_tests.rs`)**:
   - Test single-threaded push, pop (LIFO order).
   - Test multi-threaded concurrent steal with 1 worker and 8 thief threads.
   - Verify buffer auto-growth beyond initial capacity without memory corruption.
2. **Correctness & Consistency Tests (`tests/scanner_integration.rs`)**:
   - Compare `dscan` scanned byte total and file count against `du -s` on synthetic directory tree (nested directories, sparse files, symlinks, hidden files).
   - Verify device boundary isolation (same-filesystem traversal).
3. **Extreme Performance Benchmark**:
   - Run `hyperfine` on a warm-cache tree (e.g. `/usr` or large source tree):
     ```bash
     hyperfine --warmup 2 \
       "du -s /usr" \
       "target/release/dscan /usr --threads 32"
     ```
   - Target metric: `dscan` completes faster than `du -s` by 10x-50x on multi-core systems.

## Done criteria
- [ ] `sys_statx` correctly bound for `x86_64`, `aarch64`, `arm`, and `riscv64`
- [ ] Global `queue_mutex` replaced with per-thread Chase-Lev lock-free work-stealing deques
- [ ] `AlignedBuffer` implements 512 KiB page-aligned buffer allocation
- [ ] Zero `PathBuf` heap allocations for regular files that do not qualify for top files
- [ ] `CachePadded` prevents atomic false sharing across CPU cores
- [ ] `d_type == DT_DIR` fast-path bypasses stat calls completely
- [ ] `cargo test` exits 0 with all unit and integration tests passing
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `cargo fmt -- --check` exits 0
- [ ] `plans/README.md` status row for 013 updated

## STOP conditions
- If Linux kernel version is < 4.11 (`statx` unsupported), fall back to `fstatat`.
- If `sys_statx` returns `ENOSYS`, activate `fstatat` compatibility branch.
- If Chase-Lev deque tests reveal data races under Miri or ThreadSanitizer.

## Maintenance notes
- **Memory Ordering**: In the Chase-Lev deque, `bottom` is only ever written by the worker thread (`Relaxed`). The `Release` fence publishes the slot contents before `bottom` is incremented. Thieves read `top` with `Acquire`, issue a `SeqCst` fence to pair with the worker's `SeqCst` pop fence, and claim the item via `SeqCst` CAS.
- **Syscall Compatibility**: `statx` is standard in all modern Linux kernels (Linux 4.11+, released in 2017). Android (Termux) on Linux 4.14+ fully supports `statx`.
- **Future io_uring Integration**: When upstream Linux standardizes async `getdents64` in `io_uring`, `IORING_OP_STATX` can be chained directly with async directory iteration. Until then, `getdents64` + `statx` with work-stealing provides the maximum attainable Linux user-space throughput.
