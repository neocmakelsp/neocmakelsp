mod findpackage;
pub mod query;
pub mod treehelper;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use chrono::{DateTime, Local};
use etcetera::BaseStrategy;
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::CompletionItem;
use tower_lsp::lsp_types::Uri;

pub use self::findpackage::*;
use crate::fileapi;
use crate::jump::JumpCacheUnit;

pub mod cache {
    pub mod builtin {
        pub const MODULE_CACHE: &str = "builtin_module_cache.json";
        pub const VARIABLE_CACHE: &str = "builtin_variable_cache.json";
        pub const COMMANDS_CACHE: &str = "builtin_commands.json";
        pub const COMMANDS_SNIPPET_CACHE: &str = "builtin_commands_snippet.json";
        pub const MESSAGE_CACHE: &str = "messages_cache.json";
    }

    pub mod project {
        pub const TREE_MAP_CACHE: &str = "tree_map_cache.json";
        pub const TREE_CMAKE_MAP_CACHE: &str = "tree_cmake_map_cache.json";
        pub const COMPLETIONS_CACHE: &str = "project_completions_cache.json";
        pub const JUMPITEMS_CACHE: &str = "project_jumpitems_cache.json";
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CachedData<Data, const TIME_CHECK: bool = true> {
    pub date: DateTime<Local>,
    pub data: Data,
}

pub type CachedCompleteItems = CachedData<Vec<CompletionItem>>;
pub type CachedMessages = CachedData<HashMap<String, String>>;

pub type CachedProjectTree = CachedData<HashMap<PathBuf, PathBuf>, false>;
pub type CachedProjectCMakeMap = CachedData<HashMap<PathBuf, Vec<PathBuf>>, false>;
pub type CachedPCompleteItems = CachedData<HashMap<PathBuf, Vec<CompletionItem>>, false>;
pub type CachedPJumpItems = CachedData<HashMap<String, JumpCacheUnit>, false>;

pub static BUILTIN_MODULE_CACHED_DIR: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    let strategy = etcetera::choose_base_strategy().ok()?;
    let cache_dir = strategy.cache_dir();
    Some(cache_dir.join("neocmakelsp"))
});

impl<Data, const TIME_CHECK: bool> CachedData<Data, TIME_CHECK>
where
    Data: for<'a> Deserialize<'a>,
{
    pub fn read<P: AsRef<Path>>(path: P) -> Option<Self> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn new(data: Data) -> Self {
        let dt = Local::now();
        // Get components
        let naive_utc = dt.naive_utc();
        let offset = *dt.offset();
        Self {
            date: DateTime::from_naive_utc_and_offset(naive_utc, offset),
            data,
        }
    }
    pub fn need_update(&self) -> bool {
        if !TIME_CHECK {
            return false;
        }
        let utc = self.date.naive_utc();
        let dt = Local::now();
        // Get components
        let naive_utc = dt.naive_utc();
        let duration = naive_utc - utc;
        duration.num_weeks() > 4
    }
}

static PLACE_HODER_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\$\{(\w+)\}").unwrap());

static PLACE_ENV_HODER_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\$ENV\{(\w+)\}").unwrap());

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq, Eq)]
pub enum PackageType {
    Dir,
    File,
}
impl std::fmt::Display for PackageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dir => write!(f, "Dir"),
            Self::File => write!(f, "File"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CMakePackageFrom {
    System,
    Vcpkg,
}

impl std::fmt::Display for CMakePackageFrom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vcpkg => write!(f, "Vcpkg"),
            Self::System => write!(f, "System"),
        }
    }
}

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq, Eq)]
pub struct CMakePackage {
    pub name: String,
    pub packagetype: PackageType,
    pub location: Uri,
    pub version: Option<String>,
    pub tojump: Vec<PathBuf>,
    pub from: CMakePackageFrom,
}

pub fn include_is_module(file_name: &str) -> bool {
    !file_name.ends_with(".cmake")
}

pub trait NeoStrExt {
    /// just remote the quotation
    fn remove_quotation(&self) -> &str;

    /// [NeoStrExt::try_replace_placeholders] should run [NeoStrExt::remove_quotation] first, and
    /// try to replace the placeholder, if cannot find the key, it should give up and return None
    fn try_replace_placeholders(&self) -> Option<String>;
}

/// Some extension used in neocmakelsp for str and String
impl NeoStrExt for str {
    fn remove_quotation(&self) -> &str {
        self.trim_matches('"')
    }

    fn try_replace_placeholders(&self) -> Option<String> {
        replace_placeholders(self.remove_quotation())
    }
}

impl NeoStrExt for String {
    fn remove_quotation(&self) -> &str {
        self.trim_matches('"')
    }

    fn try_replace_placeholders(&self) -> Option<String> {
        replace_placeholders(self.remove_quotation())
    }
}

