#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "openbsd"
))]
mod packageunix;
mod vcpkg;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "openbsd"
))]
use packageunix as cmakepackage;
#[cfg(target_os = "windows")]
mod packagewin;
#[cfg(target_os = "windows")]
use packagewin as cmakepackage;
#[cfg(target_os = "macos")]
mod packagemac;
#[cfg(target_os = "macos")]
use packagemac as cmakepackage;
use tower_lsp::lsp_types::Url;

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::CMakeNodeKinds;

use super::{remove_quotation, CMakePackage, CMakePackageFrom, PackageType};

pub use cmakepackage::*;
pub use vcpkg::*;

use std::{collections::HashMap, sync::LazyLock};

fn handle_config_package(filename: &str) -> Option<&str> {
    if let Some(tryfirst) = filename.strip_suffix("-config.cmake") {
        return Some(tryfirst);
    }
    filename.strip_suffix("Config.cmake")
}

#[test]
fn handle_config_package_tst() {
    let test_file = "libaec-config.cmake";
    let tst_file = handle_config_package(&test_file).unwrap();
    assert_eq!(tst_file, "libaec");
}

static SPECIAL_PACKAGE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"([a-zA-Z_\d\-]+)-(\d+(\.\d+)*)").unwrap());

#[test]
fn special_package_pattern_tst() {
    let matched_pattern = "boost_atomic-1.86.0";
    assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
    let captures = SPECIAL_PACKAGE_PATTERN.captures(&matched_pattern).unwrap();
    assert_eq!(captures.get(1).unwrap().as_str(), "boost_atomic");
    assert_eq!(captures.get(2).unwrap().as_str(), "1.86.0");

    let matched_pattern = "QuaZip-Qt5-1.4";
    assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
    let captures = SPECIAL_PACKAGE_PATTERN.captures(&matched_pattern).unwrap();
    assert_eq!(captures.get(1).unwrap().as_str(), "QuaZip-Qt5");
    assert_eq!(captures.get(2).unwrap().as_str(), "1.4");

    let matched_pattern = "boost_atomic2-1.86.0";
    assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
    let captures = SPECIAL_PACKAGE_PATTERN.captures(&matched_pattern).unwrap();
    assert_eq!(captures.get(1).unwrap().as_str(), "boost_atomic2");
    assert_eq!(captures.get(2).unwrap().as_str(), "1.86.0");

    let matched_pattern = "mongoc-1.0";
    assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
    let captures = SPECIAL_PACKAGE_PATTERN.captures(&matched_pattern).unwrap();
    assert_eq!(captures.get(1).unwrap().as_str(), "mongoc");
    assert_eq!(captures.get(2).unwrap().as_str(), "1.0");

    let unmatched_pattern = "Qt5Core";

    assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern));

    let unmatched_pattern = "libjpeg-turbo";

    assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern));

    let unmatched_pattern = "QuaZip-Qt5";
    assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern))
}

// match file xx.cmake and CMakeLists.txt
static CMAKEREGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^.+\.cmake$|CMakeLists.txt$").unwrap());

// config file
static CMAKECONFIG: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^*Config.cmake$|^*-config.cmake$").unwrap());
// config version file
static CMAKECONFIGVERSION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^*ConfigVersion.cmake$").unwrap());

pub trait FindPackageFunsTrait {
    fn get_cmake_packages(&self) -> Vec<CMakePackage> {
        let mut cmake_packages = CMAKE_PACKAGES.clone();
        cmake_packages.extend(VCPKG_CMAKE_PACKAGES.clone());
        cmake_packages
    }
    fn get_cmake_packages_withkeys(&self) -> HashMap<String, CMakePackage> {
        let mut cmake_packages_keys = CMAKE_PACKAGES_WITHKEY.clone();
        cmake_packages_keys.extend(VCPKG_CMAKE_PACKAGES_WITHKEY.clone());
        cmake_packages_keys
    }

    #[cfg(unix)]
    fn get_pkg_config_packages_withkey(&self) -> HashMap<String, packagepkgconfig::PkgConfig> {
        packagepkgconfig::get_pkg_messages()
    }

