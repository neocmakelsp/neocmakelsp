use tree_sitter::{Node, Point, Query, QueryCapture, QueryCursor, Range, StreamingIterator};

use crate::{
    CMakeNodeKinds,
    consts::TREESITTER_CMAKE_LANGUAGE,
    utils::treehelper::{ToPoint, ToPosition},
};
use tower_lsp::lsp_types::Range as LspRange;
#[derive(Debug)]
pub struct AstNode<'a, Data = ()> {
    pub node: Node<'a>,
    /// names of captured nodes
    /// it can be the highlight name
    pub names: Vec<&'a str>,
    pub children: Vec<Self>,

    /// This part allow you to storage extra data
    pub data: Data,
}

impl<'a, Data> PartialEq for AstNode<'a, Data> {
    fn eq(&self, other: &Self) -> bool {
        self.node.eq(&other.node)
    }
}

pub trait ToLspRange {
    fn lsp_range(&self) -> LspRange;
}

impl ToLspRange for Range {
    fn lsp_range(&self) -> LspRange {
        LspRange {
            start: self.start_point.to_position(),
            end: self.end_point.to_position(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QueryRange {
    start: Point,
    end: Point,
}
pub trait ToQueryRange {
    fn to_query_range(self) -> QueryRange;
}
impl From<Point> for QueryRange {
    fn from(value: Point) -> Self {
        Self {
            start: value,
            end: value,
        }
    }
}

impl ToQueryRange for QueryRange {
    fn to_query_range(self) -> QueryRange {
        self
    }
}

impl ToQueryRange for Point {
    fn to_query_range(self) -> QueryRange {
        QueryRange::from(self)
    }
}

impl From<Range> for QueryRange {
    fn from(value: Range) -> Self {
        Self {
            start: value.start_point,
            end: value.end_point,
        }
    }
}

impl ToQueryRange for Range {
    fn to_query_range(self) -> QueryRange {
        QueryRange::from(self)
    }
}

impl From<LspRange> for QueryRange {
    fn from(value: LspRange) -> Self {
        Self {
            start: value.start.to_point(),
            end: value.end.to_point(),
        }
    }
}
impl ToQueryRange for LspRange {
    fn to_query_range(self) -> QueryRange {
        QueryRange::from(self)
    }
}
impl<'a, Data> Eq for AstNode<'a, Data> {}

pub trait RangeContain {
    fn contain(&self, other: &Self) -> bool;
}

impl RangeContain for Range {
    fn contain(&self, other: &Self) -> bool {
        self.start_byte <= other.start_byte && self.end_byte >= other.end_byte
    }
}

impl<'a> RangeContain for AstNode<'a> {
    fn contain(&self, other: &Self) -> bool {
        self.node.range().contain(&other.node.range())
    }
}

impl<'a, Data> Ord for AstNode<'a, Data> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.range().start_byte.cmp(&other.range().start_byte)
    }
}

impl<'a, Data> PartialOrd for AstNode<'a, Data> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, Data> AstNode<'a, Data>
where
    Data: Default,
{
    pub fn new(node: Node<'a>, highlight: &'a str) -> Self {
        Self {
            node,
            names: vec![highlight],
            children: vec![],
            data: Data::default(),
        }
    }

    pub fn with_data(self, data: Data) -> Self {
        Self { data, ..self }
    }
}
impl<'a, Data> AstNode<'a, Data> {
    pub fn range(&self) -> Range {
        self.node.range()
    }
    pub fn insert_node(&mut self, ast_node: Self) {
        if let Some(hnode) = self
            .children
            .iter_mut()
            .find(|hnode| hnode.range() == ast_node.range())
        {
            hnode.names.extend(ast_node.names);
            return;
        }
        if let Some(hnode) = self
            .children
            .iter_mut()
            .find(|hnode| hnode.range().contain(&ast_node.range()))
        {
            return hnode.insert_node(ast_node);
        }
        self.children.push(ast_node);
    }

    pub fn sort_node(&mut self) {
        self.children.sort();
        for child in self.children.iter_mut() {
            child.sort_node();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AstNodeContainer<'a, Data = ()> {
    pub nodes: Vec<AstNode<'a, Data>>,
}

impl<'a, Data> AstNodeContainer<'a, Data> {
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }
    // FIXME: we should never sort it so many times
    pub fn insert_node(&mut self, ast_node: AstNode<'a, Data>) {
        if let Some(hnode) = self
            .nodes
            .iter_mut()
            .find(|hnode| hnode.range() == ast_node.range())
        {
            hnode.names.extend(ast_node.names);
            return;
        }
        if let Some(hnode) = self
            .nodes
            .iter_mut()
            .find(|hnode| hnode.range().contain(&ast_node.range()))
        {
            hnode.insert_node(ast_node);
            return;
        }
        self.nodes.push(ast_node);
    }

    /// You need to call it to finish the job
    pub fn sort_to_finish(&mut self) {
        self.nodes.sort();
        for node in self.nodes.iter_mut() {
            node.sort_node();
        }
    }
}

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
   (macro_def
        (macro_command
            (argument_list ((argument)*) @args))) @macro_def
)";

const FUNCTION_QUERY: &str = r"(
    (function_def
        (function_command
            (argument_list ((argument)*) @args))) @function_def
)";

pub const NORMAL_COMMAND_QUERY: &str = r"
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
    pub main_node: Node<'a>,
    pub arguments: Vec<Node<'a>>,
}

