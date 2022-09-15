/// privide go to definition
use crate::treehelper::{get_positon_string, point_to_position, position_to_point};
use lsp_types::{MessageType, Position, Range, Url};
use tree_sitter::Node;
mod findpackage;
mod include;
mod subdirectory;
/// find the definition
pub async fn godef(
    location: Position,
    source: &str,
    originuri: String,
    client: &tower_lsp::Client,
) -> Option<Vec<JumpLocation>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let positionstring = get_positon_string(location, tree.root_node(), source);
    match positionstring {
        Some(tofind) => {
            if &tofind != "(" && &tofind != ")" {
                let jumptype = get_jump_type(location, tree.root_node(), source, JumpType::Variable);
                match jumptype {
                    JumpType::Variable => godefsub(tree.root_node(), source, &tofind, originuri),
                    JumpType::FindPackage => findpackage::cmpfindpackage(tofind,client).await,
                    JumpType::NotFind => None,
                    JumpType::Include => include::cmpinclude(originuri, &tofind),
                    JumpType::SubDir => subdirectory::cmpsubdirectory(originuri, &tofind),
                }
            } else {
                client.log_message(MessageType::INFO, "Empty").await;
                None
            }
        }
        None => None,
    }
}
#[derive(Clone, Copy)]
enum JumpType {
    Variable,
    FindPackage,
    SubDir,
    Include,
    NotFind,
}

fn get_jump_type(location: Position, root: Node, source: &str, jumptype: JumpType) -> JumpType {
    let neolocation = position_to_point(location);
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if neolocation.row <= child.end_position().row
            && neolocation.row >= child.start_position().row
        {
            if child.child_count() != 0 {
                let jumptype = match child.kind() {
                    "normal_command" => {
                        let h = child.start_position().row;
                        let ids = child.child(0).unwrap();
                        //let ids = ids.child(2).unwrap();
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        //println!("name = {}", name);
                        //name == "find_package"
                        match name {
                            "find_package" => JumpType::FindPackage,
                            "include" => JumpType::Include,
                            "add_subdirectory" => JumpType::SubDir,
                            _ => JumpType::Variable,
                        }
                    }
                    "argument" => match jumptype {
                        JumpType::FindPackage | JumpType::SubDir | JumpType::Include => jumptype,
                        _ => JumpType::Variable,
                    },
                    _ => JumpType::Variable,
                };

                match jumptype {
                    JumpType::FindPackage | JumpType::SubDir | JumpType::Include => {
                        return get_jump_type(location, child, source, jumptype);
                    }

                    JumpType::Variable => {
                        //} else {
                        let currenttype =
                            get_jump_type(location, child, source, JumpType::Variable);
                        match currenttype {
                            JumpType::NotFind => {}
                            _ => return currenttype,
                        };
                    }
                    JumpType::NotFind => {}
                }
            }
            // if is the same line
            else if child.start_position().row == child.end_position().row
                && neolocation.column <= child.end_position().column
                && neolocation.column >= child.start_position().column
            {
                return jumptype;
            }
        }
    }
    JumpType::NotFind
}
/// sub get the def
fn godefsub(
    root: Node,
    source: &str,
    tofind: &str,
    originuri: String,
) -> Option<Vec<JumpLocation>> {
    let mut definitions: Vec<JumpLocation> = vec![];
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        //
        if child.child_count() != 0 {
            //let range = godefsub(child, source, tofind);
            if let Some(mut context) = godefsub(child, source, tofind, originuri.clone()) {
                definitions.append(&mut context);
            }
        } else if child.start_position().row == child.end_position().row {
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;
            let message = &newsource[h][x..y];
            if message == tofind {
                definitions.push(JumpLocation {
                    uri: Url::parse(&format!("file://{}", originuri)).unwrap(),
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

// TODO jump to file
pub struct JumpLocation {
    pub range: Range,
    pub uri: Url,
}
