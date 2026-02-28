use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

use lsp_types::{Position, Range};
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::{Node, Point, Query, QueryCursor, StreamingIterator};

use crate::{CMakeNodeKinds, consts::TREESITTER_CMAKE_LANGUAGE};

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

pub trait ToPosition {
    fn to_position(&self) -> Position;
}

pub trait ToPoint {
    fn to_point(&self) -> Point;
}

impl ToPosition for Point {
    fn to_position(&self) -> Position {
        point_to_position(*self)
    }
}

impl ToPoint for Position {
    fn to_point(&self) -> Point {
        position_to_point(*self)
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
pub fn get_point_string<'a>(location: Point, root: Node, source: &'a [u8]) -> Option<&'a str> {
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
                .is_some_and(|message| !BLACK_POS_STRING.contains(message))
            {
                return mabepos;
            }
        }
        // if is the same line
        if child.start_position().row == child.end_position().row
            && location.column <= child.end_position().column
            && location.column >= child.start_position().column
        {
            return child.utf8_text(source).ok();
        }
    }
    None
}

/// from the position to get range
pub fn get_position_range(location: Position, root: Node) -> Option<Range> {
    let neolocation = location.to_point();
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
                start: child.start_position().to_position(),
                end: child.end_position().to_position(),
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
    }

    if range_start_position.row == location.row && range_start_position.column > location.column {
        return false;
    }
    if range_end_position.row == location.row && range_end_position.column < location.column {
        return false;
    }
    true
}

pub fn contain_comment(location: Point, root: Node) -> bool {
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
        if child.child_count() != 0 && contain_comment(location, child) {
            return true;
        }
    }
    false
}

#[inline]
pub fn get_pos_type<'a>(location: Point, root: Node, source: &'a str) -> PositionType<'a> {
    get_pos_type_inner(location, root, source.as_bytes(), PositionType::Unknown)
}

fn get_pos_type_inner<'a>(
    location: Point,
    root: Node,
    source: &'a [u8],
    input_type: PositionType<'a>,
) -> PositionType<'a> {
    let mut course = root.walk();
    for child in root.children(&mut course) {
        if !location_range_contain(location, child) {
            continue;
        }

        let jumptype = match child.kind() {
            CMakeNodeKinds::NORMAL_COMMAND => {
                let identifier = child.child(0).unwrap();
                let name = identifier.utf8_text(source).unwrap().to_lowercase();
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
                    let node_source = child.utf8_text(source).unwrap();
                    let val = first_argument.utf8_text(source).unwrap();
                    if child_count >= 2
                        && !location_range_contain(location, first_argument)
                        && input_type == PositionType::FindPackage
                        && node_source.contains("COMPONENTS")
                    {
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
            CMakeNodeKinds::LINE_COMMENT
            | CMakeNodeKinds::BRACKET_COMMENT
            | CMakeNodeKinds::BRACKET_COMMENT_CONTENT
            | CMakeNodeKinds::BRACKET_COMMENT_CLOSE
            | CMakeNodeKinds::BRACKET_COMMENT_OPEN
            | CMakeNodeKinds::BRACKET_ARGUMENT_OPEN => PositionType::Comment,
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
                    if let Some(name) = name
                        && name.to_lowercase() == "pkg_check_modules"
                    {
                        return PositionType::Unknown;
                    }
                    return jumptype;
                }
                PositionType::VarOrFun => {
                    let currenttype =
                        get_pos_type_inner(location, child, source, PositionType::VarOrFun);
                    match currenttype {
                        PositionType::Unknown => {}
                        _ => return currenttype,
                    }
                }
                PositionType::Unknown
                | PositionType::Comment
                | PositionType::ArgumentOrList
                | PositionType::FunOrMacroArgs => {
                    return get_pos_type_inner(location, child, source, jumptype);
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

const LINE_COMMENT_QUERY: &str = r#"(
    (line_comment) @comment
)"#;

const BRACKET_COMMENT_QUERY: &str = r#"(
    (bracket_comment) @comment
)"#;

const MACRO_QUERY: &str = r#"(
   (macro_command
       (argument_list) @argument_list
   )
)"#;

const FUNCTION_QUERY: &str = r#"(
   (function_command
       (argument_list) @argument_list
   )
)"#;

const NORMAL_COMMAND_QUERY: &str = r#"
(
    (normal_command
        (identifier) @identifier
        (argument_list) @argument_list
    )
)
"#;

pub struct LineCommentNode<'a> {
    pub content: &'a str,
    pub node: Node<'a>,
}
pub struct BracketCommentNode<'a> {
    pub content: &'a str,
}

pub struct MacroNode<'a> {
    pub name: &'a str,
    pub arguments: Vec<Node<'a>>,
}

pub struct FuncNode<'a> {
    pub name: &'a str,
    pub arguments: Vec<Node<'a>>,
}