pub struct LineCommentNode<'a> {
    pub content: &'a str,
    pub node: Node<'a>,
}

pub struct BracketCommentNode<'a> {
    pub content: &'a str,
    pub node: Node<'a>,
}

pub struct MacroNode<'a> {
    pub name: &'a str,
    pub node: Node<'a>,
    pub arguments: Vec<Node<'a>>,
}

impl<'a> MacroNode<'a> {
    pub fn args(&'a self, source: &'a [u8]) -> Vec<FunMarcoArg<'a>> {
        let mut arg_strs = vec![];
        for arg in self.arguments[1..].iter() {
            arg_strs.push(FunMarcoArg {
                node: *arg,
                content: arg.utf8_text(source).unwrap(),
            });
        }
        arg_strs
    }
}
pub struct FuncNode<'a> {
    pub name: &'a str,
    pub node: Node<'a>,
    pub arguments: Vec<Node<'a>>,
}

pub struct FunMarcoArg<'a> {
    #[allow(unused)]
    // I want to use it in jump
    pub node: Node<'a>,
    pub content: &'a str,
}

impl<'a> FuncNode<'a> {
    pub fn args(&'a self, source: &'a [u8]) -> Vec<FunMarcoArg<'a>> {
        let mut arg_strs = vec![];
        for arg in self.arguments[1..].iter() {
            arg_strs.push(FunMarcoArg {
                node: *arg,
                content: arg.utf8_text(source).unwrap(),
            });
        }
        arg_strs
    }
}

