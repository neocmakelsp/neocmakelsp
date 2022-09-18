use crate::treehelper::position_to_point;
use lsp_types::Position;
use tree_sitter::Node;
#[derive(Clone, Copy)]
pub enum InputType {
    Variable,
    FindPackage,
    SubDir,
    Include,
    NotFind,
}

pub fn get_input_type(
    location: Position,
    root: Node,
    source: &str,
    inputtype: InputType,
) -> InputType {
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
                            "find_package" => InputType::FindPackage,
                            "include" => InputType::Include,
                            "add_subdirectory" => InputType::SubDir,
                            _ => InputType::Variable,
                        }
                    }
                    "argument" => match inputtype {
                        InputType::FindPackage | InputType::SubDir | InputType::Include => inputtype,
                        _ => InputType::Variable,
                    },
                    _ => InputType::Variable,
                };

                match jumptype {
                    InputType::FindPackage | InputType::SubDir | InputType::Include => {
                        return get_input_type(location, child, source, jumptype);
                    }

                    InputType::Variable => {
                        //} else {
                        let currenttype =
                            get_input_type(location, child, source, InputType::Variable);
                        match currenttype {
                            InputType::NotFind => {}
                            _ => return currenttype,
                        };
                    }
                    InputType::NotFind => {}
                }
            }
            // if is the same line
            else if child.start_position().row == child.end_position().row
                && neolocation.column <= child.end_position().column
                && neolocation.column >= child.start_position().column
            {
                return inputtype;
            }
        }
    }
    InputType::NotFind
}
