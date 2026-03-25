use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{Location, MessageType, Position, Range, Uri};

use crate::languageserver::get_or_update_buffer_contents;
use crate::scansubs::TREE_CMAKE_MAP;
use crate::utils::remove_quotation_and_replace_placeholders;
/// provide go to definition
use crate::{
    consts::TREESITTER_CMAKE_LANGUAGE,
    scansubs::TREE_MAP,
    utils::{
        CACHE_CMAKE_PACKAGES_WITHKEYS, gen_module_pattern, get_the_packagename, include_is_module,
        replace_placeholders,
        treehelper::{ToPoint, ToPosition, get_point_string},
    },
};
mod findpackage;
mod include;
mod subdirectory;
use tree_sitter::Node;

use crate::utils::treehelper::{PositionType, get_pos_type, location_range_contain};

use crate::utils::query::{
    get_functions, get_line_comments, get_macros, get_normal_commands, get_variables,
};

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
        context,
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
    documents: &DashMap<Uri, String>,
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
    documents: &DashMap<Uri, String>,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None)?;

    let tofind = get_point_string(location, tree.root_node(), source.as_bytes())?;

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
            let Some(ReferenceInfo {
                loc: jump_cache,
                is_function,
            }) = get_cached_def(&originuri, tofind, documents).await
            else {
                if !is_jump {
                    return None;
                }
                let jumps = query_reference(
                    tree.root_node(),
                    source.as_bytes(),
                    location,
                    tofind,
                    originuri,
                    false,
                    true,
                )?;
                return Some(jumps);
            };
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
        let newsource = source.as_bytes();
        if let Some(mut locs) = query_reference(
            tree.root_node(),
            newsource,
            None,
            tofind,
            rp,
            is_function,
            false,
        ) {
            results.append(&mut locs);
        }
    }
    results
}

