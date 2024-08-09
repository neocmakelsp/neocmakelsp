mod findpackage;
pub mod treehelper;
use std::path::PathBuf;

//use anyhow::Result;
//use once_cell::sync::Lazy;
//use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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
