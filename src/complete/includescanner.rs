use crate::{consts::TREESITTER_CMAKE_LANGUAGE, utils::treehelper::PositionType};

use super::getsubcomplete;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tower_lsp::lsp_types::CompletionItem;

use std::sync::{Arc, Mutex};

use std::sync::LazyLock;

type CacheData = HashMap<PathBuf, Vec<CompletionItem>>;

static PACKAGE_COMPLETE_CACHE: LazyLock<Arc<Mutex<CacheData>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

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
    let content = fs::read_to_string(path).ok()?;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(content.clone(), None)?;
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
    let content = fs::read_to_string(path).ok()?;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&content, None)?;
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

#[cfg(test)]
mod include_scan_test {
    use super::*;
    #[test]
    fn test_compelete_scan_1() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;
        use tower_lsp::lsp_types::{CompletionItemKind, Documentation};

        let file_info_0 = r#"
include(another.cmake)
    "#;
        let file_info_1 = r#"
set(AB "100")
function(bb)
endfunction()
    "#;

        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(file_info_0, None).unwrap();
        let dir = tempdir().unwrap();
        let root_cmake = dir.path().join("CMakeList.txt");
        let mut file = File::create(&root_cmake).unwrap();
        writeln!(file, "{}", file_info_0).unwrap();
        let another_cmake = dir.path().join("another.cmake");
        let mut file_2 = File::create(&another_cmake).unwrap();
        writeln!(file_2, "{}", file_info_1).unwrap();
        let data = getsubcomplete(
            thetree.root_node(),
            &file_info_0.lines().collect(),
            &root_cmake,
            PositionType::VarOrFun,
            None,
            &mut vec![],
            &mut vec![],
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            data,
            vec![
                CompletionItem {
                    label: "AB".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("Value".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined variable\nfrom: {}",
                        another_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
                CompletionItem {
                    label: "bb".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined function\nfrom: {}",
                        another_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                }
            ]
        );
    }

    #[test]
    fn test_compelete_scan_2() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;
        use tower_lsp::lsp_types::{CompletionItemKind, Documentation};

        let file_info_0 = r#"
include("another.cmake")
    "#;
        let file_info_1 = r#"
set(AB "100")
function(bb)
endfunction()
    "#;

        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(file_info_0, None).unwrap();
        let dir = tempdir().unwrap();
        let root_cmake = dir.path().join("CMakeList.txt");
        let mut file = File::create(&root_cmake).unwrap();
        writeln!(file, "{}", file_info_0).unwrap();
        let another_cmake = dir.path().join("another.cmake");
        let mut file_2 = File::create(&another_cmake).unwrap();
        writeln!(file_2, "{}", file_info_1).unwrap();
        let data = getsubcomplete(
            thetree.root_node(),
            &file_info_0.lines().collect(),
            &root_cmake,
            PositionType::VarOrFun,
            None,
            &mut vec![],
            &mut vec![],
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            data,
            vec![
                CompletionItem {
                    label: "AB".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("Value".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined variable\nfrom: {}",
                        another_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
                CompletionItem {
                    label: "bb".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined function\nfrom: {}",
                        another_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                }
            ]
        );
    }
}
