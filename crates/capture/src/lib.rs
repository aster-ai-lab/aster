#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

pub mod aster;

#[cfg(not(target_os = "macos"))]
#[path = "capture_unsupported.rs"]
pub mod capture;
