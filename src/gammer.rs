/// about the gammers
use crate::snippets::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use crate::CompletionResponse;
use lsp_types::{CompletionItem, CompletionItemKind};
/// checkerror the gammer error
/// if there is error , it will return the position of the error
pub fn checkerror(
    input: tree_sitter::Node,
) -> Option<Vec<(tree_sitter::Point, tree_sitter::Point)>> {
    if input.has_error() {
        if input.is_error() {
            Some(vec![(input.start_position(), input.end_position())])
        } else {
            let mut course = input.walk();
            {
                let mut output = vec![];
                for node in input.children(&mut course) {
                    if let Some(mut tran) = checkerror(node) {
                        output.append(&mut tran);
                    }
                }
                if output.is_empty() {
                    None
                } else {
                    Some(output)
                }
            }
        }
    } else {
        None
    }
}
/// get the complet messages
pub fn getcoplete(input: tree_sitter::Node, source: &str) -> Option<CompletionResponse> {
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
                if name == "set" || name == "SET" {
                    let ids = child.child(2).unwrap();
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
            _ => {}
        }
    }
    if let Ok(messages) = &*BUILDIN_COMMAND {
        complete.append(&mut messages.clone());
    }
    if let Ok(messages) = &*BUILDIN_VARIABLE {
        complete.append(&mut messages.clone());
    }
    if let Ok(messages) = &*BUILDIN_MODULE {
        complete.append(&mut messages.clone());
    }
    if complete.is_empty() {
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
                if name == "set" || name == "SET" {
                    let ids = child.child(2).unwrap();
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
            _ => {}
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}
