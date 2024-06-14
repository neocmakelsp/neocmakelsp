mod buildin;
mod findpackage;
mod includescanner;
use crate::languageserver::BUFFERS_CACHE;
use crate::scansubs::TREE_MAP;
use crate::utils::treehelper::{get_pos_type, PositionType};
use crate::{utils, CompletionResponse};
use buildin::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use tower_lsp::lsp_types;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position};

use once_cell::sync::Lazy;

pub type CompleteKV = HashMap<PathBuf, Vec<CompletionItem>>;

/// NOTE: collect the all completeitems in this PathBuf
/// Include the top CMakeList.txt
pub static COMPLETE_CACHE: Lazy<Arc<Mutex<CompleteKV>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

#[cfg(unix)]
const PKG_IMPORT_TARGET: &str = "IMPORTED_TARGET";

pub fn rst_doc_read(doc: String, filename: &str) -> Vec<CompletionItem> {
    doc.lines()
        .filter(|line| line.starts_with(".. command:: "))
        .map(|line| &line[13..])
        .map(|line| CompletionItem {
            label: line.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("defined command from {filename}\n{doc}")),
            ..Default::default()
        })
        .collect()
}

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> Vec<CompletionItem> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree.unwrap();
    let Some(result_data) = getsubcomplete(
        tree.root_node(),
        context,
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
    let mut cache = COMPLETE_CACHE.lock().await;
    cache.insert(path.as_ref().to_path_buf(), result_data.clone());
    result_data
}

pub async fn get_cached_completion<P: AsRef<Path>>(path: P) -> Vec<CompletionItem> {
    let mut path = path.as_ref().to_path_buf();
    let mut completions = Vec::new();

    let tree_map = TREE_MAP.lock().await;

    while let Some(parent) = tree_map.get(&path) {
        let complet_cache = COMPLETE_CACHE.lock().await;
        if let Some(datas) = complet_cache.get(parent) {
            completions.append(&mut datas.clone());
        } else if let Ok(context) = fs::read_to_string(parent).await {
            let mut buffer_cache = BUFFERS_CACHE.lock().await;
            buffer_cache.insert(
                lsp_types::Url::from_file_path(parent).unwrap(),
                context.clone(),
            );
            drop(complet_cache);
            completions.append(&mut update_cache(parent, context.as_str()).await);
            path.clone_from(parent);
            continue;
        }
        path.clone_from(parent);
    }

    completions
}

