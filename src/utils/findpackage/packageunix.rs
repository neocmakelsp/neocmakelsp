use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use std::sync::LazyLock;

use crate::Url;

use crate::utils::{CMakePackage, CMakePackageFrom, PackageType};

use super::{get_version, CMAKECONFIG, CMAKECONFIGVERSION, CMAKEREGEX};

// here is the logic of findpackage on linux
//
const PREFIXS: [&str; 2] = ["/usr", "/usr/local"];

const LIBS: [&str; 5] = ["lib", "lib32", "lib64", "share", "lib/x86_64-linux-gnu"];

fn get_prefixs() -> Vec<String> {
    if let Ok(prefix) = std::env::var("PREFIX") {
        let mut prefixs: Vec<String> = PREFIXS
            .to_vec()
            .iter()
            .map(|prefix| prefix.to_string())
            .collect();
        prefixs.push(prefix.to_string());
        return prefixs;
    }
    PREFIXS
        .to_vec()
        .iter()
        .map(|prefix| prefix.to_string())
        .collect()
}

fn get_available_libs(prefixs: &Vec<String>) -> Vec<PathBuf> {
    let mut ava: Vec<PathBuf> = vec![];
    for prefix in prefixs {
        for lib in LIBS {
            let p = Path::new(&prefix).join(lib).join("cmake");
            if p.exists() {
                ava.push(p);
            }
        }
    }
    ava
}

#[inline]
fn get_cmake_message() -> HashMap<String, CMakePackage> {
    get_cmake_message_with_prefixs(&get_prefixs())
}

fn get_cmake_message_with_prefixs(prefixs: &Vec<String>) -> HashMap<String, CMakePackage> {
    let mut packages: HashMap<String, CMakePackage> = HashMap::new();
    for prefix in prefixs {
        let Ok(paths) = glob::glob(&format!("{prefix}/share/*/cmake/")) else {
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
                tojump.push(f.canonicalize().unwrap());
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
                let location = Url::from_file_path(&path).unwrap();
                let packagename = path
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
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
    }
    for lib in get_available_libs(prefixs) {
        let Ok(paths) = std::fs::read_dir(lib) else {
            continue;
        };

        for path in paths.flatten() {
            let mut version: Option<String> = None;
            let mut tojump: Vec<PathBuf> = vec![];
            let pathname = path.file_name().to_str().unwrap().to_string();
            let package_path = Url::from_file_path(path.path()).unwrap();
            let (packagetype, packagename) = {
                if path.metadata().is_ok_and(|data| data.is_dir()) {
                    let Ok(paths) = std::fs::read_dir(path.path()) else {
                        continue;
                    };
                    for path in paths.flatten() {
                        let filepath = path.path().canonicalize().unwrap();
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
                    (PackageType::Dir, pathname)
                } else {
                    tojump.push(path.path().canonicalize().unwrap());
                    let Some(pathname) = pathname.strip_suffix(".cmake") else {
                        continue;
                    };
                    (PackageType::File, pathname.to_owned())
                }
            };
            packages.insert(
                packagename.clone(),
                CMakePackage {
                    name: packagename,
                    packagetype,
                    location: package_path,
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

#[test]
fn test_prefix() {
    std::env::set_var("PREFIX", "/data/data/com.termux/files/usr");
    assert_eq!(
        get_prefixs(),
        vec![
            "/usr".to_string(),
            "/usr/local".to_string(),
            "/data/data/com.termux/files/usr".to_string()
        ]
    )
}

#[test]
fn test_package_search() {
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    let dir = tempdir().unwrap();

    let share_dir = dir.path().join("share");
    let cmake_dir = share_dir.join("cmake");
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

    let ecm_dir = share_dir.join("ECM").join("cmake");
    fs::create_dir_all(&ecm_dir).unwrap();
    let ecm_config_cmake = ecm_dir.join("ECMConfig.cmake");
    File::create(&ecm_config_cmake).unwrap();
    let ecm_config_version_cmake = ecm_dir.join("ECMConfigVersion.cmake");
    let mut ecm_config_version_file = File::create(&ecm_config_version_cmake).unwrap();
    writeln!(ecm_config_version_file, r#"set(PACKAGE_VERSION "6.5.0")"#).unwrap();

    let prefix = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let target = HashMap::from_iter([
        (
            "VulkanHeaders".to_string(),
            CMakePackage {
                name: "VulkanHeaders".to_string(),
                packagetype: PackageType::Dir,
                location: Url::from_file_path(vulkan_dir).unwrap(),
                version: Some("1.3.295".to_string()),
                tojump: vec![vulkan_config_cmake, vulkan_config_version_cmake],
                from: CMakePackageFrom::System,
            },
        ),
        (
            "ECM".to_string(),
            CMakePackage {
                name: "ECM".to_string(),
                packagetype: PackageType::Dir,
                location: Url::from_file_path(ecm_dir).unwrap(),
                version: Some("6.5.0".to_string()),
                tojump: vec![ecm_config_cmake, ecm_config_version_cmake],
                from: CMakePackageFrom::System,
            },
        ),
    ]);
    assert_eq!(get_cmake_message_with_prefixs(&vec![prefix]), target);
}