    #[cfg(unix)]
    fn get_pkg_config_packages(&self) -> Vec<packagepkgconfig::PkgConfig> {
        packagepkgconfig::get_pkg_messages().into_values().collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FindPackageFunsFake;

impl FindPackageFunsFake {
    pub fn fake_cmake_data(&self) -> HashMap<String, CMakePackage> {
        use std::path::Path;
        let fake_package = CMakePackage {
            name: "bash-completion-fake".to_string(),
            packagetype: PackageType::Dir,
            #[cfg(unix)]
            location: Url::from_file_path("/usr/share/bash-completion-fake").unwrap(),
            #[cfg(not(unix))]
            location: Url::from_file_path(r"C:\Develop\bash-completion-fake").unwrap(),
            version: None,
            #[cfg(unix)]
            tojump: vec![Path::new(
                "/usr/share/bash-completion-fake/bash_completion-fake-config.cmake",
            )
            .to_path_buf()],
            #[cfg(not(unix))]
            tojump: vec![Path::new(
                r"C:\Develop\bash-completion-fake\bash-completion-fake-config.cmake",
            )
            .to_path_buf()],
            from: CMakePackageFrom::System,
        };

        HashMap::from_iter([("bash-completion-fake".to_string(), fake_package.clone())])
    }
}

impl FindPackageFunsTrait for FindPackageFunsFake {
    fn get_cmake_packages(&self) -> Vec<CMakePackage> {
        self.fake_cmake_data().into_values().collect()
    }
    fn get_cmake_packages_withkeys(&self) -> HashMap<String, CMakePackage> {
        self.fake_cmake_data()
    }
}

// NOTE:This is the real function to find package
// To use trait is to make it possible to moc the logic
#[derive(Debug, Clone, Copy)]
pub struct FindPackageFunsReal;

impl FindPackageFunsTrait for FindPackageFunsReal {}

#[inline]
fn get_cmake_packages_withkeys() -> HashMap<String, CMakePackage> {
    if cfg!(test) {
        FindPackageFunsFake.get_cmake_packages_withkeys()
    } else {
        FindPackageFunsReal.get_cmake_packages_withkeys()
    }
}

#[inline]
fn get_cmake_packages() -> Vec<CMakePackage> {
    if cfg!(test) {
        FindPackageFunsFake.get_cmake_packages()
    } else {
        FindPackageFunsReal.get_cmake_packages()
    }
}

pub static CACHE_CMAKE_PACKAGES: LazyLock<Vec<CMakePackage>> = LazyLock::new(get_cmake_packages);

pub static CACHE_CMAKE_PACKAGES_WITHKEYS: LazyLock<HashMap<String, CMakePackage>> =
    LazyLock::new(get_cmake_packages_withkeys);

fn get_version(source: &str) -> Option<String> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let input = tree.root_node();
    let mut course = input.walk();
    for child in input.children(&mut course) {
        if child.kind() == CMakeNodeKinds::NORMAL_COMMAND {
            let h = child.start_position().row;
            let ids = child.child(0).unwrap();
            let x = ids.start_position().column;
            let y = ids.end_position().column;
            let name = &newsource[h][x..y];
            if name == "set" || name == "SET" {
                let argumentlist = child.child(2)?;
                let id = argumentlist.child(0)?;
                let version = argumentlist.child(1)?;
                let h = id.start_position().row;
                let x = id.start_position().column;
                let y = id.end_position().column;
                if x != y {
                    let name = &newsource[h][x..y];
                    if name == "PACKAGE_VERSION" {
                        let h = version.start_position().row;
                        let x = version.start_position().column;
                        let y = version.end_position().column;
                        let version = &newsource[h][x..y];
                        return Some(remove_quotation(version).to_string());
                    }
                }
            }
        }
    }

    None
}
#[cfg(unix)]
pub mod packagepkgconfig {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};

    use super::{FindPackageFunsFake, FindPackageFunsReal, FindPackageFunsTrait};
    use crate::Url;

    pub struct PkgConfig {
        pub libname: String,
        pub path: Url,
    }

    pub static QUERYSRULES: LazyLock<Arc<Mutex<Vec<&str>>>> = LazyLock::new(|| {
        Arc::new(Mutex::new(
            ["/usr/lib/pkgconfig/*.pc", "/usr/lib/*/pkgconfig/*.pc"].to_vec(),
        ))
    });

    pub(super) fn get_pkg_messages() -> HashMap<String, PkgConfig> {
        let mut packages: HashMap<String, PkgConfig> = HashMap::new();
        let mut generatepackage = || -> anyhow::Result<()> {
            for path in QUERYSRULES.lock().unwrap().iter() {
                for entry in glob::glob(path)?.flatten() {
                    let Some(file_name) =
                        entry.file_name().and_then(|file_name| file_name.to_str())
                    else {
                        continue;
                    };
                    let Some(realname) = file_name.strip_suffix(".pc") else {
                        continue;
                    };

                    packages.insert(
                        realname.to_string(),
                        PkgConfig {
                            libname: realname.to_string(),
                            path: Url::from_file_path(entry).unwrap(),
                        },
                    );
                }
            }
            Ok(())
        };
        let _ = generatepackage();
        packages
    }

    #[inline]
    fn get_pkg_config_packages_withkey() -> HashMap<String, PkgConfig> {
        if cfg!(test) {
            FindPackageFunsFake.get_pkg_config_packages_withkey()
        } else {
            FindPackageFunsReal.get_pkg_config_packages_withkey()
        }
    }

    #[inline]
    fn get_pkg_config_packages() -> Vec<PkgConfig> {
        if cfg!(test) {
            FindPackageFunsFake.get_pkg_config_packages()
        } else {
            FindPackageFunsReal.get_pkg_config_packages()
        }
    }

    pub static PKG_CONFIG_PACKAGES_WITHKEY: LazyLock<HashMap<String, PkgConfig>> =
        LazyLock::new(get_pkg_config_packages_withkey);

    pub static PKG_CONFIG_PACKAGES: LazyLock<Vec<PkgConfig>> =
        LazyLock::new(get_pkg_config_packages);
}

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
    assert!(CMAKECONFIG.is_match("/usr/share/ECM/cmake/ECMConfig.cmake"));
    assert!(!CMAKECONFIG.is_match("DtkCoreconfig.cmake"));
}
#[test]
fn tst_version() {
    let projectversion = "set(PACKAGE_VERSION 5.11)";
    assert_eq!(get_version(projectversion), Some("5.11".to_string()));
    let projectversion = "SET(PACKAGE_VERSION 5.11)";
    assert_eq!(get_version(projectversion), Some("5.11".to_string()));
    let qmlversion = include_str!("../../assert/Qt5QmlConfigVersion.cmake");
    assert_eq!(get_version(qmlversion), Some("5.15.6".to_string()));
}
