/// privide go to definition
use crate::treehelper::{get_positon_string, point_to_position, position_to_point};
use lsp_types::{Position, Range, Url};
use tree_sitter::Node;
mod findpackage;
mod subdirectory;
/// find the definition
pub fn godef(
    location: Position,
    root: Node,
    source: &str,
    originuri: String,
) -> Option<Vec<JumpLocation>> {
    match get_positon_string(location, root, source) {
        Some(tofind) => {
            if &tofind != "(" && &tofind != ")" {
                match get_jump_type(location, root, source, JumpType::Variable) {
                    JumpType::Variable => godefsub(root, source, &tofind, originuri),
                    JumpType::FindPackage => findpackage::cmpfindpackage(tofind),
                    JumpType::NotFind => None,
                }
            } else {
                None
            }
        }
        None => None,
    }
}
enum JumpType {
    Variable,
    FindPackage,
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
                let isfindpackage = match child.kind() {
                    "normal_command" => {
                        let h = child.start_position().row;
                        let ids = child.child(0).unwrap();
                        //let ids = ids.child(2).unwrap();
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        //println!("name = {}", name);
                        name == "find_package"
                    }
                    "argument" => matches!(jumptype, JumpType::FindPackage),
                    _ => false,
                };

                if isfindpackage {
                    return get_jump_type(location, child, source, JumpType::FindPackage);
                } else {
                    let currenttype = get_jump_type(location, child, source, JumpType::Variable);
                    match currenttype {
                        JumpType::NotFind => {}
                        _ => return currenttype,
                    };
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
                    uri: Url::parse(&format!("file://{}",originuri)).unwrap(),
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
