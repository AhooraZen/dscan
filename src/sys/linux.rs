use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

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

#[cfg(target_arch = "x86_64")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64"
))]
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

pub const O_RDONLY: i32 = 0;
pub const O_DIRECTORY: i32 = 0o0200000;
pub const O_CLOEXEC: i32 = 0o2000000;
pub const O_NOATIME: i32 = 0o1000000;

pub const DT_UNKNOWN: u8 = 0;
pub const DT_DIR: u8 = 4;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;

pub const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
pub const AT_STATX_DONT_SYNC: i32 = 0x4000;
pub const AT_EMPTY_PATH: i32 = 0x1000;

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
pub struct LinuxDirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
}

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
        // SAFETY: Statx is a plain C struct with integer/array fields; zeroed representation is valid.
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
    // SAFETY: sys_statx invokes SYS_STATX with valid dirfd, null-terminated pathname or empty path with AT_EMPTY_PATH, and a valid pointer to Statx.
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

#[cfg(target_arch = "x86_64")]
pub const SYS_SCHED_SETAFFINITY: i64 = 203;

#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub const SYS_SCHED_SETAFFINITY: i64 = 122;

#[cfg(any(target_arch = "arm", target_arch = "x86"))]
pub const SYS_SCHED_SETAFFINITY: i64 = 241;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "arm",
    target_arch = "x86"
)))]
pub const SYS_SCHED_SETAFFINITY: i64 = 203;

pub const PROT_READ: i32 = 0x1;
pub const PROT_WRITE: i32 = 0x2;
pub const MAP_PRIVATE: i32 = 0x02;
pub const MAP_SHARED: i32 = 0x01;
pub const MAP_ANONYMOUS: i32 = 0x20;
pub const MAP_HUGETLB: i32 = 0x40000;
pub const MAP_POPULATE: i32 = 0x08000;
pub const MADV_HUGEPAGE: i32 = 14;
pub const MAP_FAILED: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;

unsafe extern "C" {
    pub fn syscall(number: i64, ...) -> i64;
    pub fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    pub fn close(fd: i32) -> i32;
    pub fn ioctl(fd: i32, request: u64, ...) -> i32;
    pub fn madvise(addr: *mut std::ffi::c_void, length: usize, advice: i32) -> i32;
    pub fn mmap(
        addr: *mut std::ffi::c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut std::ffi::c_void;
    pub fn munmap(addr: *mut std::ffi::c_void, length: usize) -> i32;
}

/// Pin current thread to physical CPU core.
pub fn pin_thread_to_core(core_id: usize) -> bool {
    let num_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);
    let target = core_id % num_cores;
    let mut mask = [0u64; 16]; // 1024-bit cpu_set
    let word = target / 64;
    let bit = target % 64;
    if word < mask.len() {
        mask[word] |= 1u64 << bit;
    }
    // SAFETY: SYS_SCHED_SETAFFINITY on current thread (tid = 0), mask size 128 bytes.
    let ret = unsafe { syscall(SYS_SCHED_SETAFFINITY, 0, 128, mask.as_ptr() as i64) };
    ret == 0
}

/// Open a directory with direct flags (O_DIRECTORY | O_CLOEXEC | O_NOATIME).
/// If open with O_NOATIME fails (e.g. unprivileged user), falls back to opening without O_NOATIME.
pub fn open_dir(path: &Path) -> Option<i32> {
    let path_c = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: path_c is a valid null-terminated C string.
    let fd = unsafe {
        open(
            path_c.as_ptr(),
            O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOATIME,
        )
    };
    if fd >= 0 {
        return Some(fd);
    }
    // Fallback without O_NOATIME for unprivileged scans
    // SAFETY: path_c is a valid null-terminated C string.
    let fd = unsafe { open(path_c.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC) };
    if fd >= 0 { Some(fd) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statx_struct_size() {
        assert_eq!(std::mem::size_of::<Statx>(), 256);
    }

    #[test]
    fn test_makedev() {
        let dev = makedev(8, 1);
        assert_eq!(dev, 0x801);
    }

    #[test]
    fn test_open_dir_and_statx() {
        let path = Path::new(".");
        let fd = open_dir(path).expect("open_dir failed");
        assert!(fd >= 0);

        let mut stx = Statx::default();
        let empty_path = b"\0";
        // AT_EMPTY_PATH is 0x1000
        let ret = sys_statx(
            fd,
            empty_path.as_ptr() as *const std::ffi::c_char,
            0x1000,
            STATX_BASIC_STATS,
            &mut stx,
        );
        assert_eq!(ret, 0, "statx failed on open dirfd");
        assert!(stx.stx_mode & S_IFMT == S_IFDIR);

        unsafe { close(fd) };
    }
}
