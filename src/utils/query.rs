use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::{CMakeNodeKinds, consts::TREESITTER_CMAKE_LANGUAGE};

const ARGUMENT_LIST_QUERY: &str = r"(
    (argument_list) @argument_list
)";

const LINE_COMMENT_QUERY: &str = r"(
    (line_comment) @comment
)";

const BRACKET_COMMENT_QUERY: &str = r"(
    (bracket_comment_content) @comment
)";

const MACRO_QUERY: &str = r"(
   (macro_command
       (argument_list ((argument)*) @args))
)";

const FUNCTION_QUERY: &str = r"(
   (function_command
       (argument_list ((argument)*) @args))
)";

const NORMAL_COMMAND_QUERY: &str = r"
(
    (normal_command) @normal_command
)
";

const VARIABLE_QUERY: &str = r"
(
    (variable) @variable
)
";

pub struct VariableNode<'a> {
    pub content: &'a str,
    pub node: Node<'a>,
}

pub struct ArgumentListNode<'a> {
    pub main_node: Option<Node<'a>>,
    pub arguments: Vec<Node<'a>>,
}

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
    pub first_arg: Option<&'a str>,
    pub args: Vec<Node<'a>>,
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_variables<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<VariableNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
    let mut variables = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, VARIABLE_QUERY).unwrap();
    let mut cursor_vars = QueryCursor::new();
    let mut matches_comments = cursor_vars.matches(&query_comment, node, source);
    'out: while let Some(m) = matches_comments.next() {
        for c in m.captures {
            let node = c.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let content = node.utf8_text(source).unwrap();
            variables.push(VariableNode { content, node });
        }
    }
    variables
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_argument_lists<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<ArgumentListNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
    let mut arguments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, ARGUMENT_LIST_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    while let Some(m) = matches_comments.next() {
        let mut ag_node = ArgumentListNode {
            main_node: None,
            arguments: vec![],
        };
        let node = m.nodes_for_capture_index(0).next().unwrap();
        if node.start_position().row as u32 > max_height {
            continue;
        }
        ag_node.main_node = Some(node);

        let mut walk = node.walk();
        for child in node.children(&mut walk) {
            ag_node.arguments.push(child);
        }
        arguments.push(ag_node);
    }
    arguments
}
/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_line_comments<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<LineCommentNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
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
/// if you want to ignore it, use None
pub fn get_bracket_comments<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<BracketCommentNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
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
/// if you want to ignore it, use None
pub fn get_macros<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<MacroNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, MACRO_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    while let Some(m) = matches_macro.next() {
        let mut macro_node = MacroNode {
            name: "",
            arguments: vec![],
        };
        let first_arg = m.nodes_for_capture_index(0).next().unwrap();
        if first_arg.start_position().row as u32 > max_height {
            continue;
        }
        macro_node.name = first_arg.utf8_text(source).unwrap();
        macro_node.arguments = m.captures.iter().map(|q| q.node).collect();
        macros.push(macro_node);
    }
    macros
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_normal_commands<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<NormalCommandNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
    let mut commands = vec![];
    let query_cmd = Query::new(&TREESITTER_CMAKE_LANGUAGE, NORMAL_COMMAND_QUERY).unwrap();
    let mut cursor_cmd = QueryCursor::new();
    let mut matches_cmd = cursor_cmd.matches(&query_cmd, node, source);

    while let Some(m) = matches_cmd.next() {
        let mut normal_command = NormalCommandNode {
            identifier: "",
            identifier_node: None,
            first_arg: None,
            args: vec![],
        };
        let node = m.nodes_for_capture_index(0).next().unwrap();
        if node.start_position().row as u32 > max_height {
            continue;
        }
        let Some(identifier) = node.child(0) else {
            continue;
        };
        if identifier.kind() != CMakeNodeKinds::IDENTIFIER {
            continue;
        }
        normal_command.identifier = identifier.utf8_text(source).unwrap();
        normal_command.identifier_node = Some(identifier);
        // NOTE: child 1 is "(", it is child 2 that argument_list
        if let Some(argument_list) = node.child(2)
            && argument_list.kind() == CMakeNodeKinds::ARGUMENT_LIST
        {
            let mut walk = argument_list.walk();
            for child in argument_list.children(&mut walk) {
                normal_command.args.push(child);
            }
            if let Some(first_arg) = argument_list.child(0) {
                normal_command.first_arg = first_arg.utf8_text(source).ok();
            }
        }
        commands.push(normal_command);
    }

    commands
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_functions<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: impl Into<Option<u32>>,
) -> Vec<FuncNode<'a>> {
    let max_height = max_height.into().unwrap_or(u32::MAX);
    let mut funs = vec![];
    let query_fun = Query::new(&TREESITTER_CMAKE_LANGUAGE, FUNCTION_QUERY).unwrap();
    let mut cursor_fun = QueryCursor::new();
    let mut matches_fun = cursor_fun.matches(&query_fun, node, source);

    while let Some(m) = matches_fun.next() {
        let mut fun_node = FuncNode {
            name: "",
            arguments: vec![],
        };
        let first_arg = m.nodes_for_capture_index(0).next().unwrap();
        if first_arg.start_position().row as u32 > max_height {
            continue;
        }
        fun_node.name = first_arg.utf8_text(source).unwrap();
        fun_node.arguments = m.captures.iter().map(|q| q.node).collect();
        funs.push(fun_node);
    }
    funs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;
    fn parse_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        parser.parse(source, None).unwrap()
    }
    #[test]
    fn test_get_normal_commands() {
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
        let normal_commands = get_normal_commands(source.as_bytes(), input, None);
        assert_eq!(normal_commands.len(), 6);
        let args = vec!["ABC", "EFT", "PkgConfig", "zlib", "\"abcd/efg.cmake\""];
        for test_arg in args {
            assert!(
                normal_commands
                    .iter()
                    .any(|command| command.first_arg.is_some_and(|arg| arg == test_arg))
            );
        }
    }

    #[test]
    fn test_get_functions() {
        let source = r"
function(abc)
endfunction()

function(efg d, e, f)
endfunction()
    ";
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_functions(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 2);
        assert!(
            funs.iter()
                .any(|fun| fun.name == "abc" && fun.arguments.len() == 1)
        );
        assert!(
            funs.iter()
                .any(|fun| fun.name == "efg" && fun.arguments.len() == 4)
        );
    }

    #[test]
    fn test_get_macros() {
        let source = r"
macro(abc)
endmacro()

macro(efg d, e, f)
endmacro()
    ";
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_macros(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 2);
        assert!(
            funs.iter()
                .any(|fun| fun.name == "abc" && fun.arguments.len() == 1)
        );
        assert!(
            funs.iter()
                .any(|fun| fun.name == "efg" && fun.arguments.len() == 4)
        );
    }

    #[test]
    fn test_get_arguments() {
        let source = r"
macro(abc)
endmacro()

macro(efg d)
endmacro()

set(g a b c)
    ";
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_argument_lists(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 3);
        assert!(funs.iter().any(|fun| fun.arguments.len() == 1));
        assert!(funs.iter().any(|fun| fun.arguments.len() == 2));
        assert!(funs.iter().any(|fun| fun.arguments.len() == 4));
    }

    #[test]
    fn test_get_variables() {
        let source = r#"
set(a "${abcd}")
set(a "${efg}/${hijk}")
    "#;
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_variables(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 3);
        assert_eq!(funs[0].node.utf8_text(source.as_bytes()).unwrap(), "abcd");
        assert_eq!(funs[1].node.utf8_text(source.as_bytes()).unwrap(), "efg");
        assert_eq!(funs[2].node.utf8_text(source.as_bytes()).unwrap(), "hijk");
    }

    #[test]
    fn test_get_line_comments() {
        let source = r"
# Hello
# World
    ";
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_line_comments(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 2);
        assert_eq!(
            funs[0].node.utf8_text(source.as_bytes()).unwrap(),
            "# Hello"
        );
        assert_eq!(
            funs[1].node.utf8_text(source.as_bytes()).unwrap(),
            "# World"
        );
    }

    #[test]
    fn test_get_bracket_comments() {
        let source = r"
#[=============[
Hello world
#]=============]
    ";
        let tree = parse_tree(source);
        let input = tree.root_node();
        let funs = get_bracket_comments(source.as_bytes(), input, None);
        assert_eq!(funs.len(), 1);
        assert_eq!(funs[0].content, "Hello world\n#");
    }
}
