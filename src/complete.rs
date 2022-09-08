mod findpackage;
use crate::snippets::{BUILDIN_COMMAND, BUILDIN_MODULE, BUILDIN_VARIABLE};
use crate::CompletionResponse;
use lsp_types::{CompletionItem, CompletionItemKind};
/// get the complet messages
pub fn getcoplete(input: tree_sitter::Node, source: &str) -> Option<CompletionResponse> {
    //let mut course2 = course.clone();
    //let mut hasid = false;
    let mut complete: Vec<CompletionItem> = vec![];
    if let Some(mut message) = getsubcoplete(input, source) {
        complete.append(&mut message);
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
