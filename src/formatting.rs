use lsp_types::{MessageType, Position, TextEdit};
mod adddefinitions;
mod functiondef;
mod ifcondition;
mod loopdef;
mod macrodef;
mod othercommand;

const NOT_FORMAT_ME: &str = "# Not Format Me";

fn strip_trailing_newline(input: &str) -> &str {
    input
        .strip_suffix("\r\n")
        .or(input.strip_suffix('\n'))
        .unwrap_or(input)
}

// remove all \r to normal one
fn strip_trailing_newline_document(input: &str) -> String {
    let cll: Vec<&str> = input.lines().map(strip_trailing_newline).collect();
    let mut output = String::new();

    for line in cll {
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn get_space(spacelen: u32, usespace: bool) -> String {
    let unit = if usespace { ' ' } else { '\t' };
    let mut space = String::new();
    for _ in 0..spacelen {
        space.push(unit);
    }
    space
}

// use crate::utils::treehelper::point_to_position;
pub async fn getformat(
    source: &str,
    client: &tower_lsp::Client,
    spacelen: u32,
    usespace: bool,
) -> Option<Vec<TextEdit>> {
    let source = strip_trailing_newline_document(source);
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(source.as_str(), None).unwrap();
    let formatresult =
        get_format_from_root_node(tree.root_node(), source.as_str(), spacelen, usespace);
    if formatresult.is_none() {
        client
            .log_message(MessageType::WARNING, "Error source")
            .await;
    }
    formatresult
}

pub fn get_format_from_root_node(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> Option<Vec<TextEdit>> {
    if input.has_error() {
        None
    } else {
        let mut new_text = String::new();
        let mut course = input.walk();
        let mut startline = 0;
        let mut not_format = false;
        for child in input.children(&mut course) {
            let childstartline = child.start_position().row;
            let reformat = if not_format {
                not_format = false;
                get_origin_source(child, source)
            } else {
                if is_notformat_mark(child, source) {
                    not_format = true;
                }
                get_format_from_node(child, source, spacelen, usespace)
            };
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

pub fn get_format_cli(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> Option<String> {
    if input.has_error() {
        None
    } else {
        let source = strip_trailing_newline_document(source);
        let mut new_text = String::new();
        let mut course = input.walk();
        let mut startline = 0;
        let mut not_format = false;
        for child in input.children(&mut course) {
            let childstartline = child.start_position().row;
            let reformat = if not_format {
                not_format = false;
                get_origin_source(child, source.as_str())
            } else {
                if is_notformat_mark(child, source.as_str()) {
                    not_format = true;
                }
                get_format_from_node(child, source.as_str(), spacelen, usespace)
            };
            for _ in startline..childstartline {
                new_text.push('\n');
            }
            new_text.push_str(&reformat);
            startline = child.end_position().row;
        }
        Some(new_text)
    }
}

fn get_origin_source(input: tree_sitter::Node, source: &str) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let start_y = input.start_position().row;
    let end_y = input.end_position().row;
    let mut output = String::new();
    for line in newsource.iter().take(end_y + 1).skip(start_y) {
        output.push_str(line);
        output.push('\n');
    }
    output.pop();
    output
}

fn get_format_from_node(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> String {
    match CommandType::from_node(input, source) {
        CommandType::AddDefinitions => adddefinitions::format_definition(input, source),
        CommandType::OtherCommand => {
            othercommand::format_othercommand(input, source, spacelen, usespace)
        }
        CommandType::IfCondition => {
            ifcondition::format_ifcondition(input, source, spacelen, usespace)
        }
        CommandType::Loop => loopdef::format_loopdef(input, source, spacelen, usespace),
        CommandType::MacroDef => macrodef::format_macrodef(input, source, spacelen, usespace),
        CommandType::FunctionDef => {
            functiondef::format_functiondef(input, source, spacelen, usespace)
        }
        _ => default_format(input, source),
    }
}

fn is_notformat_mark(input: tree_sitter::Node, source: &str) -> bool {
    if CommandType::LineComment != CommandType::from_node(input, source) {
        return false;
    };
    let newsource: Vec<&str> = source.lines().collect();
    let start_position = input.start_position();
    let end_position = input.end_position();
    if start_position.row != end_position.row {
        return false;
    }
    let start_y = start_position.row;
    let start_x = start_position.column;
    let end_x = end_position.column;

    let comment = newsource[start_y][start_x..end_x].to_string();
    comment == NOT_FORMAT_ME
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
            output.push_str(&format!("{item}\n"));
        }
        output.push_str(&newsource[end_y][0..end_x]);
        output
    }
}

#[derive(Debug, PartialEq)]
enum CommandType {
    //Option,
    AddDefinitions,
    //FindPackage,
    IfCondition,
    MacroDef,
    FunctionDef,
    Loop,
    LineComment,
    BranketComment,
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
                    "add_definitions"
                    | "add_compile_definitions"
                    | "target_compile_definitions" => CommandType::AddDefinitions,
                    //"option" => CommandType::Option,
                    //"find_package" => CommandType::FindPackage,
                    _ => Self::OtherCommand,
                }
            }
            "bracket_comment" => Self::BranketComment,
            "line_comment" => Self::LineComment,
            _ => Self::OtherCommand,
        }
    }
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

#[test]
fn tst_is_notformat_me() {
    let a = NOT_FORMAT_ME;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(a, None).unwrap();
    assert!(is_notformat_mark(tree.root_node().child(0).unwrap(), a));
}

#[test]
fn strip_newline_works() {
    assert_eq!(
        strip_trailing_newline_document("Test0\r\n\r\n"),
        "Test0\n\n"
    );
    assert_eq!(strip_trailing_newline("Test1\r\n"), "Test1");
    assert_eq!(strip_trailing_newline("Test2\n"), "Test2");
    assert_eq!(strip_trailing_newline("Test3"), "Test3");
}
