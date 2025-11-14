use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use lsp_types::Uri;
use tower_lsp::lsp_types;

use super::{CacheDataUnit, Location, gen_module_pattern, getsubdef};
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::include_is_module;
use crate::utils::treehelper::PositionType;

pub(super) fn cmpinclude<P: AsRef<Path>>(localpath: P, subpath: &str) -> Option<Vec<Location>> {
    let target = if !include_is_module(subpath) {
        let root_dir = localpath.as_ref().parent()?;
        root_dir.join(subpath)
    } else {
        let glob_pattern = gen_module_pattern(subpath)?;
        glob::glob(glob_pattern.as_str())
            .into_iter()
            .flatten()
            .flatten()
            .next()?
    };

    if target.exists() {
        Some(vec![Location {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            uri: Uri::from_file_path(target).unwrap(),
        }])
    } else {
        None
    }
}

#[test]
fn tst_cmp_included_cmake() {
    use std::fs::File;

    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let top_cmake = dir.path().join("CMakeLists.txt");
    File::create_new(&top_cmake).unwrap();
    let include_cmake = dir.path().join("abcd_test.cmake");
    File::create_new(&include_cmake).unwrap();

    let locations = cmpinclude(&top_cmake, "abcd_test.cmake").unwrap();

    assert_eq!(
        locations,
        vec![Location {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            uri: Uri::from_file_path(include_cmake).unwrap(),
        }]
    );
}

type CacheData = HashMap<PathBuf, Vec<CacheDataUnit>>;

static PACKAGE_COMPLETE_CACHE: LazyLock<Arc<Mutex<CacheData>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn scanner_include_defs(
    path: &PathBuf,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    find_cmake_in_package: bool,
    is_builtin: bool,
) -> Option<Vec<CacheDataUnit>> {
    if is_builtin
        && let Ok(cache) = PACKAGE_COMPLETE_CACHE.lock()
        && let Some(complete_items) = cache.get(path)
    {
        return Some(complete_items.clone());
    }
    let content = fs::read_to_string(path).ok()?;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(&content, None)?;
    let result_data = getsubdef(
        thetree.root_node(),
        &content.lines().collect(),
        path,
        postype,
        include_files,
        complete_packages,
        true,
        find_cmake_in_package,
    );
    if !is_builtin {
        return result_data;
    }
    if let Some(ref content) = result_data
        && let Ok(mut cache) = PACKAGE_COMPLETE_CACHE.lock()
    {
        cache.insert(path.clone(), content.clone());
    }
    result_data
}

#[test]
fn scanner_include_defs_tst() {
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let top_cmake = dir.path().join("CMakeLists.txt");

    let mut cmake_file = File::create_new(&top_cmake).unwrap();
    let top_cmake_context = r#"
include(abcd_test.cmake)
"#;
    writeln!(cmake_file, "{}", top_cmake_context).unwrap();
    let include_cmake_path = dir.path().join("abcd_test.cmake");
    let mut include_cmake = File::create_new(&include_cmake_path).unwrap();
    let include_cmake_context = r#"
set(ABCD "abcd")
include(efg_test.cmake)
"#;
    writeln!(include_cmake, "{}", include_cmake_context).unwrap();

    // NOTE: this is used to test if the include cache append will work
    let include_cmake_path_2 = dir.path().join("efg_test.cmake");
    File::create(&include_cmake_path_2).unwrap();

    let mut include_files = vec![];
    let data = scanner_include_defs(
        &include_cmake_path,
        PositionType::VarOrFun,
        &mut include_files,
        &mut vec![],
        false,
        false,
    )
    .unwrap();

    assert_eq!(
        data,
        vec![CacheDataUnit {
            key: "ABCD".to_string(),
            location: Location {
                uri: Uri::from_file_path(&include_cmake_path).unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 1,
                        character: 4
                    },
                    end: lsp_types::Position {
                        line: 1,
                        character: 8
                    }
                }
            },
            document_info: format!("defined variable\nfrom: {}", include_cmake_path.display()),
            is_function: false
        }]
    );
    assert_eq!(include_files, vec![include_cmake_path_2]);
}

pub fn scanner_package_defs(
    path: &PathBuf,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CacheDataUnit>> {
    if let Ok(cache) = PACKAGE_COMPLETE_CACHE.lock()
        && let Some(complete_items) = cache.get(path)
    {
        return Some(complete_items.clone());
    }
    let content = fs::read_to_string(path).ok()?;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(&content, None)?;
    let result_data = getsubdef(
        thetree.root_node(),
        &content.lines().collect(),
        path,
        postype,
        include_files,
        complete_packages,
        false,
        true,
    );
    if let Some(ref content) = result_data
        && let Ok(mut cache) = PACKAGE_COMPLETE_CACHE.lock()
    {
        cache.insert(path.clone(), content.clone());
    }
    result_data
}
