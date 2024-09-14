use lsp_types::Position;
use lsp_types::Range;
use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::{Node, Point};

use crate::CMakeNodeKinds;

const BLACK_POS_STRING: [&str; 2] = ["(", ")"];

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
pub fn get_point_string(neolocation: Point, root: Node, source: &Vec<&str>) -> Option<String> {
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if neolocation.row <= child.end_position().row
            && neolocation.row >= child.start_position().row
        {
            if child.child_count() != 0 {
                let mabepos = get_point_string(neolocation, child, source);
                if mabepos
                    .as_ref()
                    .is_some_and(|message| !BLACK_POS_STRING.contains(&message.as_str()))
                {
                    return mabepos;
                };
            }
            // if is the same line
            if child.start_position().row == child.end_position().row
                && neolocation.column <= child.end_position().column
                && neolocation.column >= child.start_position().column
            {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;

                let message = &source[h][x..y];
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

pub static MESSAGE_STORAGE: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionType {
    VarOrFun,       // the variable defined in cmake file or macro or fun
    ArgumentOrList, // Not the ${abc} kind, just normake argument
    FindPackage,    // normal_command start with find_package
    #[cfg(unix)]
    FindPkgConfig, // PkgConfig file
    SubDir,
    Include,
    Unknown, // Unknown type, use as the input
    TargetInclude,
    TargetLink,
    Comment,
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
    if root.kind() == CMakeNodeKinds::LINE_COMMENT {
        return true;
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if !location_range_contain(child.start_position(), child.end_position(), location) {
            continue;
        }
        if child.kind() == CMakeNodeKinds::LINE_COMMENT {
            return true;
        }
        if child.child_count() != 0 && is_comment(location, child) {
            return true;
        }
    }
    false
}

#[inline]
pub fn get_pos_type(location: Point, root: Node, source: &str) -> PositionType {
    get_pos_type_inner(
        location,
        root,
        &source.lines().collect(),
        PositionType::Unknown,
    )
}

fn node_in_range(node: Point, range_node: Node) -> bool {
    let range_start_position = range_node.start_position();
    let range_end_position = range_node.end_position();
    if range_end_position.row < node.row || range_start_position.row > node.row {
        return false;
    };

    if range_start_position.row == node.row && range_start_position.column > node.column {
        return false;
    }
    if range_end_position.row == node.row && range_end_position.column < node.column {
        return false;
    }
    true
}

fn get_pos_type_inner(
    location: Point,
    root: Node,
    source: &Vec<&str>,
    input_type: PositionType,
) -> PositionType {
    let mut course = root.walk();
    for child in root.children(&mut course) {
        if !node_in_range(location, child) {
            continue;
        }

        let jumptype = match child.kind() {
            CMakeNodeKinds::NORMAL_COMMAND => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = source[h][x..y].to_lowercase();
                match name.as_str() {
                    "find_package" => PositionType::FindPackage,
                    #[cfg(unix)]
                    "pkg_check_modules" => PositionType::FindPkgConfig,
                    "include" => PositionType::Include,
                    "add_subdirectory" => PositionType::SubDir,
                    "target_include_directories" => PositionType::TargetInclude,
                    "target_link_libraries" => PositionType::TargetLink,
                    _ => PositionType::VarOrFun,
                }
            }
            CMakeNodeKinds::UNQUOTED_ARGUMENT
            | CMakeNodeKinds::ARGUMENT_LIST
            | CMakeNodeKinds::QUOTED_ELEMENT => PositionType::ArgumentOrList,
            CMakeNodeKinds::NORMAL_VAR
            | CMakeNodeKinds::VARIABLE_REF
            | CMakeNodeKinds::VARIABLE => PositionType::VarOrFun,
            CMakeNodeKinds::ARGUMENT => match input_type {
                PositionType::FindPackage | PositionType::SubDir | PositionType::Include => {
                    input_type
                }
                #[cfg(unix)]
                PositionType::FindPkgConfig => input_type,
                _ => PositionType::VarOrFun,
            },
            CMakeNodeKinds::LINE_COMMENT | CMakeNodeKinds::BRACKET_COMMENT => PositionType::Comment,
            _ => PositionType::VarOrFun,
        };

        if child.child_count() != 0 {
            match jumptype {
                PositionType::FindPackage
                | PositionType::SubDir
                | PositionType::Include
                | PositionType::TargetInclude
                | PositionType::TargetLink => {
                    if let PositionType::VarOrFun =
                        get_pos_type_inner(location, child, source, input_type)
                    {
                        return PositionType::VarOrFun;
                    }
                    let name = get_point_string(location, root, source);
                    if let Some(name) = name {
                        let name = name.to_lowercase();
                        if SPECIALCOMMANDS.contains(&name.as_str()) {
                            return PositionType::Unknown;
                        }
                    }
                    return jumptype;
                }
                #[cfg(unix)]
                PositionType::FindPkgConfig => {
                    let name = get_point_string(location, root, source);
                    if let Some(name) = name {
                        if name.to_lowercase() == "pkg_check_modules" {
                            return PositionType::Unknown;
                        }
                    }
                    return jumptype;
                }
                PositionType::VarOrFun => {
                    let currenttype =
                        get_pos_type_inner(location, child, source, PositionType::VarOrFun);
                    match currenttype {
                        PositionType::Unknown => {}
                        _ => return currenttype,
                    };
                }
                PositionType::Unknown | PositionType::Comment | PositionType::ArgumentOrList => {
                    return get_pos_type_inner(location, child, source, input_type)
                }
            }
        } else {
            // if is the same line
            return jumptype;
        }
    }
    PositionType::Unknown
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

#[test]
fn tst_postype() {
    let source = r#"
# it is a comment
set(ABC, "abcd")
function(abc)
endfunction()
find_package(PkgConfig)
pkg_check_modules(zlib)
target_link_libraries(ABC PUBLIC
    ${zlib_LIBRARIES}
    ${abcd}
)
include("abcd/efg.cmake")
    "#;
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&source, None).unwrap();
    let input = tree.root_node();

    assert_eq!(
        get_pos_type(Point { row: 1, column: 3 }, input, source,),
        PositionType::Comment
    );
    assert_eq!(
        get_pos_type(Point { row: 2, column: 4 }, input, source,),
        PositionType::VarOrFun
    );
    assert_eq!(
        get_pos_type(Point { row: 5, column: 15 }, input, source,),
        PositionType::FindPackage
    );
    assert_eq!(
        get_pos_type(Point { row: 5, column: 1 }, input, source,),
        PositionType::VarOrFun
    );
    #[cfg(unix)]
    assert_eq!(
        get_pos_type(Point { row: 6, column: 22 }, input, source,),
        PositionType::FindPkgConfig
    );
    assert_eq!(
        get_pos_type(Point { row: 8, column: 2 }, input, source,),
        PositionType::TargetLink
    );
    assert_eq!(
        get_pos_type(Point { row: 9, column: 6 }, input, source,),
        PositionType::VarOrFun
    );
    assert_eq!(
        get_pos_type(
            Point {
                row: 11,
                column: 11
            },
            input,
            source,
        ),
        PositionType::Include
    )
}
