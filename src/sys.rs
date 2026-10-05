use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[cfg(target_arch = "x86_64")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(target_arch = "aarch64")]
pub const SYS_GETDENTS64: i64 = 61;

#[cfg(target_arch = "arm")]
pub const SYS_GETDENTS64: i64 = 217;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm")))]
pub const SYS_GETDENTS64: i64 = 217;

pub const O_RDONLY: i32 = 0;
pub const O_DIRECTORY: i32 = 0o0200000;
pub const O_CLOEXEC: i32 = 0o2000000;
pub const O_NOATIME: i32 = 0o1000000;

pub const DT_DIR: u8 = 4;
pub const DT_REG: u8 = 8;
pub const DT_UNKNOWN: u8 = 0;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LinuxDirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
}

unsafe extern "C" {
    pub fn syscall(number: i64, ...) -> i64;
    pub fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    pub fn close(fd: i32) -> i32;
}

/// Open a directory with direct flags (O_DIRECTORY | O_CLOEXEC | O_NOATIME).
pub fn open_dir(path: &Path) -> Option<i32> {
    let path_c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let fd = unsafe { open(path_c.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOATIME) };
    if fd >= 0 {
        Some(fd)
    } else {
        None
    }
}
