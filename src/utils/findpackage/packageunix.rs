use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;

use crate::utils::{CMakePackage, FileType};

use super::{get_version, CMAKECONFIGVERSION, CMAKEREGEX};

// here is the logic of findpackage on linux
//
pub const PREFIX: [&str; 2] = ["/usr", "/usr/local"];

pub const LIBS: [&str; 5] = ["lib", "lib32", "lib64", "share", "lib/x86_64-linux-gnu"];

fn get_available_libs() -> Vec<PathBuf> {
    let mut ava: Vec<PathBuf> = vec![];
    for prefix in PREFIX {
        for lib in LIBS {
            let p = Path::new(prefix).join(lib).join("cmake");
            if p.exists() {
                ava.push(p.into());
            }
        }
    }
    ava
}

fn get_cmake_message() -> HashMap<String, CMakePackage> {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();
    for lib in get_available_libs() {
        if let Ok(paths) = std::fs::read_dir(lib) {
            for path in paths.flatten() {
                let mut version: Option<String> = None;
                let mut tojump: Vec<String> = vec![];
                let pathname = path.file_name().to_str().unwrap().to_string();
                let packagepath = path.path().to_str().unwrap().to_string();
                let (packagetype, packagename) = {
                    if path.metadata().unwrap().is_dir() {
                        if let Ok(paths) = std::fs::read_dir(path.path().to_str().unwrap()) {
                            for path in paths.flatten() {
                                let filepath = path.path().to_str().unwrap().to_string();
                                if path.metadata().unwrap().is_file() {
                                    let filename = path.file_name().to_str().unwrap().to_string();
                                    if CMAKEREGEX.is_match(&filename) {
                                        tojump.push(format!("file://{}", filepath));
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
                        let filepath = path.path().to_str().unwrap().to_string();
                        tojump.push(format!("file://{}", filepath));
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
    }
    packages
}

pub static CMAKE_PACKAGES: Lazy<Vec<CMakePackage>> =
    Lazy::new(|| get_cmake_message().into_values().collect());
pub static CMAKE_PACKAGES_WITHKEY: Lazy<HashMap<String, CMakePackage>> =
    Lazy::new(get_cmake_message);
