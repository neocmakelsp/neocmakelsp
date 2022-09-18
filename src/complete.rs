// todo compelete type
mod buildin;
mod findpackage;
use crate::utils::types::*;
use crate::CompletionResponse;
use buildin::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position};
/// get the complet messages
pub async fn getcoplete(
    source: &str,
    location: Position,
    client: &tower_lsp::Client,
) -> Option<CompletionResponse> {
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let mut complete: Vec<CompletionItem> = vec![];
    match get_input_type(location, tree.root_node(), source, InputType::Variable) {
        InputType::Variable => {
            if let Some(mut message) = getsubcoplete(tree.root_node(), source) {
                complete.append(&mut message);
            }

            if let Ok(messages) = &*BUILDIN_COMMAND {
                complete.append(&mut messages.clone());
            }
            if let Ok(messages) = &*BUILDIN_VARIABLE {
                complete.append(&mut messages.clone());
            }
        }
        InputType::FindPackage => {
            if let Ok(package) = &*findpackage::CMAKE_SOURCE {
                complete.append(&mut package.clone());
            }
        }
        InputType::Include => {
            if let Ok(messages) = &*BUILDIN_MODULE {
                complete.append(&mut messages.clone());
            }
        }
        _ => {}
    }

    if complete.is_empty() {
        client.log_message(MessageType::INFO, "Empty").await;
        None
    } else {
        Some(CompletionResponse::Array(complete))
    }
}
/// get the variable from the loop
fn getsubcoplete(input: tree_sitter::Node, source: &str) -> Option<Vec<CompletionItem>> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut complete: Vec<CompletionItem> = vec![];
    for child in input.children(&mut course) {
        match child.kind() {
            "function_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                complete.push(CompletionItem {
                    label: format!("{}()", name),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("defined function".to_string()),
                    ..Default::default()
                });
            }
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) = getsubcoplete(child, source) {
                    complete.append(&mut message);
                }
            }
            "normal_command" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                if name == "set" || name == "SET" || name == "option" {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        complete.push(CompletionItem {
                            label: name.to_string(),
                            kind: Some(CompletionItemKind::VALUE),
                            detail: Some("defined variable".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}
