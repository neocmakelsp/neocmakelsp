use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use super::{
    CMAKECONFIG, CMAKECONFIGVERSION, CMAKEREGEX, SPECIAL_PACKAGE_PATTERN, get_version,
    handle_config_package,
};
use crate::Uri;
use crate::utils::{CMakePackage, CMakePackageFrom, PackageType};

#[inline]
pub fn did_vcpkg_project(path: &Path) -> bool {
    path.is_dir() && path.join("vcpkg.json").is_file()
}

pub static VCPKG_PREFIX: LazyLock<Arc<Mutex<Vec<&str>>>> =
    LazyLock::new(|| Arc::new(Mutex::new([].to_vec())));

pub static VCPKG_LIBS: LazyLock<Arc<Mutex<Vec<&str>>>> =
    LazyLock::new(|| Arc::new(Mutex::new([].to_vec())));

#[cfg(not(windows))]
fn safe_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    path.canonicalize()
}

#[cfg(windows)]
fn safe_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    use path_absolutize::Absolutize;
    Ok(path.absolutize()?.into_owned())
}

fn get_available_libs() -> Vec<PathBuf> {
    let mut ava: Vec<PathBuf> = vec![];
    let vcpkg_prefix = VCPKG_PREFIX.lock().unwrap();
    let vcpkg_libs = VCPKG_LIBS.lock().unwrap();
    for prefix in vcpkg_prefix.iter() {
        for lib in vcpkg_libs.iter() {
            let p = Path::new(prefix).join(lib);
            if p.exists() {
                ava.push(p);
            }
        }
    }
    ava
}

fn get_cmake_message() -> HashMap<String, CMakePackage> {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();
    let vcpkg_prefix = VCPKG_PREFIX.lock().unwrap();
    for lib in vcpkg_prefix.iter() {
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
                tojump.push(safe_canonicalize(&file).unwrap());
                if CMAKECONFIG.is_match(file.to_str().unwrap()) {
                    ispackage = true;
                }
                if CMAKECONFIGVERSION.is_match(file.to_str().unwrap())
                    && let Ok(context) = fs::read_to_string(&file)
                {
                    version = get_version(&context);
                }
            }

            if let Some(config_file_location) = tojump
                .iter()
                .position(|file| CMAKECONFIG.is_match(file.to_str().unwrap()))
                && config_file_location != 0
            {
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
            let location = Uri::from_file_path(&path).unwrap();

            packages.insert(
                packagename.to_string(),
                CMakePackage {
                    name: packagename.to_string(),
                    packagetype: PackageType::Dir,
                    location,
                    version,
                    tojump,
                    from: CMakePackageFrom::Vcpkg,
                },
            );
        }
    }
    drop(vcpkg_prefix);
    for lib in get_available_libs() {
        let Ok(paths) = std::fs::read_dir(lib) else {
            continue;
        };
        for path in paths.flatten() {
            let mut version: Option<String> = None;
            let mut tojump: Vec<PathBuf> = vec![];
            let pathname = path.file_name().to_str().unwrap().to_string();
            let location = Uri::from_file_path(path.path()).unwrap();
            let (packagetype, mut packagename) = {
                if path.metadata().is_ok_and(|data| data.is_dir()) {
                    let Ok(paths) = std::fs::read_dir(path.path()) else {
                        continue;
                    };
                    for path in paths.flatten() {
                        let filepath = safe_canonicalize(&path.path()).unwrap();
                        if path.metadata().is_ok_and(|metadata| metadata.is_file()) {
                            let path_name = path.file_name();
                            let filename = path_name.to_str().unwrap();
                            if CMAKEREGEX.is_match(filename) {
                                tojump.push(filepath.clone());
                                if CMAKECONFIGVERSION.is_match(filename)
                                    && let Ok(context) = fs::read_to_string(&filepath)
                                {
                                    version = get_version(&context);
                                }
                            }
                        }
                    }
                    (PackageType::Dir, pathname)
                } else {
                    let filepath = safe_canonicalize(&path.path()).unwrap();
                    tojump.push(filepath);
                    let Some(pathname) = handle_config_package(&pathname) else {
                        continue;
                    };
                    (PackageType::File, pathname.to_owned())
                }
            };
            let config_file_location = tojump
                .iter()
                .position(|file| CMAKECONFIG.is_match(file.to_str().unwrap()))
                .unwrap();
            if config_file_location != 0 {
                tojump.swap(0, config_file_location);
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
                    from: CMakePackageFrom::Vcpkg,
                },
            );
        }
    }
    packages
}

