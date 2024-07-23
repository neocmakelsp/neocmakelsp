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
        self,
        treehelper::{get_position_string, point_to_position},
    },
};
use lsp_types::{MessageType, Position, Range, Url};
use once_cell::sync::Lazy;
use tower_lsp::lsp_types;
use tree_sitter::Node;
mod findpackage;
mod include;
mod subdirectory;
use crate::utils::treehelper::{get_pos_type, PositionType};
use lsp_types::Location;

pub type JumpKV = HashMap<PathBuf, Vec<(String, Location)>>;

pub static JUMP_CACHE: Lazy<Arc<Mutex<JumpKV>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

const JUMP_FILITER_KIND: &[&str] = &["identifier", "unquoted_argument"];

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> Vec<(String, Location)> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree.unwrap();
    let Some(result_data) = getsubdef(
        tree.root_node(),
        &context.lines().collect(),
        path.as_ref(),
        PositionType::Variable,
        None,
        &mut Vec::new(),
        &mut Vec::new(),
        true,
        true,
    ) else {
        return Vec::new();
    };
    let mut cache = JUMP_CACHE.lock().await;
    cache.insert(path.as_ref().to_path_buf(), result_data.clone());
    result_data
}
pub async fn get_cached_defs<P: AsRef<Path>>(path: P, key: &str) -> Vec<Location> {
    let mut path = path.as_ref().to_path_buf();
    let mut completions: Vec<Location> = Vec::new();

    let tree_map = TREE_MAP.lock().await;

    while let Some(parent) = tree_map.get(&path) {
        let jump_cache = JUMP_CACHE.lock().await;
        if let Some(data) = jump_cache.get(parent) {
            let mut append_data = data
                .iter()
                .filter(|data| data.0 == key)
                .map(|d| d.1.clone())
                .collect();
            completions.append(&mut append_data);
        } else if let Ok(context) = tokio::fs::read_to_string(parent).await {
            let mut buffer_cache = BUFFERS_CACHE.lock().await;
            buffer_cache.insert(
                lsp_types::Url::from_file_path(parent).unwrap(),
                context.clone(),
            );
            drop(jump_cache);
            let data = update_cache(parent, context.as_str()).await;
            let mut append_data = data
                .iter()
                .filter(|data| data.0 == key)
                .map(|d| d.1.clone())
                .collect();
            completions.append(&mut append_data);
            path.clone_from(parent);
            continue;
        }
        path.clone_from(parent);
    }

    completions
}
/// find the definition
pub async fn godef(
    location: Position,
    source: &str,
    originuri: String,
    client: &tower_lsp::Client,
    _is_jump: bool,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let positionstring = get_position_string(location, tree.root_node(), source);
    match positionstring {
        Some(tofind) => {
            if &tofind != "(" && &tofind != ")" {
                let jumptype =
                    get_pos_type(location, tree.root_node(), source, PositionType::Variable);
                match jumptype {
                    // TODO: maybe can hadle Include?
                    PositionType::Variable => {
                        let mut defs: Vec<Location> = vec![];

                        let mut cached_defs = get_cached_defs(&originuri, tofind.as_str()).await;

                        if !cached_defs.is_empty() {
                            defs.append(&mut cached_defs);
                        }
                        let newsource: Vec<&str> = source.lines().collect();
                        if let Some(data) = getsubdef(
                            tree.root_node(),
                            &newsource,
                            &Path::new(&originuri),
                            PositionType::Variable,
                            None,
                            &mut Vec::new(),
                            &mut Vec::new(),
                            true,
                            true,
                        ) {
                            let mut scanresults = data
                                .iter()
                                .filter(|data| data.0 == tofind)
                                .map(|d| d.1.clone())
                                .collect();
                            defs.append(&mut scanresults);
                        }
                        if defs.is_empty() {
                            None
                        } else {
                            Some(defs)
                        }
                    }
                    PositionType::FindPackage
                    | PositionType::TargetLink
                    | PositionType::TargetInclude => {
                        let tofind = tofind.split('_').collect::<Vec<&str>>()[0].to_string();
                        findpackage::cmpfindpackage(tofind, client).await
                    }
                    PositionType::NotFind => None,
                    #[cfg(unix)]
                    PositionType::FindPkgConfig => None,
                    PositionType::Include => include::cmpinclude(originuri, &tofind, client).await,
                    PositionType::SubDir => {
                        subdirectory::cmpsubdirectory(originuri, &tofind, client).await
                    }
                }
            } else {
                client.log_message(MessageType::INFO, "Empty").await;
                None
            }
        }
        None => None,
    }
}

/// sub get the def
fn godefsub(
    root: Node,
    newsource: &Vec<&str>,
    tofind: &str,
    originuri: String,
    is_jump: bool,
) -> Option<Vec<Location>> {
    let mut definitions: Vec<Location> = vec![];
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        //
        if child.kind() == "identifier" {
            continue;
        }
        if child.child_count() != 0 {
            if is_jump && JUMP_FILITER_KIND.contains(&child.kind()) {
                continue;
            }
            //let range = godefsub(child, source, tofind);
            if let Some(mut context) =
                godefsub(child, newsource, tofind, originuri.clone(), is_jump)
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
                    uri: Url::from_file_path(&originuri).unwrap(),
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
    location: Option<Position>,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<(String, Location)>> {
    if let Some(location) = location {
        if input.start_position().row as u32 > location.line {
            return None;
        }
    }
    let mut course = input.walk();
    let mut defs: Vec<(String, Location)> = vec![];
    for child in input.children(&mut course) {
        if let Some(location) = location {
            if child.start_position().row as u32 > location.line {
                // if this child is below row, then break all loop
                break;
            }
        }
        let start = point_to_position(child.start_position());
        let end = point_to_position(child.end_position());
        match child.kind() {
            "function_def" => {
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
                defs.push((
                    name.to_string(),
                    Location {
                        uri: Url::from_file_path(&local_path).unwrap(),
                        range: Range { start, end },
                    },
                ));
            }
            "macro_def" => {
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
                defs.push((
                    name.to_string(),
                    Location {
                        uri: Url::from_file_path(&local_path).unwrap(),
                        range: Range { start, end },
                    },
                ));
            }
            "body" => {
                if let Some(mut message) = getsubdef(
                    child,
                    source,
                    local_path,
                    postype,
                    location,
                    include_files,
                    complete_packages,
                    true,
                    find_cmake_in_package,
                ) {
                    defs.append(&mut message);
                }
            }
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) = getsubdef(
                    child,
                    source,
                    local_path,
                    postype,
                    location,
                    include_files,
                    complete_packages,
                    true,
                    find_cmake_in_package,
                ) {
                    defs.append(&mut message);
                }
            }
            "normal_command" => {
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
                                let Some(path) = glob::glob(
                                    format!("/usr/share/cmake*/Modules/{name}.cmake").as_str(),
                                )
                                .into_iter()
                                .flatten()
                                .flatten()
                                .next() else {
                                    continue;
                                };
                                (true, path)
                            }
                        };
                        if include_files.contains(&subpath) {
                            continue;
                        }
                        if let Ok(true) = subpath.try_exists() {
                            if let Some(mut comps) = include::scanner_include_def(
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
                }
            }
            _ => {}
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
) -> Option<Vec<(String, Location)>> {
    let packageinfo = utils::CMAKE_PACKAGES_WITHKEY.get(package_name)?;
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
