use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

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
                        writeln!(f, "  -> {}", mes)?
                    }
                }
                Ok(())
            }
        }
    }
}

// Path Input is xxx/CMakeLists.txt
pub fn get_treedir(path: &Path) -> Option<TreeDir> {
    if let Ok(content) = std::fs::read_to_string(path) {
        let mut top = TreeDir {
            dir: path.into(),
            subdirs: None,
        };
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(tree_sitter_cmake::language()).unwrap();
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
    } else {
        None
    }
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
