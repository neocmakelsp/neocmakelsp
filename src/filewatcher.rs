use std::sync::LazyLock;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

// match like ss_DIR:PATH=ss_DIR-NOTFOUND

static NOT_FOUND_LIBRARY: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?P<library>[\da-zA-Z]+)_DIR:PATH=([\da-zA-Z]+)_DIR-NOTFOUND$").unwrap()
});

static ERROR_PACKAGES: LazyLock<Arc<Mutex<Vec<String>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

#[test]
fn failedtest() {
    let a = "ss_DIR:PATH=ss_DIR-NOTFOUND";
    assert!(NOT_FOUND_LIBRARY.is_match(a));
    let cap = NOT_FOUND_LIBRARY.captures(a).unwrap();
    assert_eq!("ss", &cap["library"]);
}

pub fn refresh_error_packages<P: AsRef<Path>>(p: P) {
    let geterr = || -> anyhow::Result<_> {
        let mut errors: Vec<String> = Vec::new();
        let mut file = File::open(&p)?;
        let mut context = String::new();
        file.read_to_string(&mut context)?;
        let lines: Vec<&str> = context.lines().collect();
        for line in lines {
            if NOT_FOUND_LIBRARY.is_match(line) {
                let cap = NOT_FOUND_LIBRARY.captures(line).unwrap();
                errors.push(cap["library"].to_string());
            }
        }
        Ok(errors)
    };
    let errorpackages = geterr().unwrap_or_else(|_| Vec::new());
    let Ok(mut packages) = ERROR_PACKAGES.lock() else {
        return;
    };
    *packages = errorpackages;
}

pub fn clear_error_packages() {
    let Ok(mut packages) = ERROR_PACKAGES.lock() else {
        return;
    };
    *packages = Vec::new();
}

pub fn get_error_packages() -> Vec<String> {
    match ERROR_PACKAGES.lock() {
        Ok(packages) => packages.to_vec(),
        Err(_) => Vec::new(),
    }
}
