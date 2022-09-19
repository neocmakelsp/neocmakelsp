#[cfg(target_os="linux")]
mod packagelinux;
#[cfg(target_os="linux")]
use packagelinux as cmakepackage;
#[cfg(target_os="windows")]
mod packagewin;
#[cfg(target_os="windows")]
use packagewin as cmakepackage;
#[cfg(target_os="macos")]
mod packagemac;
#[cfg(target_os="macos")]
use packagemac as cmakepackage;
