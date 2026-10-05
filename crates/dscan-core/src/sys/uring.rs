#![cfg(target_os = "linux")]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::sys::linux::{
    MAP_FAILED, MAP_POPULATE, MAP_SHARED, PROT_READ, PROT_WRITE, Statx, close, mmap, munmap,
    syscall,
};

#[cfg(target_arch = "x86_64")]
pub const SYS_IO_URING_SETUP: i64 = 425;
#[cfg(target_arch = "x86_64")]
pub const SYS_IO_URING_ENTER: i64 = 426;

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "arm"
))]
pub const SYS_IO_URING_SETUP: i64 = 425;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "arm"
))]
pub const SYS_IO_URING_ENTER: i64 = 426;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "arm"
)))]
pub const SYS_IO_URING_SETUP: i64 = 425;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "arm"
)))]
pub const SYS_IO_URING_ENTER: i64 = 426;

pub const IORING_OP_STATX: u8 = 21;
pub const IORING_ENTER_GETEVENTS: u32 = 1 << 0;
pub const IORING_FEAT_SINGLE_MMAP: u32 = 1 << 0;

pub const IORING_OFF_SQ_RING: i64 = 0;
pub const IORING_OFF_CQ_RING: i64 = 0x8000000;
pub const IORING_OFF_SQES: i64 = 0x10000000;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct IoSqringOffsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub flags: u32,
    pub dropped: u32,
    pub array: u32,
    pub resv1: u32,
    pub user_addr: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct IoCqringOffsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub overflow: u32,
    pub cqes: u32,
    pub flags: u32,
    pub resv1: u32,
    pub user_addr: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct IoUringParams {
    pub sq_entries: u32,
    pub cq_entries: u32,
    pub flags: u32,
    pub sq_thread_cpu: u32,
    pub sq_thread_idle: u32,
    pub features: u32,
    pub wq_fd: u32,
    pub resv: [u32; 3],
    pub sq_off: IoSqringOffsets,
    pub cq_off: IoCqringOffsets,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IoUringSqe {
    pub opcode: u8,
    pub flags: u8,
    pub ioprio: u16,
    pub fd: i32,
    pub addr2: u64,
    pub addr: u64,
    pub len: u32,
    pub statx_flags: u32,
    pub user_data: u64,
    pub buf_index_or_group: u16,
    pub personality: u16,
    pub splice_fd_in: i32,
    pub addr3_or_pad: [u64; 2],
}

impl Default for IoUringSqe {
    #[inline(always)]
    fn default() -> Self {
        // SAFETY: IoUringSqe consists of primitive integer and array fields; zeroed representation is valid.
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct IoUringCqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

pub struct IoUringBatcher {
    ring_fd: i32,
    sq_ptr: *mut c_void,
    sq_ring_sz: usize,
    cq_ptr: *mut c_void,
    cq_ring_sz: usize,
    sqes_ptr: *mut IoUringSqe,
    sqes_sz: usize,
    sq_entries: u32,
    sq_ring_mask: u32,
    sq_tail: u32,
    sq_khead: *const AtomicU32,
    sq_ktail: *mut AtomicU32,
    sq_array: *mut u32,
    cq_ring_mask: u32,
    cq_khead: *mut AtomicU32,
    cq_ktail: *const AtomicU32,
    cqes: *const IoUringCqe,
}

// SAFETY: IoUringBatcher owns its file descriptor and mapped memory exclusively.
unsafe impl Send for IoUringBatcher {}

impl IoUringBatcher {
    /// Probe if the current Linux kernel supports io_uring and IORING_OP_STATX.
    pub fn probe_supported() -> bool {
        static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            let mut batcher = match IoUringBatcher::new(4) {
                Ok(b) => b,
                Err(_) => return false,
            };

            let mut stx = Statx::default();
            let dot_path = b".\0";
            // -100 is AT_FDCWD
            batcher.prep_statx(
                1,
                -100,
                dot_path.as_ptr() as *const std::ffi::c_char,
                0,
                crate::sys::STATX_TYPE,
                &mut stx,
            );

            if batcher.submit_and_wait(1).is_err() {
                return false;
            }

            let mut success = false;
            batcher.reap_completions(|_user_data, res| {
                if res == 0 {
                    success = true;
                }
            });
            success
        })
    }

    /// Create a new io_uring instance with given number of SQ entries.
    pub fn new(entries: u32) -> Result<Self, i32> {
        let mut params = IoUringParams::default();
        // SAFETY: SYS_IO_URING_SETUP invocation with valid params pointer.
        let fd = unsafe {
            syscall(
                SYS_IO_URING_SETUP,
                entries as i64,
                &mut params as *mut _ as i64,
            )
        } as i32;
        if fd < 0 {
            return Err(fd);
        }

        let sq_ring_sz_init = (params.sq_off.array as usize)
            + (params.sq_entries as usize) * std::mem::size_of::<u32>();
        let cq_ring_sz_init = (params.cq_off.cqes as usize)
            + (params.cq_entries as usize) * std::mem::size_of::<IoUringCqe>();

        let single_mmap = (params.features & IORING_FEAT_SINGLE_MMAP) != 0;
        let (sq_ring_sz, cq_ring_sz) = if single_mmap {
            let max_sz = sq_ring_sz_init.max(cq_ring_sz_init);
            (max_sz, max_sz)
        } else {
            (sq_ring_sz_init, cq_ring_sz_init)
        };

        // SAFETY: mmap SQ ring.
        let sq_ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                sq_ring_sz,
                PROT_READ | PROT_WRITE,
                MAP_SHARED | MAP_POPULATE,
                fd,
                IORING_OFF_SQ_RING,
            )
        };
        if sq_ptr == MAP_FAILED {
            // SAFETY: close ring fd on error.
            unsafe { close(fd) };
            return Err(-1);
        }

        let cq_ptr = if single_mmap {
            sq_ptr
        } else {
            // SAFETY: mmap CQ ring.
            let ptr = unsafe {
                mmap(
                    std::ptr::null_mut(),
                    cq_ring_sz,
                    PROT_READ | PROT_WRITE,
                    MAP_SHARED | MAP_POPULATE,
                    fd,
                    IORING_OFF_CQ_RING,
                )
            };
            if ptr == MAP_FAILED {
                // SAFETY: clean up previously mapped SQ ring and close fd.
                unsafe {
                    munmap(sq_ptr, sq_ring_sz);
                    close(fd);
                }
                return Err(-1);
            }
            ptr
        };

        let sqes_sz = (params.sq_entries as usize) * std::mem::size_of::<IoUringSqe>();
        // SAFETY: mmap SQE array.
        let sqes_ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                sqes_sz,
                PROT_READ | PROT_WRITE,
                MAP_SHARED | MAP_POPULATE,
                fd,
                IORING_OFF_SQES,
            )
        } as *mut IoUringSqe;

        if sqes_ptr as *mut c_void == MAP_FAILED {
            // SAFETY: clean up mapped memory and close fd.
            unsafe {
                if !single_mmap {
                    munmap(cq_ptr, cq_ring_sz);
                }
                munmap(sq_ptr, sq_ring_sz);
                close(fd);
            }
            return Err(-1);
        }

        // SAFETY: offsets provided by kernel params into mapped rings are within bounds.
        let (sq_khead, sq_ktail, sq_array, sq_ring_mask, cq_khead, cq_ktail, cqes, cq_ring_mask) = unsafe {
            let sq_khead =
                (sq_ptr as *const u8).add(params.sq_off.head as usize) as *const AtomicU32;
            let sq_ktail = (sq_ptr as *mut u8).add(params.sq_off.tail as usize) as *mut AtomicU32;
            let sq_array = (sq_ptr as *mut u8).add(params.sq_off.array as usize) as *mut u32;
            let sq_ring_mask =
                *((sq_ptr as *const u8).add(params.sq_off.ring_mask as usize) as *const u32);

            let cq_khead = (cq_ptr as *mut u8).add(params.cq_off.head as usize) as *mut AtomicU32;
            let cq_ktail =
                (cq_ptr as *const u8).add(params.cq_off.tail as usize) as *const AtomicU32;
            let cqes = (cq_ptr as *const u8).add(params.cq_off.cqes as usize) as *const IoUringCqe;
            let cq_ring_mask =
                *((cq_ptr as *const u8).add(params.cq_off.ring_mask as usize) as *const u32);

            (
                sq_khead,
                sq_ktail,
                sq_array,
                sq_ring_mask,
                cq_khead,
                cq_ktail,
                cqes,
                cq_ring_mask,
            )
        };

        Ok(Self {
            ring_fd: fd,
            sq_ptr,
            sq_ring_sz,
            cq_ptr,
            cq_ring_sz,
            sqes_ptr,
            sqes_sz,
            sq_entries: params.sq_entries,
            sq_ring_mask,
            sq_tail: 0,
            sq_khead,
            sq_ktail,
            sq_array,
            cq_ring_mask,
            cq_khead,
            cq_ktail,
            cqes,
        })
    }

    #[inline(always)]
    pub fn capacity(&self) -> u32 {
        self.sq_entries
    }

    #[inline(always)]
    pub fn pending(&self) -> u32 {
        // SAFETY: sq_khead is a valid mapped pointer.
        let head = unsafe { (*self.sq_khead).load(Ordering::Acquire) };
        self.sq_tail.wrapping_sub(head)
    }

    #[inline]
    pub fn prep_statx(
        &mut self,
        user_data: u64,
        dirfd: i32,
        path_ptr: *const std::ffi::c_char,
        flags: i32,
        mask: u32,
        stx_ptr: *mut Statx,
    ) {
        let index = (self.sq_tail & self.sq_ring_mask) as usize;
        // SAFETY: index is bounded by sq_ring_mask, which is < sq_entries.
        unsafe {
            let sqe = &mut *self.sqes_ptr.add(index);
            *sqe = IoUringSqe::default();
            sqe.opcode = IORING_OP_STATX;
            sqe.fd = dirfd;
            sqe.addr = path_ptr as u64;
            sqe.len = mask;
            sqe.addr2 = stx_ptr as u64;
            sqe.statx_flags = flags as u32;
            sqe.user_data = user_data;

            *self.sq_array.add(index) = index as u32;
        }
        self.sq_tail = self.sq_tail.wrapping_add(1);
    }

    pub fn submit_and_wait(&mut self, wait_nr: u32) -> Result<u32, i32> {
        // SAFETY: sq_ktail is valid mapped memory.
        unsafe {
            (*self.sq_ktail).store(self.sq_tail, Ordering::Release);
        }

        let to_submit = self.pending();
        let flags = if wait_nr > 0 {
            IORING_ENTER_GETEVENTS
        } else {
            0
        };

        // SAFETY: SYS_IO_URING_ENTER syscall.
        let res = unsafe {
            syscall(
                SYS_IO_URING_ENTER,
                self.ring_fd as i64,
                to_submit as i64,
                wait_nr as i64,
                flags as i64,
                std::ptr::null::<c_void>() as i64,
                0i64,
            )
        };
        if res < 0 {
            Err(res as i32)
        } else {
            Ok(res as u32)
        }
    }

    pub fn reap_completions<F: FnMut(u64, i32)>(&mut self, mut handler: F) {
        // SAFETY: cq_khead and cq_ktail are valid mapped memory.
        let mut head = unsafe { (*self.cq_khead).load(Ordering::Acquire) };
        let tail = unsafe { (*self.cq_ktail).load(Ordering::Acquire) };
        let mask = self.cq_ring_mask;

        while head != tail {
            let index = (head & mask) as usize;
            // SAFETY: index is bounded by cq_ring_mask.
            let cqe = unsafe { &*self.cqes.add(index) };
            handler(cqe.user_data, cqe.res);
            head = head.wrapping_add(1);
        }
        // SAFETY: store updated head to kernel ring.
        unsafe {
            (*self.cq_khead).store(head, Ordering::Release);
        }
    }
}

