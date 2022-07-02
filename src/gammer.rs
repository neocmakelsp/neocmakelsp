use lsp_types::{CompletionItem, CompletionItemKind};

use crate::CompletionResponse;
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
                    label: format!("{}()",name),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("defined function".to_string()),
                    ..Default::default()
                });
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
        Some(CompletionResponse::Array(complete))
    }
}
//    for child in input.children(&mut course) {
//        match child.kind() {
//            "winid" => {
//                let h = child.start_position().row;
//                let ids = child.child(2).unwrap();
//                let x = ids.start_position().column;
//                let y = ids.end_position().column;
//                let name = &newsource[h][x..y];
//                println!("name= {}", name);
//                if name == id {
//                    println!("test");
//                    hasid = true;
//                } else {
//                    hasid = false;
//                }
//            }
//            "widgetid" => {
//                let h = child.start_position().row;
//                let ids = child.child(0).unwrap();
//                let x = ids.start_position().column;
//                let y = ids.end_position().column;
//                let name = &newsource[h][x..y];
//                complete.push(CompletionItem {
//                    label: name.to_string(),
//                    kind: Some(CompletionItemKind::VALUE),
//                    detail: Some("message".to_string()),
//                    ..Default::default()
//                });
//            }
//            "qml_function" => {
//                let h = child.start_position().row;
//                let ids = child.child(1).unwrap();
//                let x = ids.start_position().column;
//                let y = ids.end_position().column;
//                let name = &newsource[h][x..y];
//                complete.push(CompletionItem {
//                    label: name.to_string(),
//                    kind: Some(CompletionItemKind::FUNCTION),
//                    detail: Some("message".to_string()),
//                    ..Default::default()
//                });
//            }
//            "qmlwidget" => {
//                let output = getcoplete(child, source, id);
//                if output.is_some() {
//                    return output;
//                }
//            }
//            _ => {}
//        }
//    }
//    if hasid {
//        Some(CompletionResponse::Array(complete))
//    } else {
//        None
//    }
//}
//#[cfg(test)]
//mod gammertests {
//    #[test]
//    fn test_complete() {
//        let source = "A { id : window function a() {} name: beta  }";
//        let mut parse = tree_sitter::Parser::new();
//        parse.set_language(tree_sitter_qml::language()).unwrap();
//        let tree = parse.parse(source, None).unwrap();
//        let root = tree.root_node();
//        println!("{}", root.to_sexp());
//        let a = super::getcoplete(root, source, "window");
//        println!("{:#?}", a);
//    }
//}
