mod findpackage;
pub mod treehelper;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::Uri;
use tree_sitter::Node;

pub use self::findpackage::*;
use crate::fileapi;

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
            PackageType::Dir => write!(f, "Dir"),
            PackageType::File => write!(f, "File"),
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

// get the content and split all argument to vector
pub fn get_node_content<'a>(source: &[&'a str], node: &Node) -> Vec<&'a str> {
    let mut content: Vec<&str> = vec![];
    let x = node.start_position().column;
    let y = node.end_position().column;

    let row_start = node.start_position().row;
    let row_end = node.end_position().row;

    if row_start == row_end {
        let tmpcontent = &source[row_start][x..y];
        content.append(&mut tmpcontent.split(' ').collect());
    } else {
        let mut row = row_start;
        content.append(&mut source[row][x..].split(' ').collect());
        row += 1;

        while row < row_end {
            content.append(&mut source[row].split(' ').collect());
            row += 1;
        }

        if row != row_start {
            assert_eq!(row, row_end);
            // NOTE: like ")" this kind should check again
            if y != 0 {
                content.append(&mut source[row][..y].split(' ').collect());
            }
        }
    }
    content
}

pub fn remove_quotation_and_replace_placeholders(origin_template: &str) -> Option<String> {
    replace_placeholders(origin_template.trim_matches('"'))
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
        match values.get(key) {
            Some(value) => {
                result = result.replace(&caps[0], value);
            }
            None => return None,
        }
    }
    Some(result)
}

// FIXME: I do not know the way to gen module_pattern on windows
#[allow(unused_variables)]
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

#[derive(Debug)]
pub struct LineCommentTmp<'a> {
    pub end_y: usize,
    pub comments: Vec<&'a str>,
}

impl LineCommentTmp<'_> {
    pub fn is_node_comment(&self, start_y: usize) -> bool {
        if start_y <= self.end_y {
            return false;
        }
        start_y - self.end_y == 1 && !self.comments.is_empty()
    }
    pub fn comment(&self) -> String {
        let tmp: Vec<&str> = self
            .comments
            .iter()
            .map(|comment| comment.strip_prefix("#").unwrap_or(comment).trim())
            .collect();
        tmp.join("\n")
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
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;

    #[test]
    fn ut_ismodule() {
        assert!(include_is_module("GNUInstall"));
        assert!(!include_is_module("test.cmake"));
    }

    #[test]
    fn get_node_content_test_1() {
        let source = r#"findpackage(Qt5 COMPONENTS CONFIG Core Gui Widget)"#;
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let tree = parse.parse(source, None).unwrap();
        let input = tree.root_node();
        let argumentlist = input.child(0).unwrap().child(2).unwrap();
        let lines: Vec<&str> = source.lines().collect();
        let content = get_node_content(&lines, &argumentlist);
        assert_eq!(
            content,
            vec!["Qt5", "COMPONENTS", "CONFIG", "Core", "Gui", "Widget"]
        );
    }

    #[test]
    fn get_node_content_test_2() {
        let source = r#"findpackage(Qt5
COMPONENTS CONFIG
Core Gui Widget
)"#;
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let tree = parse.parse(source, None).unwrap();
        let input = tree.root_node();
        let argumentlist = input.child(0).unwrap().child(2).unwrap();
        let lines: Vec<&str> = source.lines().collect();
        let content = get_node_content(&lines, &argumentlist);
        assert_eq!(
            content,
            vec!["Qt5", "COMPONENTS", "CONFIG", "Core", "Gui", "Widget"]
        );
    }

    #[test]
    fn env_arg_test() {
        unsafe {
            std::env::set_var("TempDir", "/tmp");
        }
        assert_eq!(
            "/tmp/wezterm",
            replace_placeholders("$ENV{TempDir}/wezterm").unwrap()
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
    fn test_comment() {
        let linecomment = LineCommentTmp {
            end_y: 0,
            comments: vec!["# Abcd", "#   EFGH"],
        };
        assert_eq!(linecomment.comment(), "Abcd\nEFGH");
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
