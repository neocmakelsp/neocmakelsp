use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;

/// provide go to definition
use crate::{
    consts::TREESITTER_CMAKE_LANGUAGE,
    languageserver::BUFFERS_CACHE,
    scansubs::TREE_MAP,
    utils::{
        gen_module_pattern, get_the_packagename, replace_placeholders,
        treehelper::{get_point_string, point_to_position, position_to_point},
        LineCommentTmp, CACHE_CMAKE_PACKAGES_WITHKEYS,
    },
    CMakeNodeKinds,
};
use std::sync::LazyLock;
use tower_lsp::lsp_types::{self, Location, MessageType, Position, Range, Url};
mod findpackage;
mod include;
mod subdirectory;
use crate::utils::treehelper::{get_pos_type, PositionType};

use tree_sitter::Node;

/// Storage the information when jump
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpCacheUnit {
    pub location: Location,
    pub document_info: String,
}

pub type JumpKV = HashMap<String, JumpCacheUnit>;

pub static JUMP_CACHE: LazyLock<Arc<Mutex<JumpKV>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

const JUMP_FILITER_KIND: &[&str] = &["identifier", "unquoted_argument"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheDataUnit {
    key: String,
    location: Location,
    document_info: String,
}

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> Option<()> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree.unwrap();
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
    } in result_data
    {
        cache.insert(
            key,
            JumpCacheUnit {
                location,
                document_info,
            },
        );
    }
    None
}

pub async fn get_cached_defs<P: AsRef<Path>>(path: P, key: &str) -> Option<Location> {
    let mut path = path.as_ref().to_path_buf();

    let tree_map = TREE_MAP.lock().await;

    let jump_cache = JUMP_CACHE.lock().await;
    if let Some(JumpCacheUnit { location, .. }) = jump_cache.get(key) {
        return Some(location.clone());
    }
    drop(jump_cache);
    if let Ok(context) = tokio::fs::read_to_string(&path).await {
        let mut buffer_cache = BUFFERS_CACHE.lock().await;
        buffer_cache.insert(
            lsp_types::Url::from_file_path(&path).unwrap(),
            context.clone(),
        );
        drop(buffer_cache);
        update_cache(&path, context.as_str()).await;
        let jump_cache = JUMP_CACHE.lock().await;
        if let Some(JumpCacheUnit { location, .. }) = jump_cache.get(key) {
            return Some(location.clone());
        }
    }

    while let Some(parent) = tree_map.get(&path) {
        let jump_cache = JUMP_CACHE.lock().await;
        if let Some(JumpCacheUnit { location, .. }) = jump_cache.get(key) {
            return Some(location.clone());
        }
        drop(jump_cache);
        if let Ok(context) = tokio::fs::read_to_string(&parent).await {
            let mut buffer_cache = BUFFERS_CACHE.lock().await;
            buffer_cache.insert(
                lsp_types::Url::from_file_path(&path).unwrap(),
                context.clone(),
            );
            drop(buffer_cache);
            update_cache(&path, context.as_str()).await;
            let jump_cache = JUMP_CACHE.lock().await;
            if let Some(JumpCacheUnit { location, .. }) = jump_cache.get(key) {
                return Some(location.clone());
            }
        }
        path.clone_from(parent);
    }

    None
}

/// find the definition
pub async fn godef(
    location: Position,
    source: &str,
    originuri: &PathBuf,
    client: &tower_lsp::Client,
    is_jump: bool,
) -> Option<Vec<Location>> {
    let current_point = position_to_point(location);
    let locations = godef_inner(current_point, source, originuri, is_jump).await;
    if locations.is_none() {
        client
            .log_message(MessageType::INFO, "Not find any locations")
            .await;
    }
    locations
}

async fn godef_inner(
    location: tree_sitter::Point,
    source: &str,
    originuri: &PathBuf,
    is_jump: bool,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None)?;

    let tofind = get_point_string(location, tree.root_node(), &source.lines().collect())?;

    let jumptype = get_pos_type(location, tree.root_node(), source);

    println!("{jumptype:?}");
    match jumptype {
        PositionType::VarOrFun => {
            let mut locations = vec![];
            if let Some(jump_cache) = get_cached_defs(&originuri, tofind.as_str()).await {
                if is_jump {
                    return Some(vec![jump_cache]);
                }
                locations.push(jump_cache);
            }

            let newsource: Vec<&str> = source.lines().collect();
            if let Some(mut defdata) =
                simplegodefsub(tree.root_node(), &newsource, &tofind, originuri, is_jump)
            {
                locations.append(&mut defdata);
            }
            if locations.is_empty() {
                None
            } else {
                Some(locations)
            }
        }
        PositionType::FindPackageSpace(space) => {
            let tofind = format!("{space}{}", get_the_packagename(&tofind));
            findpackage::cmpfindpackage(&tofind)
        }
        PositionType::FindPackage | PositionType::TargetLink | PositionType::TargetInclude => {
            let tofind = get_the_packagename(&tofind);
            findpackage::cmpfindpackage(tofind)
        }
        PositionType::Unknown
        | PositionType::Comment
        | PositionType::ArgumentOrList
        | PositionType::FunOrMacroArgs => None,
        #[cfg(unix)]
        PositionType::FindPkgConfig => None,
        PositionType::Include => {
            let fixed_url = replace_placeholders(&tofind)?;
            include::cmpinclude(originuri, &fixed_url)
        }
        PositionType::SubDir => {
            let fixed_url = replace_placeholders(&tofind)?;
            subdirectory::cmpsubdirectory(originuri, &fixed_url)
        }
    }
}

