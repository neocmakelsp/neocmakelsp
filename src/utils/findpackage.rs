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
use once_cell::sync::Lazy;
// match file xx.cmake and CMakeLists.txt
const CMAKEREGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^.+\.cmake$|CMakeLists.txt$").unwrap());

// config file
const CMAKECONFIG: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^*Config.cmake$|^*-config.cmake").unwrap());
#[test]
fn regextest() {
    assert!(CMAKEREGEX.is_match("CMakeLists.txt"));
    assert!(!CMAKEREGEX.is_match("CMakeLists.txtb"));
    assert!(CMAKEREGEX.is_match("DtkCoreConfig.cmake"));
    assert!(!CMAKEREGEX.is_match("DtkCoreConfig.cmakeb"));
}
#[test]
fn configtest() {
    assert!(CMAKECONFIG.is_match("DtkCoreConfig.cmake"));
    assert!(CMAKECONFIG.is_match("DtkCore-config.cmake"));
    assert!(!CMAKECONFIG.is_match("DtkCoreconfig.cmake"));
}