pub fn make_vcpkg_package_search_path(search_path: &Path) -> std::io::Result<Vec<String>> {
    const LIB_PATHS: [&str; 6] = [
        "x64-linux",
        "x86-linux",
        "x64-windows",
        "x86-windows",
        "x64-osx",
        "arm64-osx",
    ];

    let mut paths: Vec<String> = Vec::new();

    // check search path is ok
    for item in LIB_PATHS {
        if search_path.join(item).is_dir() {
            let path = Path::new(item).join("share");
            paths.push(path.to_str().unwrap().to_string());
        }
    }

    Ok(paths)
}

pub static VCPKG_CMAKE_PACKAGES: LazyLock<Vec<CMakePackage>> =
    LazyLock::new(|| get_cmake_message().into_values().collect());
pub static VCPKG_CMAKE_PACKAGES_WITHKEY: LazyLock<HashMap<String, CMakePackage>> =
    LazyLock::new(get_cmake_message);

// FIXME: I can not fix the unit test on macos
// It always start with /private
#[cfg(unix)]
#[cfg(not(target_os = "macos"))]
#[test]
fn test_vcpkgpackage_search() {
    use std::fs;
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;

    use crate::utils::CMakePackageFrom;
    let dir = tempdir().unwrap();

    let vcpkg_path = dir.path().join("vcpkg.json");
    File::create(vcpkg_path).unwrap();

    assert!(did_vcpkg_project(dir.path()));

    let prefix_dir = safe_canonicalize(dir.path())
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut prefix = VCPKG_PREFIX.lock().unwrap();

    prefix.push(Box::leak(prefix_dir.into_boxed_str()));
    drop(prefix);

    let mut libs = VCPKG_LIBS.lock().unwrap();
    libs.push("x64-linux");
    libs.push("share/cmake");
    drop(libs);

    let share_path = dir.path().join("share");

    let cmake_dir = share_path.join("cmake");

    let vulkan_dir = cmake_dir.join("VulkanHeaders");
    fs::create_dir_all(&vulkan_dir).unwrap();
    let vulkan_config_cmake = vulkan_dir.join("VulkanHeadersConfig.cmake");
    File::create(&vulkan_config_cmake).unwrap();
    let vulkan_config_version_cmake = vulkan_dir.join("VulkanHeadersConfigVersion.cmake");
    let mut vulkan_config_version_file = File::create(&vulkan_config_version_cmake).unwrap();
    writeln!(
        vulkan_config_version_file,
        r#"set(PACKAGE_VERSION "1.3.295")"#
    )
    .unwrap();

    let ecm_dir = share_path.join("ECM").join("cmake");
    fs::create_dir_all(&ecm_dir).unwrap();
    let ecm_config_cmake = ecm_dir.join("ECMConfig.cmake");
    File::create(&ecm_config_cmake).unwrap();
    let ecm_config_version_cmake = ecm_dir.join("ECMConfigVersion.cmake");
    let mut ecm_config_version_file = File::create(&ecm_config_version_cmake).unwrap();
    writeln!(ecm_config_version_file, r#"set(PACKAGE_VERSION "6.5.0")"#).unwrap();

    let target = HashMap::from_iter([
        (
            "VulkanHeaders".to_string(),
            CMakePackage {
                name: "VulkanHeaders".to_string(),
                packagetype: PackageType::Dir,
                location: Uri::from_file_path(vulkan_dir).unwrap(),
                version: Some("1.3.295".to_string()),
                tojump: vec![
                    safe_canonicalize(&vulkan_config_cmake).unwrap(),
                    safe_canonicalize(&vulkan_config_version_cmake).unwrap(),
                ],
                from: CMakePackageFrom::Vcpkg,
            },
        ),
        (
            "ECM".to_string(),
            CMakePackage {
                name: "ECM".to_string(),
                packagetype: PackageType::Dir,
                location: Uri::from_file_path(ecm_dir).unwrap(),
                version: Some("6.5.0".to_string()),
                tojump: vec![
                    safe_canonicalize(&ecm_config_cmake).unwrap(),
                    safe_canonicalize(&ecm_config_version_cmake).unwrap(),
                ],
                from: CMakePackageFrom::Vcpkg,
            },
        ),
    ]);
    assert_eq!(get_cmake_message(), target);
}