impl Drop for IoUringBatcher {
    fn drop(&mut self) {
        // SAFETY: unmap previously mapped regions and close ring file descriptor.
        unsafe {
            munmap(self.sqes_ptr as *mut c_void, self.sqes_sz);
            if self.cq_ptr != self.sq_ptr {
                munmap(self.cq_ptr, self.cq_ring_sz);
            }
            munmap(self.sq_ptr, self.sq_ring_sz);
            close(self.ring_fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_uring_struct_sizes() {
        assert_eq!(std::mem::size_of::<IoUringSqe>(), 64);
        assert_eq!(std::mem::size_of::<IoUringCqe>(), 16);
    }

    #[test]
    fn test_io_uring_probe_and_batch() {
        if !IoUringBatcher::probe_supported() {
            eprintln!("io_uring not supported on this kernel/container, skipping live test");
            return;
        }

        let mut batcher = IoUringBatcher::new(16).expect("create batcher");
        let mut stx = Statx::default();
        let dot = b".\0";
        batcher.prep_statx(
            42,
            -100,
            dot.as_ptr() as *const std::ffi::c_char,
            0,
            crate::sys::STATX_BASIC_STATS,
            &mut stx,
        );

        let submitted = batcher.submit_and_wait(1).expect("submit");
        assert!(submitted >= 1);

        let mut reaped = 0;
        batcher.reap_completions(|ud, res| {
            assert_eq!(ud, 42);
            assert_eq!(res, 0);
            reaped += 1;
        });
        assert_eq!(reaped, 1);
        assert!(stx.stx_blocks > 0 || stx.stx_size > 0);
    }
}
