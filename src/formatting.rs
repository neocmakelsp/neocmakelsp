use lsp_types::{MessageType, Position, TextEdit};
//use tree_sitter::Node;
mod findpackage;
mod project;
mod set;
//use crate::utils::treehelper::point_to_position;
pub async fn getformat(source: &str, client: &tower_lsp::Client) -> Option<Vec<TextEdit>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(source, None).unwrap();
    let formatresult = get_format_from_root_node(tree.root_node(), source);
    println!("{:?}", formatresult);
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
        for child in input.children(&mut course) {
            let (reformat, _) = get_format_from_node(child, source);
            //down += downpoint;
            new_text.push_str(&format!("{}\n", reformat));
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

fn get_format_from_node(input: tree_sitter::Node, source: &str) -> (String, usize) {
    // first one is the textedit, second one is down move
    //let newsource: Vec<&str> = source.lines().collect();
    //let mut output = String::new();
    let output = match CommandType::from_node(input.clone(), source) {
        CommandType::Project => project::format_project(input, source),
        _ => default_format(input, source),
    };
    let count = output.lines().count();
    (output, count)
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
        output.push_str(&format!("{}\n",&newsource[start_y][start_x..]));
        for i in start_y+1..end_y {
            output.push_str(&format!("{}\n",newsource[i]));
        }
        output.push_str(&format!("{}\n",&newsource[end_y][0..end_x]));
        output
    }
    //let mut cursor = input.walk();
    //for child in input.children(&mut cursor) {}
}

#[derive(Debug, PartialEq)]
enum CommandType {
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
//#[test]
//fn tst_format() {
//    let mut parse = tree_sitter::Parser::new();
//    parse.set_language(tree_sitter_cmake::language()).unwrap();
//    let source = r#"
//    project(
//    Dtk
//    )
//    "#;
//    let tree = parse.parse(source, None).unwrap();
//    let node = tree.root_node().child(0).unwrap();
//    let (afformat, _) = format_project(node, source, 0, 0);
//    for unit in afformat {
//        println!(" {:?} ", unit);
//    }
//}
