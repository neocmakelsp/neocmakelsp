use std::path::{Path, PathBuf};

use dashmap::DashMap;
use tower_lsp::lsp_types::{Location, MessageType, Position, Uri};

use crate::complete::{COMPLETION_CACHE, PCacheInfo, update_cache};
use crate::languageserver::{get_extra_files, get_or_update_buffer_contents};
/// provide go to definition
use crate::{
    consts::TREESITTER_CMAKE_LANGUAGE,
    scansubs::TREE_MAP,
    utils::{
        gen_module_pattern, get_the_packagename, replace_placeholders,
        treehelper::{ToPoint, ToPosition},
    },
};
mod findpackage;
mod include;
mod subdirectory;

use crate::utils::treehelper::{CurrentNodeInfo, PositionType};

pub async fn get_def<P: AsRef<Path>>(
    path: P,
    key: &str,
    location: impl Into<Option<Position>>,
    documents: &DashMap<Uri, String>,
) -> Option<PCacheInfo> {
    let location = location.into().unwrap_or(Position {
        line: u32::MAX,
        character: u32::MAX,
    });
    let mut path = path.as_ref().to_path_buf();

    let extra_files = get_extra_files();
    let tree_map = TREE_MAP.lock().await;

    enum CacheState {
        Continue,
        Break(Box<PCacheInfo>),
    }

    let update_cache_fn = async |file_path: &PathBuf, enter_file: bool| {
        let completion_cache = COMPLETION_CACHE.lock().await;
        if let Some(units) = completion_cache.get(file_path)
            && let Some(cache) = units.get(key)
            && cache.is_definition()
            && (!enter_file || cache.range().end <= location)
        {
            return CacheState::Break(Box::new(cache.clone()));
        }
        drop(completion_cache);
        if let Ok(context) = get_or_update_buffer_contents(&file_path, documents).await {
            update_cache(&file_path, context.as_str()).await;
            let completion_cache = COMPLETION_CACHE.lock().await;
            if let Some(units) = completion_cache.get(file_path)
                && let Some(cache) = units.get(key)
                && cache.is_definition()
                && (!enter_file || cache.range().end < location)
            {
                return CacheState::Break(Box::new(cache.clone()));
            }
        }
        CacheState::Continue
    };
    for file in &extra_files {
        let is_original = *file == path;
        if let CacheState::Break(info) = update_cache_fn(file, is_original).await {
            return Some(*info);
        }
    }

    // NOTE: search self first
    match update_cache_fn(&path, true).await {
        CacheState::Continue => {}
        CacheState::Break(info) => return Some(*info),
    }

    // NOTE: then search parent
    while let Some(parent) = tree_map.get(&path) {
        match update_cache_fn(parent, false).await {
            CacheState::Continue => {
                path.clone_from(parent);
            }
            CacheState::Break(info) => return Some(*info),
        }
    }

    None
}
pub async fn get_reference<P: AsRef<Path>>(
    path: P,
    key: &str,
    documents: &DashMap<Uri, String>,
) -> Vec<PCacheInfo> {
    let mut path = path.as_ref().to_path_buf();

    let extra_files = get_extra_files();
    let tree_map = TREE_MAP.lock().await;

    let mut results = Vec::new();

    let mut update_cache_fn = async |file_path: &PathBuf| {
        let completion_cache = COMPLETION_CACHE.lock().await;
        if let Some(units) = completion_cache.get(file_path)
            && let Some(cache) = units.get(key)
        {
            results.push(cache.clone());
        }
        drop(completion_cache);
        if let Ok(context) = get_or_update_buffer_contents(&file_path, documents).await {
            update_cache(&file_path, context.as_str()).await;
            let completion_cache = COMPLETION_CACHE.lock().await;
            if let Some(units) = completion_cache.get(file_path)
                && let Some(cache) = units.get(key)
            {
                results.push(cache.clone());
            }
        }
    };
    let mut self_searched = false;
    for file in &extra_files {
        let is_original = *file == path;
        if is_original {
            self_searched = true;
        }
        update_cache_fn(file).await;
    }

    // NOTE: search self first
    if !self_searched {
        update_cache_fn(&path).await;
    }

    // NOTE: then search parent
    while let Some(parent) = tree_map.get(&path) {
        update_cache_fn(parent).await;
        path.clone_from(parent);
    }

    results
}

/// find the definition
pub async fn godef<P: AsRef<Path>>(
    location: Position,
    source: &str,
    originuri: P,
    client: &tower_lsp::Client,
    is_jump: bool,
    just_var_or_fun: bool,
    documents: &DashMap<Uri, String>,
) -> Option<Vec<Location>> {
    let current_point = location.to_point();
    let locations = godef_inner(
        current_point,
        source,
        originuri,
        is_jump,
        just_var_or_fun,
        documents,
    )
    .await;
    if locations.is_none() {
        client
            .log_message(MessageType::Info, "Not find any locations")
            .await;
    }
    locations
}