pub fn replace_placeholders(template: &str) -> Option<String> {
    if template.contains("$ENV{") {
        return replace_placeholders_with_env_map(template);
    }
    if !template.contains("${") {
        return Some(template.to_string());
    }
    let values = fileapi::get_entries_data()?;
    replace_placeholders_with_hashmap(template, &values)
}

static CACHE_ENV_DATA: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn replace_placeholders_with_env_map(template: &str) -> Option<String> {
    let mut result = template.to_string();

    let mut cache = CACHE_ENV_DATA.lock().unwrap();
    for caps in PLACE_ENV_HODER_REGEX.captures_iter(template) {
        let key = &caps[1];
        match cache.get(key) {
            Some(value) => {
                result = result.replace(&caps[0], value);
            }
            None => {
                let Ok(value) = std::env::var(key) else {
                    return None;
                };
                result = result.replace(&caps[0], &value);
                cache.insert(key.to_string(), value);
            }
        }
    }
    Some(result)
}

fn replace_placeholders_with_hashmap(
    template: &str,
    values: &HashMap<String, String>,
) -> Option<String> {
    let mut result = template.to_string();

    for caps in PLACE_HODER_REGEX.captures_iter(template) {
        let key = &caps[1];
        let value = values.get(key)?;
        result = result.replace(&caps[0], value);
    }
    Some(result)
}

// FIXME: I do not know the way to gen module_pattern on windows
#[allow(unused_variables)]
#[allow(clippy::unnecessary_wraps)]
pub fn gen_module_pattern(subpath: &str) -> Option<String> {
    #[cfg(unix)]
    #[cfg(not(target_os = "android"))]
    {
        Some(format!("/usr/share/cmake*/Modules/{subpath}.cmake"))
    }
    #[cfg(target_os = "android")]
    {
        let Ok(prefix) = std::env::var("PREFIX") else {
            return None;
        };
        Some(format!("{prefix}/share/cmake*/Modules/{subpath}.cmake"))
    }
    #[cfg(not(unix))]
    {
        let Ok(prefix) = std::env::var("MSYSTEM_PREFIX") else {
            return None;
        };
        Some(format!("{prefix}/share/cmake*/Modules/{subpath}.cmake"))
    }
}

const LIBRARIES_END: &str = "_LIBRARIES";
const INCLUDE_DIRS_END: &str = "_INCLUDE_DIRS";

pub fn get_the_packagename(package: &str) -> &str {
    if let Some(after) = package.strip_suffix(LIBRARIES_END) {
        return after;
    }
    if let Some(after) = package.strip_suffix(INCLUDE_DIRS_END) {
        return after;
    }
    package
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_ismodule() {
        assert!(include_is_module("GNUInstall"));
        assert!(!include_is_module("test.cmake"));
    }

    #[test]
    fn env_arg_test() {
        unsafe {
            std::env::set_var("TempDir", "/tmp");
        }
        assert_eq!(
            "/tmp/wezterm",
            "$ENV{TempDir}/wezterm".try_replace_placeholders().unwrap()
        );
    }

    #[test]
    fn replace_placeholders_test() {
        let mut values = HashMap::new();
        values.insert("ROOT_DIR".to_string(), "/usr".to_string());

        let template = "${ROOT_DIR}/abc";

        assert_eq!(
            replace_placeholders_with_hashmap(template, &values),
            Some("/usr/abc".to_string())
        );

        let template = "/home/abc";
        assert_eq!(
            replace_placeholders_with_hashmap(template, &values),
            Some("/home/abc".to_string())
        );
    }

    #[test]
    fn test_module_pattern() {
        #[cfg(unix)]
        #[cfg(not(target_os = "android"))]
        assert_eq!(
            gen_module_pattern("GNUInstallDirs"),
            Some("/usr/share/cmake*/Modules/GNUInstallDirs.cmake".to_string())
        );
        #[cfg(target_os = "android")]
        {
            unsafe { std::env::set_var("PREFIX", "/data/data/com.termux/files/usr") };
            assert_eq!(
                gen_module_pattern("GNUInstallDirs"),
                Some(
                    "/data/data/com.termux/files/usr/share/cmake*/Modules/GNUInstallDirs.cmake"
                        .to_string()
                )
            );
        }
        #[cfg(not(unix))]
        {
            unsafe { std::env::set_var("MSYSTEM_PREFIX", "C:/msys64") };
            assert_eq!(
                gen_module_pattern("GNUInstallDirs"),
                Some("C:/msys64/share/cmake*/Modules/GNUInstallDirs.cmake".to_string())
            );
        }
    }

    #[test]
    fn package_name_check_test() {
        let package_names = ["abc", "def_LIBRARIES", "ghi_INCLUDE_DIRS"];
        let output: Vec<&str> = package_names
            .iter()
            .map(|name| get_the_packagename(name))
            .collect();
        assert_eq!(output, vec!["abc", "def", "ghi"]);
    }
}