pub struct NormalCommandNode<'a> {
    pub identifier: &'a str,
    pub identifier_node: Node<'a>,
    pub argument_list: Option<Node<'a>>,
    pub first_arg: Option<&'a str>,
    pub args: Vec<Node<'a>>,
}
/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn try_get_variable<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: T,
) -> Option<VariableNode<'a>>
where
    T: ToQueryRange,
{
    get_variables_inner(source, node, range).into_iter().next()
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_variables<'a>(
    source: &'a [u8],
    node: Node<'a>,
    end: impl Into<Option<Point>>,
) -> Vec<VariableNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_variables_inner::<QueryRange>(source, node, end)
}
/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
fn get_variables_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<VariableNode<'a>>
where
    T: ToQueryRange,
{
    let mut variables = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, VARIABLE_QUERY).unwrap();
    let mut cursor_vars = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_vars.set_point_range(range.start..range.end);
    }
    let mut matches_comments = cursor_vars.matches(&query_comment, node, source);
    while let Some(m) = matches_comments.next() {
        for c in m.captures {
            let node = c.node;

            let content = node.utf8_text(source).unwrap();
            variables.push(VariableNode { content, node });
        }
    }
    variables
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
#[allow(unused)]
pub fn get_argument_lists<'a>(
    source: &'a [u8],
    node: Node<'a>,
    end: impl Into<Option<Point>>,
) -> Vec<ArgumentListNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_argument_lists_inner::<QueryRange>(source, node, end)
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn try_get_argument_list<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    point: T,
) -> Option<ArgumentListNode<'a>>
where
    T: ToQueryRange,
{
    get_argument_lists_inner(source, node, point)
        .into_iter()
        .next()
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
fn get_argument_lists_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<ArgumentListNode<'a>>
where
    T: ToQueryRange,
{
    let mut arguments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, ARGUMENT_LIST_QUERY).unwrap();
    let mut cursor_argument = QueryCursor::new();
    if let Some(point) = range.into() {
        let range = point.to_query_range();
        cursor_argument.set_point_range(range.start..range.end);
    }
    let mut matches_comments = cursor_argument.matches(&query_comment, node, source);

    while let Some(m) = matches_comments.next() {
        let node = m.nodes_for_capture_index(0).next().unwrap();
        let mut ag_node = ArgumentListNode {
            main_node: node,
            arguments: vec![],
        };

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
    end: impl Into<Option<Point>>,
) -> Vec<LineCommentNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_line_comments_inner::<QueryRange>(source, node, end)
}

/// try get the brack comment
#[must_use]
pub fn try_get_line_comment<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    point: T,
) -> Option<LineCommentNode<'a>>
where
    T: ToQueryRange,
{
    get_line_comments_inner(source, node, point)
        .into_iter()
        .next()
}
/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_line_comments_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<LineCommentNode<'a>>
where
    T: ToQueryRange,
{
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, LINE_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_comments.set_point_range(range.start..range.end);
    }
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;

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
    end: impl Into<Option<Point>>,
) -> Vec<BracketCommentNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_bracket_comments_inner::<QueryRange>(source, node, end)
}
/// try get the brack comment
#[must_use]
pub fn try_get_bracket_comment<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    point: T,
) -> Option<BracketCommentNode<'a>>
where
    T: ToQueryRange,
{
    get_bracket_comments_inner(source, node, point)
        .into_iter()
        .next()
}
/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
fn get_bracket_comments_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<BracketCommentNode<'a>>
where
    T: ToQueryRange,
{
    // NOTE: prepare comments
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, BRACKET_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_comments.set_point_range(range.start..range.end);
    }
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;

            comments.push(BracketCommentNode {
                content: node.utf8_text(source).unwrap(),
                node,
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
    end: impl Into<Option<Point>>,
) -> Vec<MacroNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_macros_inner::<QueryRange>(source, node, end)
}

/// try get the macro
#[must_use]
pub fn try_get_macro<'a, T>(source: &'a [u8], node: Node<'a>, range: T) -> Option<MacroNode<'a>>
where
    T: ToQueryRange,
{
    get_macros_inner(source, node, range).into_iter().next()
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
fn get_macros_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<MacroNode<'a>>
where
    T: ToQueryRange,
{
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, MACRO_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_macro.set_point_range(range.start..range.end);
    }
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    while let Some(m) = matches_macro.next() {
        let Some(node) = m
            .captures
            .iter()
            .find(|c| c.node.kind() == CMakeNodeKinds::MACRO_DEF)
            .map(|c| c.node)
        else {
            continue;
        };
        let mut macro_node = MacroNode {
            node,
            name: "",
            arguments: vec![],
        };
        let args: Vec<&QueryCapture> = m
            .captures
            .iter()
            .filter(|c| c.node.kind() == CMakeNodeKinds::ARGUMENT)
            .collect();
        let first_arg = args[0].node;
        macro_node.name = first_arg.utf8_text(source).unwrap();
        macro_node.arguments = args.iter().map(|q| q.node).collect();
        macros.push(macro_node);
    }
    macros
}