async fn godef_inner<P: AsRef<Path>>(
    location: tree_sitter::Point,
    source: &str,
    originuri: P,
    is_jump: bool,
    just_var_or_fun: bool,
    documents: &DashMap<Uri, String>,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None)?;

    let current_node_info = CurrentNodeInfo::get(source, tree.root_node(), location);
    let jumptype = current_node_info.pos_type();
    let tofind = current_node_info.content()?;

    // NOTE: when just find the var or fun, then we need to skip other position type
    // Because when value in arguments, then it maybe definition, so we also need to handle this
    // part
    if !matches!(
        jumptype,
        PositionType::VarOrFun | PositionType::FunOrMacroIdentifier
    ) && just_var_or_fun
    {
        return None;
    }

    match jumptype {
        PositionType::VarOrFun | PositionType::FunOrMacroIdentifier => {
            if is_jump {
                let location = get_def(originuri, tofind, location.to_position(), documents)
                    .await
                    .and_then(|info| info.location())?;
                Some(vec![location])
            } else {
                let infos = get_reference(originuri, tofind, documents).await;
                let location_origins: Vec<Location> = infos
                    .into_iter()
                    .map(|info| info.references)
                    .flatten()
                    .collect();
                let mut locations = vec![];
                for loc in location_origins {
                    if locations.contains(&loc) {
                        continue;
                    }
                    locations.push(loc);
                }
                Some(locations)
            }
        }
        PositionType::FindPackageSpace(space) => {
            let newtofind = format!("{space}{}", get_the_packagename(tofind));
            findpackage::cmpfindpackage(&newtofind)
        }
        PositionType::FindPackage | PositionType::TargetLink | PositionType::TargetInclude => {
            let tofind = get_the_packagename(tofind);
            findpackage::cmpfindpackage(tofind)
        }
        // NOTE: here is reserve to do next time
        PositionType::Comment | PositionType::FunOrMacroArgs => None,
        #[cfg(unix)]
        PositionType::FindPkgConfig => None,
        PositionType::Target => None,
        PositionType::Include => {
            let fixed_url = replace_placeholders(tofind)?;
            include::cmpinclude(originuri, &fixed_url)
        }
        PositionType::SubDir => {
            let fixed_url = replace_placeholders(tofind)?;
            subdirectory::cmpsubdirectory(originuri, &fixed_url)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;
    use tower_lsp::lsp_types;
    use tree_sitter::Point;

    use super::*;

    #[tokio::test]
    async fn test_jump_subdir() {
        let jump_file_src = r"add_subdirectory(abcd_test)";

        let dir = tempdir().unwrap();
        let top_cmake = dir.path().join("CMakeLists.txt");
        let mut top_file = File::create_new(&top_cmake).unwrap();
        top_file.write_all(jump_file_src.as_bytes()).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();

        let locations = godef_inner(
            Point { row: 0, column: 20 },
            jump_file_src,
            &top_cmake,
            true,
            false,
            &DashMap::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            locations,
            vec![Location {
                uri: Uri::from_file_path(subdir_file).unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                }
            }]
        );
    }

    #[tokio::test]
    async fn test_jump_variable() {
        let jump_file_src = r#"
set(ABCD 1234)
message(INFO "${ABCD}")
set(ROOT_DIR "ABCD" STRING CACHE "ROOTDIR")
include("${ROOT_DIR}/abcd_test")
add_subdirectory(abcd_test)
"#;

        let dir = tempdir().unwrap();
        let top_cmake = dir.path().join("CMakeLists.txt");
        let mut top_file = File::create_new(&top_cmake).unwrap();
        top_file.write_all(jump_file_src.as_bytes()).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();

        let locations = godef_inner(
            Point { row: 2, column: 18 },
            jump_file_src,
            &top_cmake,
            true,
            false,
            &DashMap::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            locations,
            vec![Location {
                uri: Uri::from_file_path(&top_cmake).unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 1,
                        character: 4,
                    },
                    end: lsp_types::Position {
                        line: 1,
                        character: 8,
                    },
                }
            }]
        );
        let locations_2 = godef_inner(
            Point { row: 4, column: 13 },
            jump_file_src,
            &top_cmake,
            true,
            false,
            &DashMap::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            locations_2,
            vec![Location {
                uri: Uri::from_file_path(top_cmake).unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 3,
                        character: 4,
                    },
                    end: lsp_types::Position {
                        line: 3,
                        character: 12,
                    },
                }
            }]
        );
    }
}
