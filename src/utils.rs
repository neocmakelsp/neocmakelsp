mod findpackage;
pub mod treehelper;
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use serde::{Deserialize, Serialize};

static PLACE_HODER_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\$\{(\w+)\}").unwrap());

#[derive(Deserialize, Debug, Serialize, Clone)]
pub enum FileType {
    Dir,
    File,
}
impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Dir => write!(f, "Dir"),
            FileType::File => write!(f, "File"),
        }
    }
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct CMakePackage {
    pub name: String,
    pub filetype: FileType,
    pub filepath: String,
    pub version: Option<String>,
    pub tojump: Vec<PathBuf>,
    pub from: String,
}

pub use findpackage::*;
use tree_sitter::Node;

use crate::fileapi;

pub fn get_node_content(source: &[&str], node: &Node) -> String {
    let mut content: String;
    let x = node.start_position().column;
    let y = node.end_position().column;

    let row_start = node.start_position().row;
    let row_end = node.end_position().row;

    if row_start == row_end {
        content = source[row_start][x..y].to_string();
    } else {
        let mut row = row_start;
        content = source[row][x..].to_string();
        row += 1;

        while row < row_end {
            content = format!("{} {}", content, source[row]);
            row += 1;
        }

        if row != row_start {
            assert_eq!(row, row_end);
            content = format!("{} {}", content, &source[row][..y])
        }
    }
    content
}

pub fn remove_bracked(origin: &str) -> &str {
    if origin.starts_with("\"") {
        return &origin[1..origin.len() - 1];
    }
    origin
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
    assert_eq!(remove_bracked(testa), target_str);
    assert_eq!(remove_bracked(target_str), target_str);
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