pub struct NormalCommandNode<'a> {
    pub identifier: &'a str,
    pub identifier_node: Option<Node<'a>>,
    pub first_arg: &'a str,
    pub args: Vec<Node<'a>>,
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use u32::MAX
pub fn get_line_comments<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: u32,
) -> Vec<LineCommentNode<'a>> {
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, LINE_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    'out: while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let content = node
                .utf8_text(source)
                .unwrap()
                .strip_prefix("#")
                .unwrap()
                .trim();
            comments.push(LineCommentNode { content, node });
        }
    }
    comments
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use u32::MAX
pub fn get_bracket_comments<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: u32,
) -> Vec<BracketCommentNode<'a>> {
    // NOTE: prepare comments
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, BRACKET_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    'out: while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            comments.push(BracketCommentNode {
                content: node.utf8_text(source).unwrap(),
            });
        }
    }
    comments
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use u32::MAX
pub fn get_macros<'a>(source: &'a [u8], node: Node<'a>, max_height: u32) -> Vec<MacroNode<'a>> {
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, MACRO_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    'out: while let Some(m) = matches_macro.next() {
        let mut macro_node = MacroNode {
            name: "",
            arguments: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let mut walk = node.walk();
            for child in node.children(&mut walk) {
                macro_node.arguments.push(child);
            }
            let Some(first_arg) = node.child(0) else {
                continue 'out;
            };
            macro_node.name = first_arg.utf8_text(source).unwrap();
        }
        macros.push(macro_node);
    }
    macros
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use u32::MAX
pub fn get_normal_commands<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: u32,
) -> Vec<NormalCommandNode<'a>> {
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, NORMAL_COMMAND_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    'out: while let Some(m) = matches_macro.next() {
        let mut normal_command = NormalCommandNode {
            identifier: "",
            identifier_node: None,
            first_arg: "",
            args: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            for command in m.captures {
                let node = command.node;
                if node.kind() == "identifier" {
                    normal_command.identifier = node.utf8_text(source).unwrap();
                    normal_command.identifier_node = Some(node);
                    continue;
                }
                if node.kind() == "argument_list" {
                    let mut walk = node.walk();
                    for child in node.children(&mut walk) {
                        normal_command.args.push(child);
                    }
                    let Some(first_arg) = node.child(0) else {
                        continue 'out;
                    };
                    normal_command.first_arg = first_arg.utf8_text(source).unwrap();
                }
            }
        }
        macros.push(normal_command);
    }
    macros
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use u32::MAX
pub fn get_functions<'a>(source: &'a [u8], node: Node<'a>, max_height: u32) -> Vec<FuncNode<'a>> {
    let mut funs = vec![];
    let query_fun = Query::new(&TREESITTER_CMAKE_LANGUAGE, FUNCTION_QUERY).unwrap();
    let mut cursor_fun = QueryCursor::new();
    let mut matches_fun = cursor_fun.matches(&query_fun, node, source);

    'out: while let Some(m) = matches_fun.next() {
        let mut fun_node = FuncNode {
            name: "",
            arguments: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let mut walk = node.walk();
            for child in node.children(&mut walk) {
                fun_node.arguments.push(child);
            }
            let Some(first_arg) = node.child(0) else {
                continue 'out;
            };
            fun_node.name = first_arg.utf8_text(source).unwrap();
        }
        funs.push(fun_node);
    }
    funs
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;

    #[test]
    fn test_change() {
        let point = Point {
            row: 10,
            column: 10,
        };
        assert_eq!(
            Position {
                line: 10,
                character: 10
            },
            point.to_position()
        );
        let position = Position {
            line: 10,
            character: 10,
        };
        assert_eq!(
            Point {
                row: 10,
                column: 10
            },
            position.to_point()
        );
    }

    #[test]
    fn test_line_comment() {
        let source = "set(A \"
A#ss\" #sss)";
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let tree = parse.parse(source, None).unwrap();
        let input = tree.root_node();
        assert!(!contain_comment(Point { row: 1, column: 1 }, input));
        assert!(contain_comment(Point { row: 1, column: 8 }, input));
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
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let tree = parse.parse(source, None).unwrap();
        let input = tree.root_node();
        let source_lines = source.as_bytes();
        let pos_str_1 =
            get_point_string(Point { row: 2, column: 4 }, input, &source_lines).unwrap();
        assert_eq!(pos_str_1, "ABC");
        let pos_str_2 =
            get_point_string(Point { row: 3, column: 12 }, input, &source_lines).unwrap();
        assert_eq!(pos_str_2, "ABC");
        let pos_str_3 =
            get_point_string(Point { row: 3, column: 16 }, input, &source_lines).unwrap();
        assert_eq!(pos_str_3, "${ABC}eft");
    }

    #[test]
    fn test_postype() {
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
find_package(Qt5Core CONFIG)
macro(macro_test)
endmacro()
    "#;
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let tree = parse.parse(source, None).unwrap();
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
            get_pos_type(
                Point {
                    row: 15,
                    column: 15
                },
                input,
                source,
            ),
            PositionType::FindPackage
        );
        assert_eq!(
            get_pos_type(
                Point {
                    row: 16,
                    column: 21
                },
                input,
                source,
            ),
            PositionType::FindPackage
        );
        assert_eq!(
            get_pos_type(Point { row: 17, column: 8 }, input, source,),
            PositionType::FunOrMacroIdentifier
        );
    }
}
