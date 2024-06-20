use crate::{consts::TREESITTER_CMAKE_LANGUAGE, utils::treehelper::PositionType};

use super::getsubcomplete;
use lsp_types::CompletionItem;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::lsp_types;

use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

type CacheData = HashMap<PathBuf, Vec<CompletionItem>>;

static PACKAGE_COMPLETE_CACHE: Lazy<Arc<Mutex<CacheData>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn scanner_include_complete(
    path: &PathBuf,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    find_cmake_in_package: bool,
    is_buildin: bool,
) -> Option<Vec<CompletionItem>> {
    if is_buildin {
        if let Ok(cache) = PACKAGE_COMPLETE_CACHE.lock() {
            if let Some(complete_items) = cache.get(path) {
                return Some(complete_items.clone());
            }
        }
    }
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            let result_data = getsubcomplete(
                tree.root_node(),
                &content.lines().collect(),
                path,
                postype,
                None,
                include_files,
                complete_packages,
                true,
                find_cmake_in_package,
            );
            if !is_buildin {
                return result_data;
            }
            if let Some(ref content) = result_data {
                if let Ok(mut cache) = PACKAGE_COMPLETE_CACHE.lock() {
                    cache.insert(path.clone(), content.clone());
                }
            }
            result_data
        }
        Err(_) => None,
    }
}

pub fn scanner_package_complete(
    path: &PathBuf,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CompletionItem>> {
    if let Ok(cache) = PACKAGE_COMPLETE_CACHE.lock() {
        if let Some(complete_items) = cache.get(path) {
            return Some(complete_items.clone());
        }
    }
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            let result_data = getsubcomplete(
                tree.root_node(),
                &content.lines().collect(),
                path,
                postype,
                None,
                include_files,
                complete_packages,
                false,
                true,
            );
            if let Some(ref content) = result_data {
                if let Ok(mut cache) = PACKAGE_COMPLETE_CACHE.lock() {
                    cache.insert(path.clone(), content.clone());
                }
            }
            result_data
        }
        Err(_) => None,
    }
}
