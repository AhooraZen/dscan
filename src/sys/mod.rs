#[cfg(unix)]
mod linux;
#[cfg(unix)]
pub use linux::*;

#[cfg(target_os = "linux")]
pub mod uring;
#[cfg(target_os = "linux")]
pub use uring::*;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;
