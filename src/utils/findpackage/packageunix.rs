use std::{collections::HashMap, fs};

use anyhow::Result;
use once_cell::sync::Lazy;

use crate::utils::{CMakePackage, FileType};

use super::{get_version, CMAKECONFIGVERSION, CMAKEREGEX};

// here is the logic of findpackage on linux
//
pub const PREFIX: [&str; 2] = ["/usr", "/usr/local"];
pub const LIBS: [&str; 4] = ["lib", "lib32", "lib64", "share"];
pub static CMAKE_PACKAGES: Lazy<Result<Vec<CMakePackage>>> = Lazy::new(|| {
    let mut packages = vec![];
    for prefix in PREFIX {
        for lib in LIBS {
            if let Ok(paths) = std::fs::read_dir(format!("{}/{}/cmake", prefix, lib)) {
                for path in paths {
                    if let Ok(pathunit) = path {
                        let mut version: Option<String> = None;
                        let mut tojump: Vec<String> = vec![];
                        let pathname = pathunit.file_name().to_str().unwrap().to_string();
                        let packagepath = pathunit.path().to_str().unwrap().to_string();
                        let (packagetype, packagename) = {
                            if pathunit.metadata().unwrap().is_dir() {
                                if let Ok(paths) =
                                    std::fs::read_dir(pathunit.path().to_str().unwrap())
                                {
                                    for path in paths {
                                        if let Ok(pathunit) = path {
                                            let filepath =
                                                pathunit.path().to_str().unwrap().to_string();
                                            if pathunit.metadata().unwrap().is_file() {
                                                let filename = pathunit
                                                    .file_name()
                                                    .to_str()
                                                    .unwrap()
                                                    .to_string();
                                                if CMAKEREGEX.is_match(&filename) {
                                                    tojump.push(format!("file://{}", filepath));
                                                    if CMAKECONFIGVERSION.is_match(&filename) {
                                                        if let Ok(context) =
                                                            fs::read_to_string(&filepath)
                                                        {
                                                            version = get_version(&context);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                (FileType::Dir, pathname)
                            } else {
                                let filepath = pathunit.path().to_str().unwrap().to_string();
                                tojump.push(format!("file://{}", filepath));
                                let pathname =
                                    pathname.split('.').collect::<Vec<&str>>()[0].to_string();
                                (FileType::File, pathname)
                            }
                        };
                        packages.push(CMakePackage {
                            name: packagename,
                            filetype: packagetype,
                            filepath: packagepath,
                            version,
                            tojump,
                        });
                    }
                }
            }
        }
    }
    Ok(packages)
});
pub static CMAKE_PACKAGES_WITHKEY: Lazy<Result<HashMap<String, CMakePackage>>> = Lazy::new(|| {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();

    for prefix in PREFIX {
        for lib in LIBS {
            if let Ok(paths) = std::fs::read_dir(format!("{}/{}/cmake", prefix, lib)) {
                for path in paths {
                    if let Ok(pathunit) = path {
                        let mut version: Option<String> = None;
                        let mut tojump: Vec<String> = vec![];
                        let pathname = pathunit.file_name().to_str().unwrap().to_string();
                        let packagepath = pathunit.path().to_str().unwrap().to_string();
                        let (packagetype, packagename) = {
                            if pathunit.metadata().unwrap().is_dir() {
                                if let Ok(paths) =
                                    std::fs::read_dir(pathunit.path().to_str().unwrap())
                                {
                                    for path in paths {
                                        if let Ok(pathunit) = path {
                                            let filepath =
                                                pathunit.path().to_str().unwrap().to_string();
                                            if pathunit.metadata().unwrap().is_file() {
                                                let filename = pathunit
                                                    .file_name()
                                                    .to_str()
                                                    .unwrap()
                                                    .to_string();
                                                if CMAKEREGEX.is_match(&filename) {
                                                    tojump.push(format!("file://{}", filepath));
                                                    if CMAKECONFIGVERSION.is_match(&filename) {
                                                        if let Ok(context) =
                                                            fs::read_to_string(&filepath)
                                                        {
                                                            version = get_version(&context);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                (FileType::Dir, pathname)
                            } else {
                                let filepath = pathunit.path().to_str().unwrap().to_string();
                                tojump.push(format!("file://{}", filepath));
                                let pathname =
                                    pathname.split('.').collect::<Vec<&str>>()[0].to_string();
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
        }
    }
    Ok(packages)
});
