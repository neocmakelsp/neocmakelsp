use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use super::{
    get_available_libs, get_cmake_message, get_version, handle_config_package, CMAKECONFIG,
    CMAKECONFIGVERSION, CMAKEREGEX, SPECIAL_PACKAGE_PATTERN,
};
use crate::utils::{CMakePackage, CMakePackageFrom, PackageType};
use crate::Url;

// here is the logic of findpackage on linux
//
pub(super) const LIBS: [&str; 4] = ["lib", "lib32", "lib64", "share"];

pub(super) fn get_env_prefix() -> Option<String> {
    None
}

pub(super) fn get_cmake_message_with_prefixes(
    prefixes: &Vec<String>,
) -> HashMap<String, CMakePackage> {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();
    for lib in prefixes {
        let Ok(paths) = glob::glob(&format!("{lib}/share/*/cmake/")) else {
            continue;
        };
        for path in paths.flatten() {
            let Ok(files) = glob::glob(&format!("{}/*.cmake", path.to_string_lossy())) else {
                continue;
            };
            let mut tojump: Vec<PathBuf> = vec![];
            let mut version: Option<String> = None;
            let mut ispackage = false;
            for file in files.flatten() {
                tojump.push(fs::canonicalize(file.clone()).unwrap());
                if CMAKECONFIG.is_match(file.to_str().unwrap()) {
                    ispackage = true;
                }
                if CMAKECONFIGVERSION.is_match(file.to_str().unwrap()) {
                    if let Ok(context) = fs::read_to_string(&file) {
                        version = get_version(&context);
                    }
                }
            }
            let config_file_location = tojump
                .iter()
                .position(|file| CMAKECONFIG.is_match(file.to_str().unwrap()))
                .unwrap();
            if config_file_location != 0 {
                tojump.swap(0, config_file_location);
            }
            if !ispackage {
                continue;
            }

            let Some(parent_path) = path.parent() else {
                continue;
            };
            let Some(packagename) = parent_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
            else {
                continue;
            };

            let location = Url::from_file_path(&path).unwrap();

            packages.insert(
                packagename.to_string(),
                CMakePackage {
                    name: packagename.to_string(),
                    packagetype: PackageType::Dir,
                    location,
                    version,
                    tojump,
                    from: CMakePackageFrom::System,
                },
            );
        }
    }
    for lib in get_available_libs(prefixes) {
        let Ok(paths) = std::fs::read_dir(lib) else {
            continue;
        };
        for path in paths.flatten() {
            let mut version: Option<String> = None;
            let mut tojump: Vec<PathBuf> = vec![];
            let pathname = path.file_name().to_str().unwrap().to_string();
            let location = Url::from_file_path(path.path()).unwrap();
            let (packagetype, mut packagename) = {
                if path.metadata().is_ok_and(|data| data.is_dir()) {
                    let Ok(paths) = std::fs::read_dir(path.path()) else {
                        continue;
                    };
                    for path in paths.flatten() {
                        let filepath = fs::canonicalize(path.path()).unwrap();
                        if path.metadata().is_ok_and(|metadata| metadata.is_file()) {
                            let path_name = path.file_name();
                            let filename = path_name.to_str().unwrap();
                            if CMAKEREGEX.is_match(filename) {
                                tojump.push(filepath.clone());
                                if CMAKECONFIGVERSION.is_match(filename) {
                                    if let Ok(context) = fs::read_to_string(&filepath) {
                                        version = get_version(&context);
                                    }
                                }
                            }
                        }
                    }
                    (PackageType::Dir, pathname)
                } else {
                    let filepath = fs::canonicalize(path.path()).unwrap();
                    tojump.push(filepath);
                    let Some(pathname) = handle_config_package(&pathname) else {
                        continue;
                    };
                    (PackageType::File, pathname.to_owned())
                }
            };

            if let Some(config_file_location) = tojump
                .iter()
                .position(|file| CMAKECONFIG.is_match(file.to_str().unwrap()))
            {
                if config_file_location != 0 {
                    tojump.swap(0, config_file_location);
                }
            }

            if let Some(captures) = SPECIAL_PACKAGE_PATTERN.captures(&packagename.clone()) {
                packagename = captures.get(1).unwrap().as_str().to_owned();
                version = captures.get(2).map(|version| version.as_str().to_string());
            }
            packages.insert(
                packagename.clone(),
                CMakePackage {
                    name: packagename,
                    packagetype,
                    location,
                    version,
                    tojump,
                    from: CMakePackageFrom::System,
                },
            );
        }
    }
    packages
}

pub static CMAKE_PACKAGES: LazyLock<Vec<CMakePackage>> =
    LazyLock::new(|| get_cmake_message().into_values().collect());
pub static CMAKE_PACKAGES_WITHKEY: LazyLock<HashMap<String, CMakePackage>> =
    LazyLock::new(get_cmake_message);