/// get the complet messages
pub async fn getcomplete(
    source: &str,
    location: Position,
    client: &tower_lsp::Client,
    local_path: &str,
    find_cmake_in_package: bool,
) -> Option<CompletionResponse> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let mut complete: Vec<CompletionItem> = vec![];
    let mut cached_compeletion = get_cached_completion(local_path).await;
    if !cached_compeletion.is_empty() {
        complete.append(&mut cached_compeletion);
    }
    let postype = get_pos_type(location, tree.root_node(), source, PositionType::NotFind);
    match postype {
        PositionType::Variable | PositionType::TargetLink | PositionType::TargetInclude => {
            if let Some(mut message) = getsubcomplete(
                tree.root_node(),
                source,
                Path::new(local_path),
                postype,
                Some(location),
                &mut Vec::new(),
                &mut Vec::new(),
                true,
                find_cmake_in_package,
            ) {
                complete.append(&mut message);
            }

            if let Ok(messages) = &*BUILDIN_COMMAND {
                complete.append(&mut messages.clone());
            }
            if let Ok(messages) = &*BUILDIN_VARIABLE {
                complete.append(&mut messages.clone());
            }
        }
        PositionType::FindPackage => {
            complete.append(&mut findpackage::CMAKE_SOURCE.clone());
        }
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            complete.append(&mut findpackage::PKGCONFIG_SOURCE.clone());
        }
        PositionType::Include => {
            if let Ok(messages) = &*BUILDIN_MODULE {
                complete.append(&mut messages.clone());
            }
        }
        _ => {}
    }

    if complete.is_empty() {
        client.log_message(MessageType::INFO, "Empty").await;
        None
    } else {
        Some(CompletionResponse::Array(complete))
    }
}
/// get the variable from the loop
/// use position to make only can complete which has show before
#[allow(clippy::too_many_arguments)]
fn getsubcomplete(
    input: tree_sitter::Node,
    source: &str,
    local_path: &Path,
    postype: PositionType,
    location: Option<Position>,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<CompletionItem>> {
    if let Some(location) = location {
        if input.start_position().row as u32 > location.line {
            return None;
        }
    }
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    let mut complete: Vec<CompletionItem> = vec![];
    for child in input.children(&mut course) {
        if let Some(location) = location {
            if child.start_position().row as u32 > location.line {
                // if this child is below row, then break all loop
                break;
            }
        }
        match child.kind() {
            "bracket_comment" => {
                let start_y = child.start_position().row;
                let end_y = child.end_position().row;
                let mut output = String::new();
                for item in newsource.iter().take(end_y).skip(start_y + 1) {
                    output.push_str(&format!("{item}\n"));
                }
                complete.append(&mut rst_doc_read(
                    output,
                    local_path.file_name().unwrap().to_str().unwrap(),
                ));
            }
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
                let Some(name) = &newsource[h][x..y].split(' ').next() else {
                    continue;
                };
                complete.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!("defined function\nfrom: {}", local_path.display())),
                    ..Default::default()
                });
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
                let Some(name) = &newsource[h][x..y].split(' ').next() else {
                    continue;
                };

                complete.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!("defined function\nfrom: {}", local_path.display())),
                    ..Default::default()
                });
            }
            "body" => {
                if let Some(mut message) = getsubcomplete(
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
                    complete.append(&mut message);
                }
            }
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) = getsubcomplete(
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
                    complete.append(&mut message);
                }
            }
            "normal_command" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = newsource[h][x..y].to_lowercase();
                if name == "include" && child.child_count() >= 3 && should_in {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
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
                            if let Some(mut comps) = includescanner::scanner_include_complete(
                                &subpath,
                                postype,
                                include_files,
                                complete_packages,
                                find_cmake_in_package,
                                is_buildin,
                            ) {
                                complete.append(&mut comps);
                            }
                            include_files.push(subpath);
                        }
                    }
                } else if name == "mark_as_advanced" {
                    if child.child_count() < 3 {
                        continue;
                    }
                    let child = child.child(2).unwrap();
                    let mut advancedwalk = child.walk();
                    for identifier in child.children(&mut advancedwalk) {
                        if identifier.kind() == "argument"
                            && identifier.start_position().row == identifier.end_position().row
                        {
                            let startx = identifier.start_position().column;
                            let endx = identifier.end_position().column;
                            let row = identifier.start_position().row;
                            let variable = &newsource[row][startx..endx];
                            complete.push(CompletionItem {
                                label: variable.to_string(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                detail: Some(format!(
                                    "defined var\nfrom: {}",
                                    local_path.display()
                                )),
                                ..Default::default()
                            });
                        }
                    }
                } else {
                    match postype {
                        PositionType::TargetLink
                        | PositionType::TargetInclude
                        | PositionType::Variable => {
                            if name == "set" || name == "option" {
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
                                let name = &newsource[h][x..y].split(' ').next();

                                let Some(name) = name.map(|name| name.to_string()) else {
                                    continue;
                                };
                                complete.push(CompletionItem {
                                    label: name.to_string(),
                                    kind: Some(CompletionItemKind::VALUE),
                                    detail: Some(format!(
                                        "defined variable\nfrom: {}",
                                        local_path.display()
                                    )),
                                    ..Default::default()
                                });
                            }
                            if name == "find_package" && child.child_count() >= 3 && should_in {
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
                                let package_name = &newsource[h][x..y];
                                let mut component_part = Vec::new();
                                let mut cmakepackages = Vec::new();
                                let components_packages = {
                                    if argument_count >= 2 {
                                        let mut support_commponent = false;
                                        let mut components_packages = Vec::new();
                                        for index in 1..argument_count {
                                            let package_prefix_node =
                                                argumentlist.child(index).unwrap();
                                            let h = package_prefix_node.start_position().row;
                                            let x = package_prefix_node.start_position().column;
                                            let y = package_prefix_node.end_position().column;
                                            let component = &newsource[h][x..y];
                                            if component == "COMPONENTS" {
                                                support_commponent = true;
                                            } else if component != "REQUIRED" {
                                                component_part.push(component.to_string());
                                                components_packages
                                                    .push(format!("{package_name}::{component}"));
                                            }
                                        }
                                        if support_commponent {
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
                                // mordern cmake like Qt5::Core
                                if let Some(components) = components_packages {
                                    for component in components {
                                        complete.push(CompletionItem {
                                            label: component,
                                            kind: Some(CompletionItemKind::VARIABLE),
                                            detail: Some(format!("package from: {package_name}",)),
                                            ..Default::default()
                                        });
                                    }
                                }

                                if matches!(
                                    postype,
                                    PositionType::TargetLink | PositionType::Variable
                                ) {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }

                                if matches!(
                                    postype,
                                    PositionType::TargetInclude | PositionType::Variable
                                ) {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_INCLUDE_DIRS"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                                for package in cmakepackages {
                                    if complete_packages.contains(&package) {
                                        continue;
                                    }
                                    complete_packages.push(package.clone());
                                    let Some(mut completeitem) = get_cmake_package_complete(
                                        package.as_str(),
                                        postype,
                                        include_files,
                                        complete_packages,
                                    ) else {
                                        continue;
                                    };
                                    complete.append(&mut completeitem);
                                }
                            }
                            #[cfg(unix)]
                            if name == "pkg_check_modules" && child.child_count() >= 3 {
                                let ids = child.child(2).unwrap();
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                let package_names: Vec<&str> =
                                    newsource[h][x..y].split(' ').collect();
                                let package_name = package_names[0];

                                let modernpkgconfig = package_names.contains(&PKG_IMPORT_TARGET);
                                if modernpkgconfig
                                    && matches!(
                                        postype,
                                        PositionType::Variable | PositionType::TargetLink
                                    )
                                {
                                    complete.push(CompletionItem {
                                        label: format!("PkgConfig::{package_name}"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }

                                if matches!(
                                    postype,
                                    PositionType::TargetLink | PositionType::Variable
                                ) {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                                if matches!(
                                    postype,
                                    PositionType::TargetInclude | PositionType::Variable
                                ) {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_INCLUDE_DIRS"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}

fn get_cmake_package_complete(
    package_name: &str,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CompletionItem>> {
    let packageinfo = utils::CMAKE_PACKAGES_WITHKEY.get(package_name)?;
    let mut complete_infos = Vec::new();

    for path in packageinfo.tojump.iter() {
        let Some(mut packages) = includescanner::scanner_package_complete(
            path,
            postype,
            include_files,
            complete_packages,
        ) else {
            continue;
        };
        complete_infos.append(&mut packages);
    }

    Some(complete_infos)
}
