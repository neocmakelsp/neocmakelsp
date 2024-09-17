mod findpackage;
pub mod treehelper;
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use crate::Url;
use serde::{Deserialize, Serialize};

static PLACE_HODER_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\$\{(\w+)\}").unwrap());

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
    pub location: Url,
    pub version: Option<String>,
    pub tojump: Vec<PathBuf>,
    pub from: CMakePackageFrom,
}

pub use findpackage::*;
use tree_sitter::Node;

use crate::fileapi;

pub fn include_is_module(file_name: &str) -> bool {
    !file_name.ends_with(".cmake")
}

#[test]
fn ut_ismodule() {
    assert_eq!(include_is_module("GNUInstall"), true);
    assert_eq!(include_is_module("test.cmake"), false);
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
            content.append(&mut source[row][..y].split(' ').collect())
        }
    }
    content
}

#[test]
fn get_node_content_tst() {
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;
    let source = r#"findpackage(Qt5 COMPONENTS CONFIG Core Gui Widget)"#;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&source, None).unwrap();
    let input = tree.root_node();
    let argumentlist = input.child(0).unwrap().child(2).unwrap();
    let lines: Vec<&str> = source.lines().collect();
    let content = get_node_content(&lines, &argumentlist);
    assert_eq!(
        content,
        vec!["Qt5", "COMPONENTS", "CONFIG", "Core", "Gui", "Widget"]
    );
}

pub fn remove_quotation_and_replace_placeholders(origin_template: &str) -> Option<String> {
    replace_placeholders(remove_quotation(origin_template))
}

pub fn remove_quotation(origin: &str) -> &str {
    origin
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(origin)
}

pub fn replace_placeholders(template: &str) -> Option<String> {
    if !template.contains("${") {
        return Some(template.to_string());
    }
    let values = fileapi::get_entries_data()?;
    replace_placeholders_with_hashmap(template, &values)
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

#[test]
fn brank_remove_test() {
    let testa = "\"abc\"";
    let target_str = "abc";
    assert_eq!(remove_quotation(testa), target_str);
    assert_eq!(remove_quotation(target_str), target_str);
}

#[test]
fn replace_placeholders_tst() {
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
    pub start_y: usize,
    pub comment: &'a str,
}

impl<'a> LineCommentTmp<'a> {
    pub fn is_node_comment(&self, start_y: usize) -> bool {
        if start_y <= self.start_y {
            return false;
        }
        start_y - self.start_y == 1 && !self.comment.is_empty()
    }
    pub fn comment(&self) -> &str {
        self.comment[1..].trim_start()
    }
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
        std::env::set_var("PREFIX", "/data/data/com.termux/files/usr");
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
        std::env::set_var("MSYSTEM_PREFIX", "C:/msys64");
        assert_eq!(
            gen_module_pattern("GNUInstallDirs"),
            Some("C:/msys64/share/cmake*/Modules/GNUInstallDirs.cmake".to_string())
        );
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

#[test]
fn package_name_check_tst() {
    let package_names = vec!["abc", "def_LIBRARIES", "ghi_INCLUDE_DIRS"];
    let output: Vec<&str> = package_names
        .iter()
        .map(|name| get_the_packagename(name))
        .collect();
    assert_eq!(output, vec!["abc", "def", "ghi"]);
}
