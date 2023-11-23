use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;

use crate::utils::{CMakePackage, FileType};

use super::{get_version, CMAKECONFIG, CMAKECONFIGVERSION, CMAKEREGEX};

// here is the logic of findpackage on linux
//
pub const PREFIX: [&str; 3] = ["/usr", "/usr/local", "/opt/homebrew"];

pub const LIBS: [&str; 4] = ["lib", "lib32", "lib64", "share"];

fn get_available_libs() -> Vec<PathBuf> {
    let mut ava: Vec<PathBuf> = vec![];
    for prefix in PREFIX {
        for lib in LIBS {
            let p = Path::new(prefix).join(lib).join("cmake");
            if p.exists() {
                ava.push(p);
            }
        }
    }
    ava
}

fn get_cmake_message() -> HashMap<String, CMakePackage> {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();
    for lib in PREFIX {
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
            for f in files.flatten() {
                tojump.push(fs::canonicalize(f.clone()).unwrap());
                if CMAKECONFIG.is_match(f.to_str().unwrap()) {
                    ispackage = true;
                }
                if CMAKECONFIGVERSION.is_match(f.to_str().unwrap()) {
                    if let Ok(context) = fs::read_to_string(&f) {
                        version = get_version(&context);
                    }
                }
            }
            if ispackage {
                let packagename = path
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
                packages
                    .entry(packagename.to_string())
                    .or_insert_with(|| CMakePackage {
                        name: packagename.to_string(),
                        filetype: FileType::Dir,
                        filepath: path.to_str().unwrap().to_string(),
                        version,
                        tojump,
                    });

                //ava.push(path);
            }
        }
    }
    for lib in get_available_libs() {
        let Ok(paths) = std::fs::read_dir(lib) else {
            continue;
        };
        for path in paths.flatten() {
            let mut version: Option<String> = None;
            let mut tojump: Vec<PathBuf> = vec![];
            let pathname = path.file_name().to_str().unwrap().to_string();
            let packagepath = path.path().to_str().unwrap().to_string();
            let (packagetype, packagename) = {
                if path.metadata().unwrap().is_dir() {
                    if let Ok(paths) = std::fs::read_dir(path.path().to_str().unwrap()) {
                        for path in paths.flatten() {
                            let filepath = fs::canonicalize(path.path()).unwrap();
                            if path.metadata().unwrap().is_file() {
                                let filename = path.file_name().to_str().unwrap().to_string();
                                if CMAKEREGEX.is_match(&filename) {
                                    tojump.push(filepath.clone());
                                    if CMAKECONFIGVERSION.is_match(&filename) {
                                        if let Ok(context) = fs::read_to_string(&filepath) {
                                            version = get_version(&context);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    (FileType::Dir, pathname)
                } else {
                    let filepath = fs::canonicalize(path.path()).unwrap();
                    tojump.push(filepath);
                    let pathname = pathname.split('.').collect::<Vec<&str>>()[0].to_string();
                    (FileType::File, pathname)
                }
            };
            packages
                .entry(packagename.clone())
                .or_insert_with(|| CMakePackage {
                    name: packagename,
                    filetype: packagetype,
                    filepath: packagepath,
                    version,
                    tojump,
                });
        }
    }
    packages
}

pub static CMAKE_PACKAGES: Lazy<Vec<CMakePackage>> =
    Lazy::new(|| get_cmake_message().into_values().collect());
pub static CMAKE_PACKAGES_WITHKEY: Lazy<HashMap<String, CMakePackage>> =
    Lazy::new(get_cmake_message);
