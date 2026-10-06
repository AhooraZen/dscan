#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::*;

#[cfg(target_os = "linux")]
pub mod uring;
#[cfg(target_os = "linux")]
pub use uring::*;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[inline(always)]
pub fn raise_fd_limit() {}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_rotational(dev: u64, _path: &std::path::Path) -> bool {
    linux::is_rotational_device(dev)
}

#[cfg(windows)]
pub fn is_rotational(_dev: u64, path: &std::path::Path) -> bool {
    windows::is_rotational_path(path)
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
pub fn is_rotational(_dev: u64, _path: &std::path::Path) -> bool {
    false
}
