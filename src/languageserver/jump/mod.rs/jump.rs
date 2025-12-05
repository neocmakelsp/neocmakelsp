use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{Location, MessageType, Position, Range, Uri};

use crate::document::Document;
use crate::languageserver::get_or_update_buffer_contents;
use crate::scansubs::TREE_CMAKE_MAP;
use crate::utils::remove_quotation_and_replace_placeholders;
/// provide go to definition
use crate::{
    CMakeNodeKinds,
    consts::TREESITTER_CMAKE_LANGUAGE,
    scansubs::TREE_MAP,
    utils::{
        CACHE_CMAKE_PACKAGES_WITHKEYS, LineCommentTmp, gen_module_pattern, get_the_packagename,
        include_is_module, replace_placeholders,
        treehelper::{ToPoint, ToPosition, get_point_string},
    },
};
mod findpackage;
mod include;
mod subdirectory;
use tree_sitter::Node;

use crate::utils::treehelper::{PositionType, get_pos_type};

/// Storage the information when jump
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpCacheUnit {
    pub location: Location,
    pub document_info: String,
    pub is_function: bool,
}

pub type JumpKV = HashMap<String, JumpCacheUnit>;

pub static JUMP_CACHE: LazyLock<Arc<Mutex<JumpKV>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheDataUnit {
    key: String,
    location: Location,
    document_info: String,
    is_function: bool,
}

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> Option<()> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    let result_data = getsubdef(
        tree.root_node(),
        &context.lines().collect(),
        path.as_ref(),
        PositionType::VarOrFun,
        &mut Vec::new(),
        &mut Vec::new(),
        true,
        true,
    )?;
    let mut cache = JUMP_CACHE.lock().await;
    for CacheDataUnit {
        key,
        location,
        document_info,
        is_function,
    } in result_data
    {
        cache.insert(
            key,
            JumpCacheUnit {
                location,
                document_info,
                is_function,
            },
        );
    }
    None
}

#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    is_function: bool,
    loc: Location,
}

pub async fn get_cached_def<P: AsRef<Path>>(
    path: P,
    key: &str,
    documents: &DashMap<Uri, Document>,
) -> Option<ReferenceInfo> {
    let mut path = path.as_ref().to_path_buf();

    let tree_map = TREE_MAP.lock().await;

    let jump_cache = JUMP_CACHE.lock().await;
    if let Some(JumpCacheUnit {
        location,
        is_function,
        ..
    }) = jump_cache.get(key)
    {
        return Some(ReferenceInfo {
            loc: location.clone(),
            is_function: *is_function,
        });
    }
    drop(jump_cache);
    if let Ok(context) = get_or_update_buffer_contents(&path, documents).await {
        update_cache(&path, context.as_str()).await;
        let jump_cache = JUMP_CACHE.lock().await;
        if let Some(JumpCacheUnit {
            location,
            is_function,
            ..
        }) = jump_cache.get(key)
        {
            return Some(ReferenceInfo {
                loc: location.clone(),
                is_function: *is_function,
            });
        }
    }

    while let Some(parent) = tree_map.get(&path) {
        let jump_cache = JUMP_CACHE.lock().await;
        if let Some(JumpCacheUnit {
            location,
            is_function,
            ..
        }) = jump_cache.get(key)
        {
            return Some(ReferenceInfo {
                loc: location.clone(),
                is_function: *is_function,
            });
        }
        drop(jump_cache);
        if let Ok(context) = get_or_update_buffer_contents(&path, documents).await {
            update_cache(&path, context.as_str()).await;
            let jump_cache = JUMP_CACHE.lock().await;
            if let Some(JumpCacheUnit {
                location,
                is_function,
                ..
            }) = jump_cache.get(key)
            {
                return Some(ReferenceInfo {
                    loc: location.clone(),
                    is_function: *is_function,
                });
            }
        }
        path.clone_from(parent);
    }

    None
}

