#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "openbsd"
))]
mod packageunix;
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

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use core::str;
use std::path::Path;
use std::sync::LazyLock;
// match file xx.cmake and CMakeLists.txt
static CMAKEREGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^.+\.cmake$|CMakeLists.txt$").unwrap());

// config file
static CMAKECONFIG: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^*Config.cmake$|^*-config.cmake$").unwrap());
// config version file
static CMAKECONFIGVERSION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^*ConfigVersion.cmake$").unwrap());
fn get_version(source: &str) -> Option<String> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let input = tree.root_node();
    let mut course = input.walk();
    for child in input.children(&mut course) {
        if child.kind() == "normal_command" {
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
                        return Some(newsource[h][x..y].to_string());
                    }
                }
            }
        }
    }

    None
}

pub fn did_vcpkg_project(root: &Path) -> bool {
    if root.is_dir() && root.join("vcpkg.json").is_file() {
        return true;
    }
    return false;
}

#[allow(unused_variables)]
pub fn make_vcpkg_package_search_path(search_path: &Path) -> std::io::Result<Vec<&'static str>> {
    // TODO: Dynamic search directory
    let paths: Vec<&'static str> = [
        "x64-linux/share",
        "x86-linux/share",
        "x64-windows/share",
        "x86-windows/share",
        "x64-osx/share",
    ]
    .to_vec();
    return Ok(paths);
}

#[cfg(unix)]
pub mod packagepkgconfig {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};
    pub struct PkgConfig {
        pub libname: String,
        pub path: String,
    }

    fn get_pkg_messages(querystr_s: Vec<&str>) -> HashMap<String, PkgConfig> {
        let mut packages: HashMap<String, PkgConfig> = HashMap::new();
        let mut generatepackage = || -> anyhow::Result<()> {
            for query_str in &querystr_s {
                for entry in glob::glob(&query_str)?.flatten() {
                    let p = entry.as_path().to_str().unwrap();
                    let name = p
                        .split('/')
                        .collect::<Vec<&str>>()
                        .last()
                        .unwrap()
                        .to_string();
                    let realname = name
                        .split('.')
                        .collect::<Vec<&str>>()
                        .first()
                        .unwrap()
                        .to_string();
                    packages
                        .entry(realname.to_string())
                        .or_insert_with(|| PkgConfig {
                            libname: realname,
                            path: p.to_string(),
                        });
                }
            }
            Ok(())
        };
        let _ = generatepackage();
        packages
    }

    // query packages rule
    pub static QUERYSRULES: LazyLock<Arc<Mutex<Vec<&str>>>> = LazyLock::new(|| {
        Arc::new(Mutex::new(
            ["/usr/lib/pkgconfig/*.pc", "/usr/lib/*/pkgconfig/*.pc"].to_vec(),
        ))
    });

    pub static PKG_CONFIG_PACKAGES_WITHKEY: LazyLock<HashMap<String, PkgConfig>> =
        LazyLock::new(|| get_pkg_messages(QUERYSRULES.lock().unwrap().to_vec()));

    pub static PKG_CONFIG_PACKAGES: LazyLock<Vec<PkgConfig>> = LazyLock::new(|| {
        get_pkg_messages(QUERYSRULES.lock().unwrap().to_vec())
            .into_values()
            .collect()
    });
}
pub use cmakepackage::*;

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
