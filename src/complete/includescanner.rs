use crate::utils::treehelper::PositionType;

use super::getsubcomplete;
use lsp_types::CompletionItem;
use std::fs;
use std::path::PathBuf;
pub fn scanner_include_complete(
    path: &PathBuf,
    postype: PositionType,
    complete_packages: &mut Vec<String>,
    find_cmake_in_package: bool,
) -> Option<Vec<CompletionItem>> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(tree_sitter_cmake::language()).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            getsubcomplete(
                tree.root_node(),
                content.as_str(),
                path,
                postype,
                None,
                complete_packages,
                true,
                find_cmake_in_package
            )
        }
        Err(_) => None,
    }
}

pub fn scanner_package_complete(
    path: &PathBuf,
    postype: PositionType,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CompletionItem>> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(tree_sitter_cmake::language()).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            getsubcomplete(
                tree.root_node(),
                content.as_str(),
                path,
                postype,
                None,
                complete_packages,
                false,
                true
            )
        }
        Err(_) => None,
    }
}
