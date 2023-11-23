// todo compelete type
mod buildin;
mod findpackage;
mod includescanner;
use crate::utils::treehelper::{get_pos_type, PositionType};
use crate::{utils, CompletionResponse};
use buildin::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position};
use std::path::{Path, PathBuf};

#[cfg(unix)]
const PKG_IMPORT_TARGET: &str = "IMPORTED_TARGET";

pub fn rst_doc_read(doc: String, filename: &str) -> Vec<CompletionItem> {
    doc.lines()
        .filter(|line| line.starts_with(".. command:: "))
        .map(|line| &line[13..])
        .map(|line| format!("{line}()"))
        .map(|line| CompletionItem {
            label: line,
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("defined command from {filename}\n{doc}")),
            ..Default::default()
        })
        .collect()
}

/// get the complet messages
pub async fn getcomplete(
    source: &str,
    location: Position,
    client: &tower_lsp::Client,
    local_path: &str,
) -> Option<CompletionResponse> {
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let mut complete: Vec<CompletionItem> = vec![];
    let postype = get_pos_type(location, tree.root_node(), source, PositionType::NotFind);
    match postype {
        PositionType::Variable | PositionType::TargetLink | PositionType::TargetInclude => {
            if let Some(mut message) = getsubcomplete(
                tree.root_node(),
                source,
                Path::new(local_path),
                postype,
                Some(location),
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
fn getsubcomplete(
    input: tree_sitter::Node,
    source: &str,
    local_path: &Path,
    postype: PositionType,
    location: Option<Position>,
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
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let Some(name) = &newsource[h][x..y].split(' ').next() else {
                    continue;
                };
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            "macro_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let Some(name) = &newsource[h][x..y].split(' ').next() else {
                    continue;
                };
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            "body" => {
                if let Some(mut message) =
                    getsubcomplete(child, source, local_path, postype, location)
                {
                    complete.append(&mut message);
                }
            }
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) =
                    getsubcomplete(child, source, local_path, postype, location)
                {
                    complete.append(&mut message);
                }
            }
            "normal_command" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = newsource[h][x..y].to_lowercase();
                if name == "include" && child.child_count() >= 3 {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        let subpath = {
                            if name.split('.').count() != 1 {
                                local_path.parent().unwrap().join(name)
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
                                path
                            }
                        };
                        if let Ok(true) = cmake_try_exists(&subpath) {
                            if let Some(mut comps) =
                                includescanner::scanner_include_complete(&subpath, postype)
                            {
                                complete.append(&mut comps);
                            }
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
                                    local_path.file_name().unwrap().to_str().unwrap()
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
                                        local_path.file_name().unwrap().to_str().unwrap()
                                    )),
                                    ..Default::default()
                                });
                            }
                            if name == "find_package" && child.child_count() >= 3 {
                                let Some(ids) = child.child(2) else {
                                    continue;
                                };
                                let h = ids.start_position().row;
                                //let ids = ids.child(2).unwrap();
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                if y < x {
                                    continue;
                                }
                                let package_names: Vec<&str> =
                                    newsource[h][x..y].split(' ').collect();
                                let mut cmakepackages = Vec::new();
                                let package_name = package_names[0];
                                let components_packages = {
                                    if package_names.len() >= 2 {
                                        let mut support_commponent = false;
                                        let mut components_packages = Vec::new();
                                        for component in package_names.iter().skip(1) {
                                            if *component == "COMPONENTS" {
                                                support_commponent = true;
                                            } else if *component != "REQUIRED" {
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
                                match components_packages {
                                    Some(ref packages) => {
                                        for package in packages {
                                            cmakepackages.push(format!("{package_name}{package}"));
                                        }
                                    }
                                    None => {
                                        cmakepackages.push(package_name.to_string());
                                    }
                                }
                                // mordern cmake like Qt5::Core
                                if let Some(components) = components_packages {
                                    for component in components {
                                        if let PositionType::TargetLink = postype {
                                            complete.push(CompletionItem {
                                                label: component,
                                                kind: Some(CompletionItemKind::VARIABLE),
                                                detail: Some(format!(
                                                    "package from: {package_name}",
                                                )),
                                                ..Default::default()
                                            });
                                        } else {
                                            complete.push(CompletionItem {
                                                label: component,
                                                kind: Some(CompletionItemKind::VARIABLE),
                                                detail: Some(format!(
                                                    "package from: {package_name}",
                                                )),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                } else if let PositionType::TargetLink = postype {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                } else {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_INCLUDE_DIRS"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                }
                                for package in cmakepackages {
                                    let Some(mut completeitem) =
                                        get_cmake_package_complete(package.as_str(), postype)
                                    else {
                                        continue;
                                    };
                                    complete.append(&mut completeitem);
                                }
                            }
                            #[cfg(unix)]
                            if name == "pkg_check_modules" && child.child_count() >= 3 {
                                let ids = child.child(2).unwrap();
                                //let ids = ids.child(2).unwrap();
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                let package_names: Vec<&str> =
                                    newsource[h][x..y].split(' ').collect();
                                let package_name = package_names[0];

                                let modernpkgconfig = package_names.contains(&PKG_IMPORT_TARGET);
                                if modernpkgconfig {
                                    if matches!(
                                        postype,
                                        PositionType::Variable | PositionType::TargetLink
                                    ) {
                                        complete.push(CompletionItem {
                                            label: format!("PkgConfig::{package_name}"),
                                            kind: Some(CompletionItemKind::VARIABLE),
                                            detail: Some(format!("package: {package_name}",)),
                                            ..Default::default()
                                        });
                                    }
                                } else if let PositionType::TargetLink = postype {
                                    complete.push(CompletionItem {
                                        label: format!("{package_name}_LIBRARIES"),
                                        kind: Some(CompletionItemKind::VARIABLE),
                                        detail: Some(format!("package: {package_name}",)),
                                        ..Default::default()
                                    });
                                } else {
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

fn cmake_try_exists(input: &PathBuf) -> std::io::Result<bool> {
    match std::fs::metadata(input) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn get_cmake_package_complete(
    package_name: &str,
    postype: PositionType,
) -> Option<Vec<CompletionItem>> {
    let packageinfo = utils::CMAKE_PACKAGES_WITHKEY.get(package_name)?;
    let mut complete_infos = Vec::new();

    for path in packageinfo.tojump.iter() {
        let Some(mut packages) = includescanner::scanner_include_complete(path, postype) else {
            continue;
        };
        complete_infos.append(&mut packages);
    }

    Some(complete_infos)
}