/// find the definition
pub async fn godef<P: AsRef<Path>>(
    location: Position,
    source: &str,
    originuri: P,
    client: &tower_lsp::Client,
    is_jump: bool,
    just_var_or_fun: bool,
    documents: &DashMap<Uri, Document>,
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
            .log_message(MessageType::INFO, "Not find any locations")
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
    documents: &DashMap<Uri, Document>,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None)?;

    let tofind = get_point_string(location, tree.root_node(), &source.lines().collect())?;

    let jumptype = get_pos_type(location, tree.root_node(), source);

    // NOTE: when just find the var or fun, then we need to skip other position type
    // Because when value in arguments, then it maybe definition, so we also need to handle this
    // part
    if !matches!(
        jumptype,
        PositionType::VarOrFun | PositionType::ArgumentOrList | PositionType::FunOrMacroIdentifier
    ) && just_var_or_fun
    {
        return None;
    }

    match jumptype {
        PositionType::VarOrFun
        | PositionType::ArgumentOrList
        | PositionType::FunOrMacroIdentifier => {
            let mut locations = vec![];
            let ReferenceInfo {
                loc: jump_cache,
                is_function,
            } = get_cached_def(&originuri, tofind, documents).await?;
            if is_jump {
                return Some(vec![jump_cache]);
            }

            let loc = jump_cache.uri.to_file_path().ok()?;
            locations.push(jump_cache.clone());
            let mut defdata = reference_all(&loc, tofind, is_function).await;
            locations.append(&mut defdata);
            // NOTE: ensure there is not same location, or it will cause problems
            locations.dedup();
            Some(locations)
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
        PositionType::Unknown | PositionType::Comment | PositionType::FunOrMacroArgs => None,
        #[cfg(unix)]
        PositionType::FindPkgConfig => None,
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

async fn reference_all<P: AsRef<Path>>(path: P, tofind: &str, is_function: bool) -> Vec<Location> {
    let mut results = vec![];
    let from = path.as_ref();
    let mut paths: Vec<PathBuf> = if from
        .extension()
        .is_some_and(|extension| extension == "cmake")
    {
        let avaiablepaths = TREE_CMAKE_MAP.lock().await;
        let mut temp: Vec<PathBuf> = avaiablepaths
            .iter()
            .filter(|(cmake, _parents)| **cmake == from)
            .flat_map(|(_, parents)| parents.clone())
            .collect();

        let mut results = vec![];
        let map = TREE_MAP.lock().await;
        for included in temp.iter() {
            let mut childrens = map
                .iter()
                .filter(|(_child, parent)| *parent == included)
                .map(|(child, _)| child.clone())
                .collect();
            results.append(&mut childrens);
        }
        results.append(&mut temp);
        results
    } else {
        let map = TREE_MAP.lock().await;
        map.iter()
            .filter(|(_child, parent)| **parent == from)
            .map(|(child, _)| child.clone())
            .collect()
    };
    paths.push(from.to_path_buf());

    for rp in paths {
        let Ok(source) = tokio::fs::read_to_string(&rp).await else {
            continue;
        };
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let Some(tree) = parse.parse(&source, None) else {
            continue;
        };
        let newsource = source.lines().collect();
        if let Some(mut locs) =
            reference_inner(tree.root_node(), &newsource, tofind, rp, is_function)
        {
            results.append(&mut locs);
        }
    }
    results
}

/// sub get the def
fn reference_inner<P: AsRef<Path>>(
    root: Node,
    newsource: &Vec<&str>,
    tofind: &str,
    originuri: P,
    is_function: bool,
) -> Option<Vec<Location>> {
    let mut definitions: Vec<Location> = vec![];
    let mut course = root.walk();
    for child in root.children(&mut course) {
        if child.child_count() != 0 {
            if let Some(mut context) =
                reference_inner(child, newsource, tofind, originuri.as_ref(), is_function)
            {
                definitions.append(&mut context);
            }
            continue;
        }
        if child.start_position().row == child.end_position().row {
            // NOTE: if different, means it is not what I want
            if (child.kind() == CMakeNodeKinds::IDENTIFIER) ^ is_function {
                continue;
            }
            if child.kind() != CMakeNodeKinds::VARIABLE && !is_function {
                continue;
            }
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;
            let message = &newsource[h][x..y];
            if message == tofind {
                definitions.push(Location {
                    uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                    range: Range {
                        start: child.start_position().to_position(),
                        end: child.end_position().to_position(),
                    },
                });
            }
        }
    }
    if definitions.is_empty() {
        None
    } else {
        Some(definitions)
    }
}

/// get the variable from the loop
/// use position to make only can complete which has show before
#[allow(clippy::too_many_arguments)]
fn getsubdef<P: AsRef<Path>>(
    input: tree_sitter::Node,
    source: &Vec<&str>,
    local_path: P,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<CacheDataUnit>> {
    let local_path = local_path.as_ref();
    let mut course = input.walk();
    let mut defs: Vec<CacheDataUnit> = vec![];
    let mut line_comment_tmp = LineCommentTmp {
        end_y: 0,
        comments: vec![],
    };
    for child in input.children(&mut course) {
        match child.kind() {
            CMakeNodeKinds::LINE_COMMENT => {
                let start_x = child.start_position().column;
                let end_x = child.end_position().column;
                let end_y = child.end_position().row;
                let comment = &source[end_y][start_x..end_x];
                if end_y - line_comment_tmp.end_y == 1 {
                    line_comment_tmp.end_y = end_y;
                    line_comment_tmp.comments.push(comment);
                } else {
                    line_comment_tmp = LineCommentTmp {
                        end_y,
                        comments: vec![comment],
                    }
                }
            }
            CMakeNodeKinds::FUNCTION_DEF => {
                let Some(function_whole) = child.child(0) else {
                    continue;
                };
                let Some(argument_list) = function_whole.child(2) else {
                    continue;
                };
                let Some(function_name) = argument_list.child(0) else {
                    continue;
                };
                let start = function_name.start_position().to_position();
                let end = function_name.end_position().to_position();
                let x = function_name.start_position().column;
                let y = function_name.end_position().column;
                let h = function_name.start_position().row;
                let Some(name) = &source[h][x..y].split(' ').next() else {
                    continue;
                };
                let mut document_info = format!("defined function\nfrom: {}", local_path.display());

                if line_comment_tmp.is_node_comment(h) {
                    document_info = format!("{}\n\n{}", document_info, line_comment_tmp.comment());
                }
                defs.push(CacheDataUnit {
                    key: name.to_string(),
                    location: Location {
                        uri: Uri::from_file_path(local_path).unwrap(),
                        range: Range { start, end },
                    },
                    document_info,
                    is_function: true,
                });
            }
            CMakeNodeKinds::MACRO_DEF => {
                let Some(macro_whole) = child.child(0) else {
                    continue;
                };
                let Some(argument_list) = macro_whole.child(2) else {
                    continue;
                };
                let Some(marco_name) = argument_list.child(0) else {
                    continue;
                };
                let start = marco_name.start_position().to_position();
                let end = marco_name.end_position().to_position();
                let x = marco_name.start_position().column;
                let y = marco_name.end_position().column;
                let h = marco_name.start_position().row;
                let Some(name) = &source[h][x..y].split(' ').next() else {
                    continue;
                };
                let mut document_info = format!("defined macro\nfrom: {}", local_path.display());

                if line_comment_tmp.is_node_comment(h) {
                    document_info = format!("{}\n\n{}", document_info, line_comment_tmp.comment());
                }
                defs.push(CacheDataUnit {
                    key: name.to_string(),
                    location: Location {
                        uri: Uri::from_file_path(local_path).unwrap(),
                        range: Range { start, end },
                    },
                    document_info,
                    is_function: true,
                });
            }
            CMakeNodeKinds::IF_CONDITION | CMakeNodeKinds::FOREACH_LOOP | CMakeNodeKinds::BODY => {
                if let Some(mut message) = getsubdef(
                    child,
                    source,
                    local_path,
                    postype,
                    include_files,
                    complete_packages,
                    true,
                    find_cmake_in_package,
                ) {
                    defs.append(&mut message);
                }
            }
            CMakeNodeKinds::NORMAL_COMMAND => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = source[h][x..y].to_lowercase();
                if name == "include" && child.child_count() >= 3 && should_in {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &source[h][x..y];
                        let Some(name) = remove_quotation_and_replace_placeholders(name) else {
                            continue;
                        };
                        let (is_builtin, subpath) = {
                            if !include_is_module(&name) {
                                (false, local_path.parent().unwrap().join(name))
                            } else {
                                // NOTE: Module file now is not works on windows
                                // Maybe also not works on android, please make pr for me
                                let Some(glob_pattern) = gen_module_pattern(&name) else {
                                    continue;
                                };
                                let Some(path) = glob::glob(&glob_pattern)
                                    .into_iter()
                                    .flatten()
                                    .flatten()
                                    .next()
                                else {
                                    continue;
                                };
                                (true, path)
                            }
                        };
                        if include_files.contains(&subpath) {
                            continue;
                        }
                        if let Ok(true) = subpath.try_exists() {
                            if let Some(mut comps) = include::scanner_include_defs(
                                &subpath,
                                postype,
                                include_files,
                                complete_packages,
                                find_cmake_in_package,
                                is_builtin,
                            ) {
                                defs.append(&mut comps);
                            }
                            include_files.push(subpath);
                        }
                    }
                } else if name == "find_package" && child.child_count() >= 3 && should_in {
                    let Some(argumentlist) = child.child(2) else {
                        continue;
                    };
                    // use tree_sitter to find all packages
                    let argument_count = argumentlist.child_count();
                    if argument_count == 0 {
                        continue;
                    }
                    let package_prefix_node = argumentlist.child(0).unwrap();
                    let h = package_prefix_node.start_position().row;
                    let x = package_prefix_node.start_position().column;
                    let y = package_prefix_node.end_position().column;
                    let package_name = &source[h][x..y];
                    let mut component_part = Vec::new();
                    let mut cmakepackages = Vec::new();
                    let components_packages = {
                        if argument_count >= 2 {
                            let mut support_component = false;
                            let mut components_packages = Vec::new();
                            for index in 1..argument_count {
                                let package_prefix_node = argumentlist.child(index).unwrap();
                                let h = package_prefix_node.start_position().row;
                                let x = package_prefix_node.start_position().column;
                                let y = package_prefix_node.end_position().column;
                                let component = &source[h][x..y];
                                if component == "COMPONENTS" {
                                    support_component = true;
                                } else if component != "REQUIRED" {
                                    component_part.push(component.to_string());
                                    components_packages
                                        .push(format!("{package_name}::{component}"));
                                }
                            }
                            if support_component {
                                Some(components_packages)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if find_cmake_in_package && components_packages.is_some() {
                        for package in component_part {
                            cmakepackages.push(format!("{package_name}{package}"));
                        }
                    } else {
                        cmakepackages.push(package_name.to_string());
                    }
                    for package in cmakepackages {
                        if complete_packages.contains(&package) {
                            continue;
                        }
                        complete_packages.push(package.clone());
                        let Some(mut completedefs) = get_cmake_package_defs(
                            package.as_str(),
                            postype,
                            include_files,
                            complete_packages,
                        ) else {
                            continue;
                        };
                        defs.append(&mut completedefs);
                    }
                } else if name == "set" || name == "option" {
                    let Some(arguments) = child.child(2) else {
                        continue;
                    };
                    let Some(ids) = arguments.child(0) else {
                        continue;
                    };
                    if ids.start_position().row != ids.end_position().row {
                        continue;
                    }
                    let h = ids.start_position().row;
                    let x = ids.start_position().column;
                    let y = ids.end_position().column;
                    let Some(name) = &source[h][x..y].split(' ').next() else {
                        continue;
                    };
                    let mut document_info =
                        format!("defined variable\nfrom: {}", local_path.display());

                    if line_comment_tmp.is_node_comment(h) {
                        document_info =
                            format!("{}\n\n{}", document_info, line_comment_tmp.comment());
                    }
                    defs.push(CacheDataUnit {
                        key: name.to_string(),
                        location: Location {
                            uri: Uri::from_file_path(local_path).unwrap(),
                            range: Range {
                                start: Position {
                                    line: h as u32,
                                    character: x as u32,
                                },
                                end: Position {
                                    line: h as u32,
                                    character: y as u32,
                                },
                            },
                        },
                        document_info,
                        is_function: false,
                    });
                }
            }
            CMakeNodeKinds::IDENTIFIER => {
                continue;
            }
            _ => {}
        }
        if let Some(mut message) = getsubdef(
            child,
            source,
            local_path,
            postype,
            include_files,
            complete_packages,
            true,
            find_cmake_in_package,
        ) {
            defs.append(&mut message);
        }
    }
    if defs.is_empty() { None } else { Some(defs) }
}

fn get_cmake_package_defs(
    package_name: &str,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CacheDataUnit>> {
    let packageinfo = CACHE_CMAKE_PACKAGES_WITHKEYS.get(package_name)?;
    let mut complete_infos = Vec::new();

    for path in packageinfo.tojump.iter() {
        let Some(mut packages) =
            include::scanner_package_defs(path, postype, include_files, complete_packages)
        else {
            continue;
        };
        complete_infos.append(&mut packages);
    }

    Some(complete_infos)
}
#[cfg(test)]
mod jump_test {
    use tower_lsp::lsp_types;
    use tree_sitter::Point;

    use super::*;

    #[tokio::test]
    async fn tst_jump_subdir() {
        use std::fs;
        use std::fs::File;
        use std::io::Write;

        use tempfile::tempdir;

        let jump_file_src = r#"add_subdirectory(abcd_test)"#;

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
    async fn tst_jump_variable() {
        use std::fs;
        use std::fs::File;
        use std::io::Write;

        use tempfile::tempdir;
        use tower_lsp::lsp_types;

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

#[test]
fn test_sub_def() {
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;
    use tower_lsp::lsp_types;
    let dir = tempdir().unwrap();
    let top_cmake_path = dir.path().join("CMakeLists.txt");

    let mut cmake_file = File::create_new(&top_cmake_path).unwrap();
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

    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(top_cmake_context, None).unwrap();

    let mut include_files = vec![];
    let data = getsubdef(
        thetree.root_node(),
        &top_cmake_context.lines().collect(),
        &top_cmake_path,
        PositionType::VarOrFun,
        &mut include_files,
        &mut vec![],
        true,
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
                },
            },
            document_info: format!("defined variable\nfrom: {}", include_cmake_path.display()),
            is_function: false
        }]
    );
    assert_eq!(
        include_files,
        vec![include_cmake_path_2, include_cmake_path]
    );
}
