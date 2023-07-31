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
static CMAKEREGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^.+\.cmake$|CMakeLists.txt$").unwrap());

// config file
static CMAKECONFIG: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^*Config.cmake$|^*-config.cmake$").unwrap());
// config version file
static CMAKECONFIGVERSION: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^*ConfigVersion.cmake$").unwrap());
fn get_version(source: &str) -> Option<String> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
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
                let Some(argumentlist) = child.child(2) else {
                    return None;
                };
                let Some(id) = argumentlist.child(0) else {
                    return None;
                };
                let Some(version) = argumentlist.child(1) else {
                    return None;
                };
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
#[cfg(unix)]
pub mod packagepkgconfig {
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    pub struct PkgConfig {
        pub libname: String,
        pub path: String,
    }

    fn get_pkg_messages() -> HashMap<String, PkgConfig> {
        let mut packages: HashMap<String, PkgConfig> = HashMap::new();
        let mut generatepackage = || -> anyhow::Result<()> {
            for entry in glob::glob("/usr/lib/pkgconfig/*.pc")?.flatten() {
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
            for entry in glob::glob("/usr/lib/*/pkgconfig/*.pc")?.flatten() {
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
            Ok(())
        };
        let _ = generatepackage();
        packages
    }
    pub static PKG_CONFIG_PACKAGES_WITHKEY: Lazy<HashMap<String, PkgConfig>> =
        Lazy::new(get_pkg_messages);

    pub static PKG_CONFIG_PACKAGES: Lazy<Vec<PkgConfig>> =
        Lazy::new(|| get_pkg_messages().into_values().collect());
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