/// sub get the def
fn query_reference<P: AsRef<Path>, L: Into<Option<tree_sitter::Point>>>(
    root: Node,
    source: &[u8],
    location: L,
    tofind: &str,
    originuri: P,
    is_function: bool,
    jump: bool,
) -> Option<Vec<Location>> {
    let location = location.into();
    let mut definitions: Vec<Location> = vec![];
    let funcs = get_functions(source, root, None);
    let macros = get_macros(source, root, None);
    if is_function && !jump {
        let commands = get_normal_commands(source, root, None);
        for fun in funcs {
            let fun_name = fun.name;
            if fun_name != tofind {
                continue;
            }
            let fun_node = fun.arguments[0];
            definitions.push(Location {
                uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                range: Range {
                    start: fun_node.start_position().to_position(),
                    end: fun_node.end_position().to_position(),
                },
            });
        }
        for macro_node in macros {
            let macro_name = macro_node.name;
            if macro_name != tofind {
                continue;
            }
            let macro_node = macro_node.arguments[0];
            definitions.push(Location {
                uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                range: Range {
                    start: macro_node.start_position().to_position(),
                    end: macro_node.end_position().to_position(),
                },
            });
        }
        for cmd in commands {
            let cmd_name = cmd.identifier;
            if cmd_name != tofind {
                continue;
            }
            let cmd_node = cmd.identifier_node.unwrap();
            definitions.push(Location {
                uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                range: Range {
                    start: cmd_node.start_position().to_position(),
                    end: cmd_node.end_position().to_position(),
                },
            });
        }
    } else {
        if let Some(location) = location
            && let Some(f_v) = funcs
                .iter()
                .find(|n| location_range_contain(location, n.node))
            && let Some(arg) = f_v.args(source).iter().find(|arg| arg.content == tofind)
        {
            let var_node = arg.node;
            definitions.push(Location {
                uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                range: Range {
                    start: var_node.start_position().to_position(),
                    end: var_node.end_position().to_position(),
                },
            });
        }
        if let Some(location) = location
            && let Some(m_v) = macros
                .iter()
                .find(|n| location_range_contain(location, n.node))
            && let Some(arg) = m_v.args(source).iter().find(|arg| arg.content == tofind)
        {
            let var_node = arg.node;
            definitions.push(Location {
                uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                range: Range {
                    start: var_node.start_position().to_position(),
                    end: var_node.end_position().to_position(),
                },
            });
        }
        if !jump {
            let vars = get_variables(source, root, None);
            for var in vars {
                if var.content != tofind {
                    continue;
                }
                let var_node = var.node;
                definitions.push(Location {
                    uri: Uri::from_file_path(originuri.as_ref()).unwrap(),
                    range: Range {
                        start: var_node.start_position().to_position(),
                        end: var_node.end_position().to_position(),
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
    source: &str,
    local_path: P,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<CacheDataUnit>> {
    let local_path = local_path.as_ref();
    let mut defs: Vec<CacheDataUnit> = vec![];

    let source_bytes = source.as_bytes();

    // NOTE: prepare
    let comments = get_line_comments(source_bytes, input, None);

    let macros = get_macros(source_bytes, input, None);
    let functions = get_functions(source_bytes, input, None);
    let normal_commands = get_normal_commands(source_bytes, input, None);

    // NOTE: check functions
    for fun in functions {
        let name = fun.name;
        let row = fun.arguments[0].start_position().row;

        let fun_node = fun.arguments[0];
        let start = fun_node.start_position().to_position();
        let end = fun_node.end_position().to_position();

        let mut document_info = format!("defined function\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| c.node.start_position().row + 1 == row)
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
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

    // NOTE: check macros
    for macro_node in macros {
        let name = macro_node.name;
        let row = macro_node.arguments[0].start_position().row;

        let fun_node = macro_node.arguments[0];
        let start = fun_node.start_position().to_position();
        let end = fun_node.end_position().to_position();

        let mut document_info = format!("defined macro\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| c.node.start_position().row + 1 == row)
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
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
    // NOTE: check normal_commands
    for command in normal_commands {
        let name = command.identifier.to_lowercase();
        if name == "include" && should_in {
            let Some(first_arg) = command.first_arg else {
                continue;
            };
            let Some(file_name) = remove_quotation_and_replace_placeholders(first_arg) else {
                continue;
            };
            let (is_builtin, subpath) = {
                if !include_is_module(&file_name) {
                    (false, local_path.parent().unwrap().join(file_name))
                } else {
                    // NOTE: Module file now is not works on windows
                    // Maybe also not works on android, please make pr for me
                    let Some(glob_pattern) = gen_module_pattern(&file_name) else {
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
        if name == "find_package" && should_in {
            let Some(package_name) = command.first_arg else {
                continue;
            };
            let argument_count = command.args.len();
            let mut component_part = Vec::new();
            let mut cmakepackages = Vec::new();
            let components_packages = {
                if argument_count >= 2 {
                    let mut support_component = false;
                    let mut components_packages = Vec::new();
                    for index in 1..argument_count {
                        let package_prefix_node = command.args[index];
                        let component = package_prefix_node.utf8_text(source_bytes).unwrap();
                        if component == "COMPONENTS" {
                            support_component = true;
                        } else if component != "REQUIRED" {
                            component_part.push(component.to_string());
                            components_packages.push(format!("{package_name}::{component}"));
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
            let Some(name) = command.first_arg else {
                continue;
            };
            let row = command.identifier_node.unwrap().start_position().row;
            let mut document_info = format!("defined variable\nfrom: {}", local_path.display());

            let val_name = command.args[0];
            let h = val_name.start_position().row;
            let x = val_name.start_position().column;
            let y = val_name.end_position().column;
            if let Some(line_comment) = comments
                .iter()
                .find(|c| c.node.start_position().row + 1 == row)
                .map(|c| c.content)
            {
                document_info = format!("{}\n\n{}", document_info, line_comment);
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

    #[test]
    fn test_sub_def() {
        let dir = tempdir().unwrap();
        let top_cmake_path = dir.path().join("CMakeLists.txt");

        let mut cmake_file = File::create_new(&top_cmake_path).unwrap();
        let top_cmake_context = r"
include(abcd_test.cmake)
";
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
            top_cmake_context,
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
}
