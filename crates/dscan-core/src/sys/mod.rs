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
