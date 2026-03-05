use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::remove_quotation_and_replace_placeholders;
use crate::utils::treehelper::get_normal_commands;
use crate::{complete, jump};

/// NOTE: key is be included path, value is the top CMakeLists
/// This is used to find who is on the top of the CMakeLists
pub type TreeKey = HashMap<PathBuf, PathBuf>;
pub type TreeCMakeKey = HashMap<PathBuf, Vec<PathBuf>>;

// NOTE: here get the struct of the tree
// Cache the data of the struct
// Key is the child, value is the parent
pub static TREE_MAP: LazyLock<Arc<Mutex<TreeKey>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

// NOTE: record who is using the cmake file
// Key is the cmake file, value is the place using it
pub static TREE_CMAKE_MAP: LazyLock<Arc<Mutex<TreeCMakeKey>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub async fn scan_all<P: AsRef<Path>>(project_root: P, is_first: bool) {
    let root_cmake = project_root.as_ref().join("CMakeLists.txt");
    let mut to_scan: Vec<PathBuf> = vec![root_cmake];
    while !to_scan.is_empty() {
        let mut next_to_scan = Vec::new();
        for scan_cmake in to_scan.iter() {
            let mut out = scan_dir(scan_cmake, is_first).await;
            next_to_scan.append(&mut out);
        }
        to_scan = next_to_scan;
    }
}

pub async fn scan_dir<P: AsRef<Path>>(path: P, is_first: bool) -> Vec<PathBuf> {
    let (bufs, cmakebufs) = scan_dir_inner(path.as_ref(), is_first).await;
    let mut tree = TREE_MAP.lock().await;
    let mut includetree = TREE_CMAKE_MAP.lock().await;
    for subpath in bufs.iter() {
        tree.insert(subpath.to_path_buf(), path.as_ref().into());
    }
    for cmakepath in cmakebufs {
        let include_key = includetree.entry(cmakepath).or_default();
        let toaddpath = path.as_ref().into();
        if !include_key.contains(&toaddpath) {
            include_key.push(toaddpath);
        }
    }
    bufs
}

async fn scan_dir_inner<P: AsRef<Path>>(path: P, is_first: bool) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let Ok(source) = tokio::fs::read_to_string(path.as_ref()).await else {
        return (Vec::new(), Vec::new());
    };

    if is_first {
        complete::update_cache(path.as_ref(), &source).await;
        jump::update_cache(path.as_ref(), &source).await;
    }
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&source, None).unwrap();
    let tree = tree.root_node();
    if tree.is_error() {
        return (Vec::new(), Vec::new());
    }

    scan_node(&source, tree, path)
}

fn scan_node<P: AsRef<Path>>(
    source: &str,
    tree: tree_sitter::Node,
    path: P,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut bufs = Vec::new();
    let mut cmake_bufs = Vec::new();
    let normal_commands = get_normal_commands(source.as_bytes(), tree, None);
    for command in normal_commands {
        let command_name = command.identifier.to_lowercase();
        if command_name == "add_subdirectory" {
            let Some(first_arg) = command.first_arg else {
                continue;
            };
            let Some(file_name) = remove_quotation_and_replace_placeholders(first_arg) else {
                continue;
            };

            let subpath = path
                .as_ref()
                .parent()
                .unwrap()
                .join(file_name)
                .join("CMakeLists.txt");
            bufs.push(subpath.to_path_buf());
        } else if command_name == "include" {
            let Some(first_arg) = command.first_arg else {
                continue;
            };
            let Some(file_name) = remove_quotation_and_replace_placeholders(first_arg) else {
                continue;
            };

            if !file_name.ends_with(".cmake") {
                continue;
            }
            let mut cmake_buf_path = PathBuf::from(file_name);

            if !cmake_buf_path.is_absolute() {
                cmake_buf_path = path.as_ref().parent().unwrap().join(cmake_buf_path);
            }
            cmake_bufs.push(cmake_buf_path);
        }
    }
    (bufs, cmake_bufs)
}

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq, Eq)]
pub struct TreeDir {
    current_file: PathBuf,
    subdirs: Option<Vec<TreeDir>>,
}

impl fmt::Display for TreeDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.subdirs {
            None => writeln!(f, "{}", self.current_file.display()),
            Some(dirs) => {
                writeln!(f, "{}", self.current_file.display())?;
                for dir in dirs {
                    let message = dir.to_string();
                    for mes in message.lines() {
                        writeln!(f, "  -> {mes}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

// Path Input is xxx/CMakeLists.txt
pub fn get_treedir(path: &Path) -> Option<TreeDir> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return None;
    };
    let mut top = TreeDir {
        current_file: path.into(),
        subdirs: None,
    };
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&content, None).unwrap();
    let subdirs = get_subdir_from_tree(&content, tree.root_node(), path);
    if !subdirs.is_empty() {
        let mut sub_dirs: Vec<TreeDir> = vec![];
        for dir in subdirs {
            if let Some(treedir) = get_treedir(&dir) {
                sub_dirs.push(treedir);
            }
        }
        if !sub_dirs.is_empty() {
            top.subdirs = Some(sub_dirs);
        }
    }
    Some(top)
}

fn get_subdir_from_tree(source: &str, tree: tree_sitter::Node, parent: &Path) -> Vec<PathBuf> {
    if tree.is_error() {
        return vec![];
    }
    let mut output = vec![];
    let normal_commands = get_normal_commands(source.as_bytes(), tree, None);
    for command in normal_commands {
        if command.identifier.to_lowercase() == "add_subdirectory" {
            let Some(first_arg) = command.first_arg else {
                continue;
            };
            let Some(file_name) = remove_quotation_and_replace_placeholders(first_arg) else {
                continue;
            };

            let subpath = parent
                .parent()
                .unwrap()
                .join(file_name)
                .join("CMakeLists.txt");
            if subpath.exists() {
                output.push(subpath);
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_scan_sub() {
        let dir = tempdir().unwrap();
        let top_cmake = dir.path().join("CMakeLists.txt");
        let mut top_file = File::create_new(&top_cmake).unwrap();
        writeln!(top_file, r#"add_subdirectory("abcd_test")"#).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();
        let bufs = scan_dir(&top_cmake, false).await;
        assert_eq!(bufs, vec![subdir_file.clone()]);
        let cache_data = TREE_MAP.lock().await;
        assert_eq!(*cache_data, HashMap::from_iter([(subdir_file, top_cmake)]));
    }

    #[test]
    fn test_tree_dir() {
        let dir = tempdir().unwrap();
        let top_cmake = dir.path().join("CMakeLists.txt");
        let mut top_file = File::create_new(&top_cmake).unwrap();
        writeln!(top_file, r#"add_subdirectory("abcd_test")"#).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();
        let tree_dir = get_treedir(&top_cmake).unwrap();
        assert_eq!(
            tree_dir,
            TreeDir {
                current_file: top_cmake,
                subdirs: Some(vec![TreeDir {
                    current_file: subdir_file,
                    subdirs: None,
                }])
            }
        );
    }
}
