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

const BLACK_POS_STRING: [&str; 5] = ["(", ")", "{", "}", "$"];

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
pub fn get_point_string(location: Point, root: Node, source: &Vec<&str>) -> Option<String> {
    let mut course = root.walk();
    for child in root.children(&mut course) {
        if !location_range_contain(location, child) {
            continue;
        }
        if BLACK_POS_STRING.contains(&child.kind()) {
            continue;
        }
        if child.child_count() != 0 {
            let mabepos = get_point_string(location, child, source);
            if mabepos
                .as_ref()
                .is_some_and(|message| !BLACK_POS_STRING.contains(&message.as_str()))
            {
                return mabepos;
            };
        }
        // if is the same line
        if child.start_position().row == child.end_position().row
            && location.column <= child.end_position().column
            && location.column >= child.start_position().column
        {
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;

            let message = &source[h][x..y];

            return Some(message.to_string());
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
        if !location_range_contain(neolocation, child) {
            continue;
        }
        if BLACK_POS_STRING.contains(&child.kind()) {
            continue;
        }
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
            storage.insert(akey.to_string(), message.to_string());
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
            storage.insert(akey.to_string(), message.to_string());
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
            storage.insert(akey.to_string(), message.to_string());
        }
    }
    #[cfg(unix)]
    storage.insert(
        "pkg_check_modules".to_string(),
        "please FindPackage PkgConfig first".to_string(),
    );
    storage
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionType<'a> {
    VarOrFun, // the variable defined in cmake file or macro or fun
    FunOrMacroIdentifier,
    ArgumentOrList, // Not the ${abc} kind, just normake argument
    FindPackage,    // normal_command start with find_package
    FindPackageSpace(&'a str),
    #[cfg(unix)]
    FindPkgConfig, // PkgConfig file
    SubDir,
    Include,
    FunOrMacroArgs,
    Unknown, // Unknown type, use as the input
    TargetInclude,
    TargetLink,
    Comment,
}

fn location_range_contain(location: Point, range_node: Node) -> bool {
    let range_start_position = range_node.start_position();
    let range_end_position = range_node.end_position();
    if range_end_position.row < location.row || range_start_position.row > location.row {
        return false;
    };

    if range_start_position.row == location.row && range_start_position.column > location.column {
        return false;
    }
    if range_end_position.row == location.row && range_end_position.column < location.column {
        return false;
    }
    true
}

pub fn is_comment(location: Point, root: Node) -> bool {
    if !location_range_contain(location, root) {
        return false;
    }
    if root.kind() == CMakeNodeKinds::LINE_COMMENT || root.kind() == CMakeNodeKinds::BRACKET_COMMENT
    {
        return true;
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if !location_range_contain(location, child) {
            continue;
        }
        if child.kind() == CMakeNodeKinds::LINE_COMMENT
            || child.kind() == CMakeNodeKinds::BRACKET_COMMENT
        {
            return true;
        }
        if child.child_count() != 0 && is_comment(location, child) {
            return true;
        }
    }
    false
}

#[inline]
pub fn get_pos_type<'a>(location: Point, root: Node, source: &'a str) -> PositionType<'a> {
    get_pos_type_inner(
        location,
        root,
        &source.lines().collect(),
        PositionType::Unknown,
    )
}

fn get_pos_type_inner<'a>(
    location: Point,
    root: Node,
    source: &Vec<&'a str>,
    input_type: PositionType<'a>,
) -> PositionType<'a> {
    let mut course = root.walk();
    for child in root.children(&mut course) {
        if !location_range_contain(location, child) {
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
            CMakeNodeKinds::ARGUMENT_LIST => {
                let child_count = child.child_count();
                if child.child_count() >= 1 {
                    let first_argument = child.child(0).unwrap();
                    let row = first_argument.start_position().row;
                    let col_x = first_argument.start_position().column;
                    let col_y = first_argument.end_position().column;
                    let val = &source[row][col_x..col_y];
                    if child_count >= 2 && input_type == PositionType::FindPackage {
                        return PositionType::FindPackageSpace(val);
                    }
                    if input_type == PositionType::FunOrMacroArgs
                        && location_range_contain(location, first_argument)
                    {
                        return PositionType::FunOrMacroIdentifier;
                    }
                }
                PositionType::ArgumentOrList
            }
            CMakeNodeKinds::FUNCTION_COMMAND | CMakeNodeKinds::MACRO_COMMAND => {
                PositionType::FunOrMacroArgs
            }
            CMakeNodeKinds::UNQUOTED_ARGUMENT | CMakeNodeKinds::QUOTED_ELEMENT => {
                PositionType::ArgumentOrList
            }
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
                    let inner_type = get_pos_type_inner(location, child, source, jumptype);
                    if matches!(
                        inner_type,
                        PositionType::VarOrFun | PositionType::FindPackageSpace(_)
                    ) {
                        return inner_type;
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
                PositionType::Unknown
                | PositionType::Comment
                | PositionType::ArgumentOrList
                | PositionType::FunOrMacroArgs => {
                    return get_pos_type_inner(location, child, source, jumptype)
                }
                // NOTE: it should be designed to cannot be reach
                PositionType::FindPackageSpace(_) | PositionType::FunOrMacroIdentifier => {
                    unreachable!()
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
fn test_point_string() {
    let source = r#"
# it is a comment
set(ABC "abcd")
set(EFT "${ABC}eft")
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
    let source_lines = source.lines().collect();
    let pos_str_1 = get_point_string(Point { row: 2, column: 4 }, input, &source_lines).unwrap();
    assert_eq!(pos_str_1, "ABC");
    let pos_str_2 = get_point_string(Point { row: 3, column: 12 }, input, &source_lines).unwrap();
    assert_eq!(pos_str_2, "ABC");
    let pos_str_3 = get_point_string(Point { row: 3, column: 16 }, input, &source_lines).unwrap();
    assert_eq!(pos_str_3, "${ABC}eft");
}

#[test]
fn tst_postype() {
    let source = r#"
# it is a comment
set(ABC "abcd")
function(abc)
endfunction()
find_package(PkgConfig)
pkg_check_modules(zlib)
target_link_libraries(ABC PUBLIC
    ${zlib_LIBRARIES}
    ${abcd}
)
include("abcd/efg.cmake")
#[[.rst:
test, here is BRACKET_COMMENT
]]#
find_package(Qt5 COMPONENTS Core)
macro(macro_test)
endmacro()
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
        get_pos_type(Point { row: 3, column: 5 }, input, source,),
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
        get_pos_type(Point { row: 8, column: 4 }, input, source,),
        PositionType::VarOrFun
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
    );
    assert_eq!(
        get_pos_type(Point { row: 13, column: 3 }, input, source,),
        PositionType::Comment
    );
    assert_eq!(
        get_pos_type(
            Point {
                row: 15,
                column: 30
            },
            input,
            source,
        ),
        PositionType::FindPackageSpace("Qt5")
    );
    assert_eq!(
        get_pos_type(Point { row: 16, column: 8 }, input, source,),
        PositionType::FunOrMacroIdentifier
    )
}
