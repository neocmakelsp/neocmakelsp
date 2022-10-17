use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::utils::CMakePackage;
pub const PREFIX: [&str; 2] = ["C:\\", "D:\\"];
pub static CMAKE_PACKAGES: Lazy<Vec<CMakePackage>> =
    Lazy::new(|| vec![]);
pub static CMAKE_PACKAGES_WITHKEY: Lazy<HashMap<String, CMakePackage>> =
    Lazy::new(|| HashMap::new());
