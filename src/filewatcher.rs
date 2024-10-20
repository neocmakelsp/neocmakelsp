use std::path::Path;
use std::sync::{LazyLock, Mutex};

// match like ss_DIR:PATH=ss_DIR-NOTFOUND
static NOT_FOUND_LIBRARY: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?P<library>[\da-zA-Z]+)_DIR:PATH=([\da-zA-Z]+)_DIR-NOTFOUND$").unwrap()
});

static ERROR_PACKAGES: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[test]
fn failedtest() {
    let a = "ss_DIR:PATH=ss_DIR-NOTFOUND";
    assert!(NOT_FOUND_LIBRARY.is_match(a));
    let cap = NOT_FOUND_LIBRARY.captures(a).unwrap();
    assert_eq!("ss", &cap["library"]);
}

pub fn refresh_error_packages<P: AsRef<Path>>(cmake_cache: P) -> Option<Vec<String>> {
    use std::fs;
    let mut toswap_packages: Vec<String> = Vec::new();
    let context = fs::read_to_string(cmake_cache.as_ref()).ok()?;
    for line in context.lines() {
        let Some(cap) = NOT_FOUND_LIBRARY.captures(line) else {
            continue;
        };
        toswap_packages.push(cap["library"].to_string());
    }
    let mut packages = ERROR_PACKAGES.lock().ok()?;
    std::mem::swap(&mut *packages, &mut toswap_packages);
    Some(toswap_packages)
}

pub fn clear_error_packages() -> Option<Vec<String>> {
    let mut packages = ERROR_PACKAGES.lock().ok()?;
    let mut old_packages = vec![];
    std::mem::swap(&mut *packages, &mut old_packages);
    Some(old_packages)
}

pub fn get_error_packages() -> Vec<String> {
    let Ok(packages) = ERROR_PACKAGES.lock() else {
        return vec![];
    };
    packages.to_vec()
}

#[test]
fn tst_cache_packages() {
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let error_cmake = dir.path().join("CMakeCache.txt");
    let cache_info = r"
ss_DIR:PATH=ss_DIR-NOTFOUND
";
    let mut cache_file = File::create(&error_cmake).unwrap();
    writeln!(cache_file, "{}", cache_info).unwrap();
    let origin = refresh_error_packages(error_cmake).unwrap();
    assert!(origin.is_empty());
    let error_packages = get_error_packages();
    assert_eq!(error_packages, vec!["ss"]);
    let cleared_packages = clear_error_packages().unwrap();
    assert_eq!(cleared_packages, vec!["ss"]);
    let error_packages_after = get_error_packages();
    assert!(error_packages_after.is_empty());
}
