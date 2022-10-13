use lsp_types::{MessageType, Position, TextEdit};
mod adddefinitions;
mod functiondef;
mod ifcondition;
mod loopdef;
mod macrodef;
mod othercommand;
mod project;
mod set;
//use crate::utils::treehelper::point_to_position;
pub async fn getformat(source: &str, client: &tower_lsp::Client) -> Option<Vec<TextEdit>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(source, None).unwrap();
    let formatresult = get_format_from_root_node(tree.root_node(), source);
    //println!("{:?}", formatresult);
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
        let mut new_text = String::new();
        let mut course = input.walk();
        let mut startline = 0;
        for child in input.children(&mut course) {
            let childstartline = child.start_position().row;
            let reformat = get_format_from_node(child, source);
            //down += downpoint;
            for _ in startline..childstartline {
                new_text.push('\n');
            }
            new_text.push_str(&reformat);
            startline = child.end_position().row;
        }
        let len_ot = new_text.lines().count();
        let len_origin = source.lines().count();
        let len = std::cmp::max(len_ot, len_origin);
        Some(vec![TextEdit {
            range: lsp_types::Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: len as u32,
                    character: 0,
                },
            },
            new_text,
        }])
    }
}
pub fn get_format_cli(input: tree_sitter::Node, source: &str) -> Option<String> {
    if input.has_error() {
        None
    } else {
        let mut new_text = String::new();
        let mut course = input.walk();
        let mut startline = 0;
        for child in input.children(&mut course) {
            let childstartline = child.start_position().row;
            let reformat = get_format_from_node(child, source);
            //down += downpoint;
            for _ in startline..childstartline {
                new_text.push('\n');
            }
            new_text.push_str(&reformat);
            startline = child.end_position().row;
        }
        Some(new_text)
    }
}
fn get_format_from_node(input: tree_sitter::Node, source: &str) -> String {
    match CommandType::from_node(input, source) {
        CommandType::Project => project::format_project(input, source),
        CommandType::Set => set::format_set(input, source),
        CommandType::AddDefinitions => adddefinitions::format_definition(input, source),
        CommandType::OtherCommand => othercommand::format_othercommand(input, source),
        CommandType::IfCondition => ifcondition::format_ifcondition(input, source),
        CommandType::Loop => loopdef::format_loopdef(input, source),
        CommandType::MacroDef => macrodef::format_macrodef(input, source),
        CommandType::FunctionDef => functiondef::format_functiondef(input, source),
        _ => default_format(input, source),
    }
}

fn default_format(input: tree_sitter::Node, source: &str) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let start_position = input.start_position();
    let end_position = input.end_position();
    let start_x = start_position.column;
    let start_y = start_position.row;
    let end_x = end_position.column;
    let end_y = end_position.row;
    if start_y == end_y {
        newsource[start_y][start_x..end_x].to_string()
    } else {
        let mut output = String::new();
        output.push_str(&format!("{}\n", &newsource[start_y][start_x..]));
        for item in newsource.iter().take(end_y).skip(start_y + 1) {
            output.push_str(&format!("{}\n", item));
        }
        output.push_str(&format!("{}\n", &newsource[end_y][0..end_x]));
        output
    }
}

#[derive(Debug, PartialEq)]
enum CommandType {
    Set,
    //Option,
    Project,
    AddDefinitions,
    //FindPackage,
    IfCondition,
    MacroDef,
    FunctionDef,
    Loop,
    LineComment,
    OtherCommand,
}

impl CommandType {
    fn from_node(node: tree_sitter::Node, source: &str) -> Self {
        let newsource: Vec<&str> = source.lines().collect();
        match node.kind() {
            "if_condition" => Self::IfCondition,
            "foreach_loop" => Self::Loop,
            "macro_def" => Self::MacroDef,
            "function_def" => Self::FunctionDef,
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
                    "add_definitions" => CommandType::AddDefinitions,
                    //"option" => CommandType::Option,
                    "project" => CommandType::Project,
                    //"find_package" => CommandType::FindPackage,
                    _ => Self::OtherCommand,
                }
            }
            "line_comment" => Self::LineComment,
            _ => Self::OtherCommand,
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
fn node_to_string(node: tree_sitter::Node, source: &str) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let startx = node.start_position().column;
    let starty = node.start_position().row;
    let endx = node.end_position().column;
    let endy = node.end_position().row;
    let mut output = String::new();
    output.push_str(&newsource[starty][startx..]);
    output.push('\n');
    for item in newsource.iter().take(endy).skip(starty + 1) {
        output.push_str(item);
        output.push('\n');
    }
    output.push_str(&newsource[endy][0..endx]);
    output
}
#[test]
fn tst_node_to_str() {
    let a = r#"
set(
  CMAKE_CXX_FLAGS
  "${CMAKE_CXX_FLAGS} \
  -Wall \
  -Wextra \
  -pipe \
  -pedantic \
  -fsized-deallocation \
  -fdiagnostics-color=always \
  -Wunreachable-code \
  -Wno-attributes"
)
    "#;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(a, None).unwrap();
    let e = node_to_string(tree.root_node(), a);
    assert_eq!(a, e);
}
