#[cfg(not(target_os = "windows"))]
mod packageunix;
#[cfg(target_os = "windows")]
mod packagewin;
mod vcpkg;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use tower_lsp::lsp_types::Uri;
use tree_sitter::Point;

pub use self::cmakepackage::*;
#[cfg(not(target_os = "windows"))]
use self::packageunix as cmakepackage;
#[cfg(target_os = "windows")]
use self::packagewin as cmakepackage;
pub use self::vcpkg::*;
use super::{CMakePackage, CMakePackageFrom, PackageType};
use crate::CMakeNodeKinds;
use crate::consts::TREESITTER_CMAKE_LANGUAGE;

const LIBS: &[Cow<'_, str>] = &[
    Cow::Borrowed("lib"),
    Cow::Borrowed("lib32"),
    Cow::Borrowed("lib64"),
    Cow::Borrowed("share"),
];

/// Used to query system information like platform specific paths.
static CMAKE_SYSTEM_INFORMATION: LazyLock<Option<String>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--system-information")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
});

fn handle_config_package(filename: &str) -> Option<&str> {
    if let Some(tryfirst) = filename.strip_suffix("-config.cmake") {
        return Some(tryfirst);
    }
    filename.strip_suffix("Config.cmake")
}

static SPECIAL_PACKAGE_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"([a-zA-Z_\d\-]+)-(\d+(\.\d+)*)").unwrap());

static CMAKE_PREFIXES: LazyLock<Vec<String>> = LazyLock::new(|| {
    if cfg!(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "openbsd"
    )) {
        query_cmake_prefixes_or(vec!["/usr/local".into(), "/usr".into()])
    } else if cfg!(target_os = "macos") {
        query_cmake_prefixes_or(vec![
            "/usr/local".into(),
            "/usr".into(),
            "/opt/homebrew".into(),
        ])
    } else if cfg!(target_os = "windows") {
        query_cmake_prefixes_or(vec![
            "C:\\Program Files".into(),
            "C:\\Program Files (x86)".into(),
            "C:\\Program Files\\CMake".into(),
        ])
    } else {
        vec![]
    }
});

fn query_cmake_prefixes_or(default: Vec<String>) -> Vec<String> {
    match query_cmake_prefixes() {
        Some(prefixes) => prefixes,
        None => {
            let mut prefix_paths = default;
            // Add platform specific prefixes from the environment
            if let Some(prefix) = get_env_prefix() {
                prefix_paths.push(prefix);
            }
            prefix_paths
        }
    }
}

fn query_cmake_prefixes() -> Option<Vec<String>> {
    let line = CMAKE_SYSTEM_INFORMATION.as_ref().and_then(|output| {
        output
            .lines()
            .find(|line| line.starts_with("CMAKE_SYSTEM_PREFIX_PATH"))
    })?;
    let (_, prefix_paths) = line.split_once(" ")?;
    let prefix_paths = prefix_paths.trim_matches('"');
    // FIXME: This likely contains duplicate entries of '/usr/local' on most systems
    // This could be solved by using Itertools::unique()
    let prefix_paths: Vec<String> = prefix_paths.split(";").map(String::from).collect();

    if !prefix_paths.is_empty() {
        Some(prefix_paths)
    } else {
        None
    }
}

/// Returns the value of `CMAKE_LIBRARY_ARCHITECTURE` reported by `cmake --system-information`, if
/// it's set.
fn get_library_architecture() -> Option<String> {
    CMAKE_SYSTEM_INFORMATION
        .as_ref()
        .and_then(|output| {
            output
                .lines()
                .find(|line| line.starts_with("CMAKE_LIBRARY_ARCHITECTURE"))
        })
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|line| line.trim_matches('"'))
        .map(str::to_owned)
}

fn get_available_libs(prefixes: &[String]) -> Vec<PathBuf> {
    let lib_arch = get_library_architecture().map(|arch| Cow::Owned(format!("lib/{arch}")));
    prefixes
        .iter()
        .map(Path::new)
        .flat_map(|path| {
            LIBS.iter()
                .chain(lib_arch.iter())
                .map(|lib| path.join(lib.as_ref()).join("cmake"))
                .filter(|path| path.exists())
        })
        .collect()
}