/// max_height means when over this line, it will not count,
/// if you want to ignore it, use None
pub fn get_normal_commands<'a>(
    source: &'a [u8],
    node: Node<'a>,
    end: impl Into<Option<Point>>,
) -> Vec<NormalCommandNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_normal_commands_inner::<QueryRange>(source, node, end)
}

/// try get the command
#[must_use]
pub fn try_get_normal_command<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: T,
) -> Option<NormalCommandNode<'a>>
where
    T: ToQueryRange,
{
    get_normal_commands_inner(source, node, range)
        .into_iter()
        .next()
}

fn get_normal_commands_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<NormalCommandNode<'a>>
where
    T: ToQueryRange,
{
    let mut commands = vec![];
    let query_cmd = Query::new(&TREESITTER_CMAKE_LANGUAGE, NORMAL_COMMAND_QUERY).unwrap();
    let mut cursor_cmd = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_cmd.set_point_range(range.start..range.end);
    }
    let mut matches_cmd = cursor_cmd.matches(&query_cmd, node, source);

    while let Some(m) = matches_cmd.next() {
        let node = m.nodes_for_capture_index(0).next().unwrap();

        let Some(identifier) = node.child(0) else {
            continue;
        };
        if identifier.kind() != CMakeNodeKinds::IDENTIFIER {
            continue;
        }
        let identifier_node = identifier;
        let identifier = identifier.utf8_text(source).unwrap();
        let mut normal_command = NormalCommandNode {
            identifier,
            identifier_node,
            first_arg: None,
            argument_list: None,
            args: vec![],
        };
        // NOTE: child 1 is "(", it is child 2 that argument_list
        if let Some(argument_list) = node.child(2)
            && argument_list.kind() == CMakeNodeKinds::ARGUMENT_LIST
        {
            normal_command.argument_list = Some(argument_list);
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
    end: impl Into<Option<Point>>,
) -> Vec<FuncNode<'a>> {
    let end = end.into().map(|end| QueryRange {
        start: Point { row: 0, column: 0 },
        end,
    });
    get_functions_inner::<QueryRange>(source, node, end)
}
/// try get the command
#[must_use]
pub fn try_get_function<'a, T>(source: &'a [u8], node: Node<'a>, range: T) -> Option<FuncNode<'a>>
where
    T: ToQueryRange,
{
    get_functions_inner(source, node, range).into_iter().next()
}

fn get_functions_inner<'a, T>(
    source: &'a [u8],
    node: Node<'a>,
    range: impl Into<Option<T>>,
) -> Vec<FuncNode<'a>>
where
    T: ToQueryRange,
{
    let mut funs = vec![];
    let query_fun = Query::new(&TREESITTER_CMAKE_LANGUAGE, FUNCTION_QUERY).unwrap();
    let mut cursor_fun = QueryCursor::new();
    if let Some(range) = range.into() {
        let range = range.to_query_range();
        cursor_fun.set_point_range(range.start..range.end);
    }
    let mut matches_fun = cursor_fun.matches(&query_fun, node, source);

    while let Some(m) = matches_fun.next() {
        let Some(node) = m
            .captures
            .iter()
            .find(|c| c.node.kind() == CMakeNodeKinds::FUNCTION_DEF)
            .map(|c| c.node)
        else {
            continue;
        };
        let mut fun_node = FuncNode {
            node,
            name: "",
            arguments: vec![],
        };
        let args: Vec<&QueryCapture> = m
            .captures
            .iter()
            .filter(|c| c.node.kind() == CMakeNodeKinds::ARGUMENT)
            .collect();
        let first_arg = args[0].node;
        fun_node.name = first_arg.utf8_text(source).unwrap();
        fun_node.arguments = args.iter().map(|q| q.node).collect();
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
