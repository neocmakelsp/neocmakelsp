use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

use lsp_types::{Position, Range};
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::{Node, Point};

use crate::{
    CMakeNodeKinds,
    utils::{
        NeoStrExt,
        query::{
            try_get_bracket_comment, try_get_function, try_get_line_comment, try_get_macro,
            try_get_normal_command, try_get_variable,
        },
    },
};

const BLACK_POS_STRING: [&str; 5] = ["(", ")", "{", "}", "$"];

/// treesitter to lsp_types
#[inline]
pub const fn point_to_position(input: Point) -> Position {
    Position {
        line: input.row as u32,
        character: input.column as u32,
    }
}

pub trait NodeExt {
    fn contain(self, loc: Point) -> bool;
}

impl<'a> NodeExt for Node<'a> {
    fn contain(self, loc: Point) -> bool {
        location_range_contain(loc, self)
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
pub const fn position_to_point(input: Position) -> Point {
    Point {
        row: input.line as usize,
        column: input.character as usize,
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PositionType<'a> {
    #[default]
    VarOrFun, // the variable defined in cmake file or macro or fun
    FunOrMacroIdentifier,
    FindPackage, // normal_command start with find_package
    FindPackageSpace(&'a str),
    #[cfg(unix)]
    FindPkgConfig, // PkgConfig file
    SubDir,
    Include,
    FunOrMacroArgs,
    TargetInclude,
    TargetLink,
    Comment,
}

pub fn location_range_contain(location: Point, range_node: Node) -> bool {
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

#[derive(Debug, Default)]
pub struct CurrentNodeInfo<'a> {
    #[allow(unused)]
    node: Option<Node<'a>>,
    typ: PositionType<'a>,
    content: Option<&'a str>,
    argument_index: Option<usize>,
}

impl<'a> CurrentNodeInfo<'a> {
    pub const fn pos_type(&'a self) -> PositionType<'a> {
        self.typ
    }

    pub fn content(&'a self) -> Option<&'a str> {
        self.content.map(|content| content.remove_quotation())
    }

    pub fn is_first_argument(&'a self) -> bool {
        self.argument_index.is_some_and(|index| index == 0)
    }

    pub const fn in_argument_list(&'a self) -> bool {
        self.argument_index.is_some()
    }

    pub fn get(source: &'a str, node: Node<'a>, location: Point) -> Self {
        let source = source.as_bytes();

        if let Some(variable) = Self::get_variable(source, node, location) {
            return variable;
        }

        if let Some(command) = Self::get_command(source, node, location) {
            return command;
        }

        if let Some(r#macro) = Self::get_macro(source, node, location) {
            return r#macro;
        }

        if let Some(function) = Self::get_function(source, node, location) {
            return function;
        }

        if let Some(line_comment) = Self::get_line_comment(source, node, location) {
            return line_comment;
        }

        Self::get_bracket_comment(source, node, location).unwrap_or_default()
    }

    fn get_variable(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let variable = try_get_variable(source, node, location)?;

        Some(Self {
            node: Some(variable.node),
            typ: PositionType::VarOrFun,
            content: Some(variable.content),
            argument_index: None,
        })
    }

    fn get_command(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let command = try_get_normal_command(source, node, location)?;
        let identifier = command.identifier.to_lowercase();
        let (argument_index, node) = if command.identifier_node.contain(location) {
            (None, command.identifier_node)
        } else {
            command
                .args
                .into_iter()
                .enumerate()
                .map(|(idx, node)| (Some(idx), node))
                .find(|(_, arg)| arg.contain(location))?
        };
        let content = node.utf8_text(source).unwrap();
        let typ = match identifier.as_str() {
            "find_package" => {
                let argument_list = command.argument_list?.utf8_text(source).unwrap();
                let val = command.first_arg?;

                if argument_list.contains("COMPONENTS")
                    && argument_index.is_some_and(|index| index >= 2)
                    && content != "COMPONENTS"
                {
                    PositionType::FindPackageSpace(val)
                } else {
                    PositionType::FindPackage
                }
            }
            #[cfg(unix)]
            "pkg_check_modules" => PositionType::FindPkgConfig,
            "include" => PositionType::Include,
            "add_subdirectory" => PositionType::SubDir,
            "target_include_directories" => PositionType::TargetInclude,
            "target_link_libraries" => PositionType::TargetLink,
            _ => PositionType::VarOrFun,
        };
        Some(Self {
            node: Some(node),
            typ,
            content: Some(content),
            argument_index,
        })
    }

    fn get_macro(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let command = try_get_macro(source, node, location)?;
        for (index, arg) in command.arguments.into_iter().enumerate() {
            if !arg.contain(location) {
                continue;
            }
            let typ = if index == 0 {
                PositionType::FunOrMacroIdentifier
            } else {
                PositionType::FunOrMacroArgs
            };
            return Some(Self {
                node: Some(arg),
                typ,
                content: Some(arg.utf8_text(source).unwrap()),
                argument_index: None,
            });
        }
        None
    }
    fn get_function(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let command = try_get_function(source, node, location)?;
        for (index, arg) in command.arguments.into_iter().enumerate() {
            if !arg.contain(location) {
                continue;
            }
            let typ = if index == 0 {
                PositionType::FunOrMacroIdentifier
            } else {
                PositionType::FunOrMacroArgs
            };
            return Some(Self {
                node: Some(arg),
                typ,
                content: Some(arg.utf8_text(source).unwrap()),
                argument_index: None,
            });
        }
        None
    }

    fn get_line_comment(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let comment = try_get_line_comment(source, node, location)?;
        Some(Self {
            node: Some(comment.node),
            typ: PositionType::Comment,
            content: Some(comment.content),
            argument_index: None,
        })
    }
    fn get_bracket_comment(source: &'a [u8], node: Node<'a>, location: Point) -> Option<Self> {
        let comment = try_get_bracket_comment(source, node, location)?;
        Some(Self {
            node: Some(comment.node),
            typ: PositionType::Comment,
            content: Some(comment.content),
            argument_index: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;

    use super::*;
    fn parse_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        parser.parse(source, None).unwrap()
    }
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
        let tree = parse_tree(source);
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
        let tree = parse_tree(source);
        let input = tree.root_node();
        let node_1 = CurrentNodeInfo::get(source, input, Point { row: 2, column: 4 });
        let pos_str_1 = node_1.content().unwrap();
        assert_eq!(pos_str_1, "ABC");
        let node_2 = CurrentNodeInfo::get(source, input, Point { row: 3, column: 12 });

        let pos_str_2 = node_2.content().unwrap();
        assert_eq!(pos_str_2, "ABC");
        let node_3 = CurrentNodeInfo::get(source, input, Point { row: 3, column: 16 });
        let pos_str_3 = node_3.content().unwrap();
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
    abcd
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
        let tree = parse_tree(source);
        let input = tree.root_node();

        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 1, column: 3 },).pos_type(),
            PositionType::Comment
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 2, column: 4 },).pos_type(),
            PositionType::VarOrFun
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 3, column: 5 }).pos_type(),
            PositionType::VarOrFun
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 5, column: 15 }).pos_type(),
            PositionType::FindPackage
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 5, column: 1 }).pos_type(),
            PositionType::VarOrFun
        );

        #[cfg(unix)]
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 6, column: 22 }).pos_type(),
            PositionType::FindPkgConfig
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 9, column: 4 }).pos_type(),
            PositionType::TargetLink
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 8, column: 7 }).pos_type(),
            PositionType::VarOrFun
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 9, column: 6 }).pos_type(),
            PositionType::TargetLink
        );
        assert_eq!(
            CurrentNodeInfo::get(
                source,
                input,
                Point {
                    row: 11,
                    column: 11
                },
            )
            .pos_type(),
            PositionType::Include
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 13, column: 3 },).pos_type(),
            PositionType::Comment
        );
        assert_eq!(
            CurrentNodeInfo::get(
                source,
                input,
                Point {
                    row: 15,
                    column: 30
                },
            )
            .pos_type(),
            PositionType::FindPackageSpace("Qt5")
        );
        assert_eq!(
            CurrentNodeInfo::get(
                source,
                input,
                Point {
                    row: 15,
                    column: 15
                },
            )
            .pos_type(),
            PositionType::FindPackage
        );
        assert_eq!(
            CurrentNodeInfo::get(
                source,
                input,
                Point {
                    row: 16,
                    column: 21
                },
            )
            .pos_type(),
            PositionType::FindPackage
        );
        assert_eq!(
            CurrentNodeInfo::get(source, input, Point { row: 17, column: 8 },).pos_type(),
            PositionType::FunOrMacroIdentifier
        );
    }
}
