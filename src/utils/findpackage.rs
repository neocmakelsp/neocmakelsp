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
            //let ids = ids.child(2).unwrap();
            let x = ids.start_position().column;
            let y = ids.end_position().column;
            let name = &newsource[h][x..y];
            if name == "set" || name == "SET" {
                if let Some(id) = child.child(2) {
                    if let Some(version) = child.child(3) {
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
        }
    }

    None
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