/// sub get the def
fn simplegodefsub(
    root: Node,
    newsource: &Vec<&str>,
    tofind: &str,
    originuri: &PathBuf,
    is_jump: bool,
) -> Option<Vec<Location>> {
    let mut definitions: Vec<Location> = vec![];
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        //
        if child.kind() == CMakeNodeKinds::IDENTIFIER {
            continue;
        }
        if child.child_count() != 0 {
            if is_jump && JUMP_FILITER_KIND.contains(&child.kind()) {
                continue;
            }
            if let Some(mut context) = simplegodefsub(child, newsource, tofind, originuri, is_jump)
            {
                definitions.append(&mut context);
            }
        } else if child.start_position().row == child.end_position().row {
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;
            let message = &newsource[h][x..y];
            if message == tofind {
                definitions.push(Location {
                    uri: Url::from_file_path(originuri).unwrap(),
                    range: Range {
                        start: point_to_position(child.start_position()),
                        end: point_to_position(child.end_position()),
                    },
                })
            };
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
fn getsubdef(
    input: tree_sitter::Node,
    source: &Vec<&str>,
    local_path: &Path,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<CacheDataUnit>> {
    let mut course = input.walk();
    let mut defs: Vec<CacheDataUnit> = vec![];
    let mut line_comment_tmp = LineCommentTmp {
        start_y: 0,
        comment: "",
    };
    for child in input.children(&mut course) {
        let start = point_to_position(child.start_position());
        let end = point_to_position(child.end_position());
        match child.kind() {
            CMakeNodeKinds::LINE_COMMENT => {
                let start_y = child.start_position().row;
                let start_x = child.start_position().column;
                let end_x = child.end_position().column;
                line_comment_tmp = LineCommentTmp {
                    start_y,
                    comment: &source[start_y][start_x..end_x],
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
                        uri: Url::from_file_path(local_path).unwrap(),
                        range: Range { start, end },
                    },
                    document_info,
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
                        uri: Url::from_file_path(local_path).unwrap(),
                        range: Range { start, end },
                    },
                    document_info,
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
                        let (is_buildin, subpath) = {
                            if name.split('.').count() != 1 {
                                (false, local_path.parent().unwrap().join(name))
                            } else {
                                // NOTE: Module file now is not works on windows
                                // Maybe also not works on android, please make pr for me
                                let Some(glob_pattern) = gen_module_pattern(name) else {
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
                                is_buildin,
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
                            uri: Url::from_file_path(local_path).unwrap(),
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
    if defs.is_empty() {
        None
    } else {
        Some(defs)
    }
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
        top_file.write(jump_file_src.as_bytes()).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();

        let locations = godef_inner(
            Point { row: 0, column: 20 },
            &jump_file_src,
            &top_cmake,
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            locations,
            vec![Location {
                uri: Url::from_file_path(subdir_file).unwrap(),
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
        )
    }

    #[tokio::test]
    async fn tst_jump_variable() {
        use std::fs;

        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

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
        top_file.write(jump_file_src.as_bytes()).unwrap();
        let subdir = dir.path().join("abcd_test");
        fs::create_dir_all(&subdir).unwrap();
        let subdir_file = subdir.join("CMakeLists.txt");
        File::create_new(&subdir_file).unwrap();

        let locations = godef_inner(
            Point { row: 2, column: 18 },
            &jump_file_src,
            &top_cmake,
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            locations,
            vec![Location {
                uri: Url::from_file_path(&top_cmake).unwrap(),
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
            &jump_file_src,
            &top_cmake,
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            locations_2,
            vec![Location {
                uri: Url::from_file_path(top_cmake).unwrap(),
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
        )
    }
}

#[test]
fn test_sub_def() {
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
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
                uri: Url::from_file_path(&include_cmake_path).unwrap(),
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
            document_info: format!("defined variable\nfrom: {}", include_cmake_path.display())
        }]
    );
    assert_eq!(
        include_files,
        vec![include_cmake_path_2, include_cmake_path]
    );
}
