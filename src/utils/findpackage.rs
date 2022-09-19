#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
mod packageunix;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use packageunix as cmakepackage;
#[cfg(target_os = "windows")]
mod packagewin;
#[cfg(target_os = "windows")]
use packagewin as cmakepackage;
#[cfg(target_os = "macos")]
mod packagemac;
#[cfg(target_os = "macos")]
use packagemac as cmakepackage;

pub use cmakepackage::PREFIX;
