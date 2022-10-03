use lsp_types::{MessageType, TextEdit};
use tree_sitter::Node;

use crate::utils::treehelper::point_to_position;
pub async fn getformat(source: &str, client: &tower_lsp::Client) -> Option<Vec<TextEdit>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(source, None).unwrap();
    let formatresult = get_format_from_root_node(tree.root_node(), source);
    if formatresult.is_none() {
        client
            .log_message(MessageType::WARNING, "Error source")
            .await;
    }
    formatresult
}

pub fn get_format_from_root_node(input: tree_sitter::Node, source: &str) -> Option<Vec<TextEdit>> {
    if input.has_error() {
        None
    } else {
        let (output, _) = get_format_from_node(input, source, 0);
        Some(output)
    }
}

fn get_format_from_node(input: tree_sitter::Node, source: &str, down: i32) -> (Vec<TextEdit>, i32) {
    // first one is the textedit, second one is down move
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = vec![];
    let mut down = down;
    match CommandType::from_node(input.clone(), source) {
        CommandType::SourceFile => {
            let mut course = input.walk();
            for child in input.children(&mut course) {
                let (mut reformat, downpoint) = get_format_from_node(child, source, down);
                down += downpoint;
                output.append(&mut reformat);
            }
        }
        CommandType::Project => {
            format_project(input,source, down);
        }
        _ => {}
    }
    (output, 0)
}
fn format_project(input: tree_sitter::Node, source: &str,down: i32) -> (Vec<TextEdit>, i32) {
    todo!()
}

trait AsString {
    fn to_string() -> String;
}

impl AsString for Node<'_> {
    fn to_string() -> String {
        todo!()
    }
}
#[derive(Debug, PartialEq)]
enum CommandType {
    SourceFile,
    Set,
    Option,
    Project,
    FindPackage,
    Closure,
    LineComment,
    UnKnown,
}

impl CommandType {
    fn from_node(node: tree_sitter::Node, source: &str) -> Self {
        let newsource: Vec<&str> = source.lines().collect();
        match node.kind() {
            "source_file" => Self::SourceFile,
            "if_condition" | "foreach_loop" => Self::Closure,
            "normal_command" => {
                let h = node.start_position().row;
                let ids = node.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y].to_lowercase();
                let name = name.as_str();
                match name {
                    "set" => CommandType::Set,
                    "option" => CommandType::Option,
                    "project" => CommandType::Project,
                    "find_package" => CommandType::FindPackage,
                    _ => Self::UnKnown,
                }
            }
            "line_comment" => Self::LineComment,
            _ => Self::UnKnown,
        }
    }
}
#[test]
fn tst_type() {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse("set(A 10)", None).unwrap();
    let node = tree.root_node().child(0).unwrap();
    assert_eq!(CommandType::Set, CommandType::from_node(node, "set(A 10)"));
    let tree = parse.parse("project(Mime)", None).unwrap();
    let node = tree.root_node().child(0).unwrap();
    assert_eq!(
        CommandType::Project,
        CommandType::from_node(node, "project(Mime)")
    );
}
