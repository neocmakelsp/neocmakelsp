mod findpackage;
pub mod treehelper;
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
    pub tojump: Vec<String>,
}

pub use findpackage::*;