#[inline]
fn get_cmake_message() -> HashMap<String, CMakePackage> {
    get_cmake_message_with_prefixes(&CMAKE_PREFIXES)
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
            location: Uri::from_file_path("/usr/share/bash-completion-fake").unwrap(),
            #[cfg(not(unix))]
            location: Uri::from_file_path(r"C:\Develop\bash-completion-fake").unwrap(),
            version: None,
            #[cfg(unix)]
            tojump: vec![
                Path::new("/usr/share/bash-completion-fake/bash_completion-fake-config.cmake")
                    .to_path_buf(),
            ],
            #[cfg(not(unix))]
            tojump: vec![
                Path::new(r"C:\Develop\bash-completion-fake\bash-completion-fake-config.cmake")
                    .to_path_buf(),
            ],
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

fn string_from_two_valid_points(source: Vec<&str>, start: &Point, end: &Point) -> String {
    if start.row == end.row {
        return source[start.row][start.column..end.column].into();
    }

    let mut span = String::new();

    span += &source[start.row][start.column..];
    for row in source.iter().take(end.row).skip(start.row + 1) {
        span += row;
    }
    span += &source[end.row][..end.column];

    span
}

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
                        let version_string = string_from_two_valid_points(
                            newsource,
                            &version.start_position(),
                            &version.end_position(),
                        );
                        return Some(
                            version_string
                                .trim_matches(|c| c == '"' || c == '\n' || c == ' ')
                                .to_string(),
                        );
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
    use crate::Uri;

    pub struct PkgConfig {
        pub libname: String,
        pub path: Uri,
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
                            path: Uri::from_file_path(entry).unwrap(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_config_package_test() {
        let test_file = "libaec-config.cmake";
        let test_file = handle_config_package(test_file).unwrap();
        assert_eq!(test_file, "libaec");
    }

    #[test]
    fn special_package_pattern_test() {
        let matched_pattern = "boost_atomic-1.86.0";
        assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
        let captures = SPECIAL_PACKAGE_PATTERN.captures(matched_pattern).unwrap();
        assert_eq!(captures.get(1).unwrap().as_str(), "boost_atomic");
        assert_eq!(captures.get(2).unwrap().as_str(), "1.86.0");

        let matched_pattern = "QuaZip-Qt5-1.4";
        assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
        let captures = SPECIAL_PACKAGE_PATTERN.captures(matched_pattern).unwrap();
        assert_eq!(captures.get(1).unwrap().as_str(), "QuaZip-Qt5");
        assert_eq!(captures.get(2).unwrap().as_str(), "1.4");

        let matched_pattern = "boost_atomic2-1.86.0";
        assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
        let captures = SPECIAL_PACKAGE_PATTERN.captures(matched_pattern).unwrap();
        assert_eq!(captures.get(1).unwrap().as_str(), "boost_atomic2");
        assert_eq!(captures.get(2).unwrap().as_str(), "1.86.0");

        let matched_pattern = "mongoc-1.0";
        assert!(SPECIAL_PACKAGE_PATTERN.is_match(matched_pattern));
        let captures = SPECIAL_PACKAGE_PATTERN.captures(matched_pattern).unwrap();
        assert_eq!(captures.get(1).unwrap().as_str(), "mongoc");
        assert_eq!(captures.get(2).unwrap().as_str(), "1.0");

        let unmatched_pattern = "Qt5Core";

        assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern));

        let unmatched_pattern = "libjpeg-turbo";

        assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern));

        let unmatched_pattern = "QuaZip-Qt5";
        assert!(!SPECIAL_PACKAGE_PATTERN.is_match(unmatched_pattern));
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
    fn test_version() {
        let projectversion = "set(PACKAGE_VERSION 5.11)";
        assert_eq!(get_version(projectversion), Some("5.11".to_string()));
        let projectversion = "SET(PACKAGE_VERSION 5.11)";
        assert_eq!(get_version(projectversion), Some("5.11".to_string()));
        let projectversion = "set(PACKAGE_VERSION \"1.3.14
\")";
        assert_eq!(get_version(projectversion), Some("1.3.14".to_string()));
        let projectversion = "set(PACKAGE_VERSION \"1.3.14

\")";
        assert_eq!(get_version(projectversion), Some("1.3.14".to_string()));
        let qmlversion = include_str!("../../assets_for_test/Qt5QmlConfigVersion.cmake");
        assert_eq!(get_version(qmlversion), Some("5.15.6".to_string()));
    }
}
