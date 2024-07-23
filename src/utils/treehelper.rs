use lsp_types::Position;
use lsp_types::Range;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::{Node, Point};

const SPECIALCOMMANDS: [&str; 3] = [
    "find_package",
    "target_link_libraries",
    "target_include_directories",
];

/// treesitter to lsp_types
#[inline]
pub fn point_to_position(input: Point) -> Position {
    Position {
        line: input.row as u32,
        character: input.column as u32,
    }
}

/// lsp_types to treesitter
#[inline]
pub fn position_to_point(input: Position) -> Point {
    Point {
        row: input.line as usize,
        column: input.character as usize,
    }
}

/// get the position of the string
pub fn get_position_string(location: Position, root: Node, source: &str) -> Option<String> {
    let neolocation = position_to_point(location);
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if neolocation.row <= child.end_position().row
            && neolocation.row >= child.start_position().row
        {
            if child.child_count() != 0 {
                let mabepos = get_position_string(location, child, source);
                if mabepos.is_some() {
                    return mabepos;
                };
            }
            // if is the same line
            else if child.start_position().row == child.end_position().row
                && neolocation.column <= child.end_position().column
                && neolocation.column >= child.start_position().column
            {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;

                let message = &newsource[h][x..y];
                //crate::notify_send(message, crate::Type::Info);
                return Some(message.to_string());
            }
        }
    }
    None
}

/// from the position to get range
pub fn get_position_range(location: Position, root: Node) -> Option<Range> {
    let neolocation = position_to_point(location);
    //let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if neolocation.row <= child.end_position().row
            && neolocation.row >= child.start_position().row
        {
            if child.child_count() != 0 {
                let mabepos = get_position_range(location, child);
                if mabepos.is_some() {
                    return mabepos;
                }
            }
            // if is the same line
            else if child.start_position().row == child.end_position().row
                && neolocation.column <= child.end_position().column
                && neolocation.column >= child.start_position().column
            {
                return Some(Range {
                    start: point_to_position(child.start_position()),
                    end: point_to_position(child.end_position()),
                });
            }
        }
    }
    None
}

//#[allow(unused)]
pub static MESSAGE_STORAGE: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut storage: HashMap<String, String> = HashMap::new();
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    if let Ok(output) = Command::new("cmake").arg("--help-commands").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage
                .entry(akey.to_string())
                .or_insert_with(|| message.to_string());
        }
    }
    if let Ok(output) = Command::new("cmake").arg("--help-variables").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage
                .entry(akey.to_string())
                .or_insert_with(|| message.to_string());
        }
    }
    if let Ok(output) = Command::new("cmake").arg("--help-modules").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage
                .entry(akey.to_string())
                .or_insert_with(|| message.to_string());
        }
    }
    #[cfg(unix)]
    storage
        .entry("pkg_check_modules".to_string())
        .or_insert_with(|| "please FindPackage PkgConfig first".to_string());
    storage
});

#[derive(Clone, Copy, Debug)]
pub enum PositionType {
    Variable,
    FindPackage,
    #[cfg(unix)]
    FindPkgConfig,
    SubDir,
    Include,
    NotFind,
    TargetInclude,
    TargetLink,
}

fn location_range_contain(start_point: Point, end_point: Point, location: Point) -> bool {
    if start_point.row > location.row || end_point.row < location.row {
        return false;
    }
    if start_point.row == end_point.row {
        return start_point.column <= location.column && end_point.column >= location.column;
    }
    if start_point.row == location.row {
        return start_point.column <= location.column;
    }
    if end_point.row == location.row {
        return end_point.column >= location.column;
    }
    true
}

pub fn is_comment(location: Point, root: Node) -> bool {
    if !location_range_contain(root.start_position(), root.end_position(), location) {
        return false;
    }
    if root.kind() == "line_comment" {
        return true;
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if !location_range_contain(child.start_position(), child.end_position(), location) {
            continue;
        }
        if child.kind() == "line_comment" {
            return true;
        }
        if child.child_count() != 0 && is_comment(location, child) {
            return true;
        }
    }
    false
}

// FIXME: there is bug
// find_package(SS)
// cannot get the type of find_package
pub fn get_pos_type(
    location: Position,
    root: Node,
    source: &str,
    inputtype: PositionType,
) -> PositionType {
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
                        let name = newsource[h][x..y].to_lowercase();
                        match name.as_str() {
                            "find_package" => PositionType::FindPackage,
                            #[cfg(unix)]
                            "pkg_check_modules" => PositionType::FindPkgConfig,
                            "include" => PositionType::Include,
                            "add_subdirectory" => PositionType::SubDir,
                            "target_include_directories" => PositionType::TargetInclude,
                            "target_link_libraries" => PositionType::TargetLink,
                            _ => PositionType::Variable,
                        }
                    }
                    "normal_var" | "unquoted_argument" | "variable_def" | "variable" => {
                        PositionType::Variable
                    }
                    "argument" => match inputtype {
                        PositionType::FindPackage
                        | PositionType::SubDir
                        | PositionType::Include => inputtype,
                        #[cfg(unix)]
                        PositionType::FindPkgConfig => inputtype,
                        _ => PositionType::Variable,
                    },
                    "line_comment" => PositionType::NotFind,
                    _ => PositionType::Variable,
                };

                match jumptype {
                    PositionType::FindPackage
                    | PositionType::SubDir
                    | PositionType::Include
                    | PositionType::TargetInclude
                    | PositionType::TargetLink => {
                        let name = get_position_string(location, root, source);
                        if let Some(name) = name {
                            let name = name.to_lowercase();
                            if SPECIALCOMMANDS.contains(&name.as_str()) {
                                return PositionType::NotFind;
                            }
                        }
                        return jumptype;
                    }
                    #[cfg(unix)]
                    PositionType::FindPkgConfig => {
                        let name = get_position_string(location, root, source);
                        if let Some(name) = name {
                            if name.to_lowercase() == "pkg_check_modules" {
                                return PositionType::NotFind;
                            }
                        }
                        return jumptype;
                    }
                    PositionType::Variable => {
                        //} else {
                        let currenttype =
                            get_pos_type(location, child, source, PositionType::Variable);
                        match currenttype {
                            PositionType::NotFind => {}
                            _ => return currenttype,
                        };
                    }
                    PositionType::NotFind => {}
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
    PositionType::NotFind
}

#[test]
fn tst_line_comment() {
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;
    let source = "set(A \"
A#ss\" #sss)";
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&source, None).unwrap();
    let input = tree.root_node();
    assert!(!is_comment(Point { row: 1, column: 1 }, input));
    assert!(is_comment(Point { row: 1, column: 8 }, input));
}
