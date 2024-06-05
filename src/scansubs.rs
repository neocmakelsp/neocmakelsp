use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// NOTE: key is be included path, value is the top CMakeLists
/// This is used to find who is on the top of the CMakeLists
pub type TreeKey = HashMap<PathBuf, PathBuf>;

// NOTE: here get the struct of the tree
// Cache the data of the struct
pub static TREE_MAP: Lazy<Arc<Mutex<TreeKey>>> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub async fn scan_all<P: AsRef<Path>>(project_root: P) {
    let root_cmake = project_root.as_ref().join("CMakeLists.txt");
    let mut to_scan: Vec<PathBuf> = vec![root_cmake];
    while !to_scan.is_empty() {
        let mut next_to_scan = Vec::new();
        for scan_cmake in to_scan.iter() {
            let mut out = scan_dir(scan_cmake).await;
            next_to_scan.append(&mut out);
        }
        to_scan = next_to_scan;
    }
}

pub async fn scan_dir<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let bufs = scan_dir_inner(path.as_ref());
    let mut tree = TREE_MAP.lock().await;
    for subpath in bufs.iter() {
        tree.insert(subpath.to_path_buf(), path.as_ref().into());
    }
    bufs
}

pub fn scan_dir_inner<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    let Ok(source) = std::fs::read_to_string(path.as_ref()) else {
        return Vec::new();
    };

    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(&source, None).unwrap();
    let tree = tree.root_node();
    let newsource: Vec<&str> = source.lines().collect();
    if tree.is_error() {
        return Vec::new();
    }
    scan_node(&newsource, tree, path)
}

fn scan_node<P: AsRef<Path>>(source: &Vec<&str>, tree: tree_sitter::Node, path: P) -> Vec<PathBuf> {
    let mut bufs = Vec::new();
    let mut course = tree.walk();
    for node in tree.children(&mut course) {
        match node.kind() {
            "normal_command" => {
                let h = node.start_position().row;
                let ids = node.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let command_name = &source[h][x..y];
                if command_name.to_lowercase() == "add_subdirectory" && node.child_count() >= 4 {
                    let ids = node.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &source[h][x..y];
                        let name = remove_quotation(name);
                        let subpath = path
                            .as_ref()
                            .parent()
                            .unwrap()
                            .join(name)
                            .join("CMakeLists.txt");
                        bufs.push(subpath.to_path_buf())
                    }
                }
            }
            "if_condition" | "foreach_loop" | "body" => {
                bufs.append(&mut scan_node(source, node, path.as_ref()));
            }
            _ => {}
        }
    }
    bufs
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct TreeDir {
    dir: PathBuf,
    subdirs: Option<Vec<TreeDir>>,
}

impl fmt::Display for TreeDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.subdirs {
            None => writeln!(f, "{}", self.dir.display()),
            Some(dirs) => {
                writeln!(f, "{}", self.dir.display())?;
                for dir in dirs {
                    let message = dir.to_string();
                    for mes in message.lines() {
                        writeln!(f, "  -> {mes}")?
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
        dir: path.into(),
        subdirs: None,
    };
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&tree_sitter_cmake::language()).unwrap();
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
    let newsource: Vec<&str> = source.lines().collect();
    if tree.is_error() {
        vec![]
    } else {
        let mut course = tree.walk();
        let mut output = vec![];
        for node in tree.children(&mut course) {
            let mut innodepath = get_subdir_from_tree(source, node, parent);
            if !innodepath.is_empty() {
                output.append(&mut innodepath);
            }
            if node.kind() == "normal_command" {
                let h = node.start_position().row;
                let ids = node.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let command_name = &newsource[h][x..y];
                if command_name.to_lowercase() == "add_subdirectory" && node.child_count() >= 4 {
                    let ids = node.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        let name = remove_quotation(name);
                        let subpath = parent.parent().unwrap().join(name).join("CMakeLists.txt");
                        if subpath.exists() {
                            output.push(subpath);
                        }
                    }
                }
            }
        }
        output
    }
}

fn remove_quotation(input: &str) -> &str {
    if input.split('"').count() == 3 {
        input.split('"').collect::<Vec<&str>>()[1]
    } else {
        input
    }
}

#[test]
fn tst_quotantion() {
    let a = r#"
    "aa"
    "#;
    assert_eq!("aa", remove_quotation(a));

    let b = "sdfds";
    assert_eq!(b, "sdfds");
}
