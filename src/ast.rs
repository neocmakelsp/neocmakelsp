use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolKind};
#[allow(deprecated)]
pub fn getast(input: tree_sitter::Node, source: &str) -> Option<DocumentSymbolResponse> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    let mut asts: Vec<DocumentSymbol> = vec![];
    for child in input.children(&mut course) {
        match child.kind() {
            "function_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                asts.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: getsubast(child, source),
                });
            }
            "if_condition" | "foreach_loop" => {
                asts.push(DocumentSymbol {
                    name: "Loop".to_string(),
                    detail: None,
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: getsubast(child, source),
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
                    asts.push(DocumentSymbol {
                        name: name.to_string(),
                        detail: None,
                        kind: SymbolKind::VARIABLE,
                        tags: None,
                        deprecated: None,
                        range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: h as u32,
                                character: x as u32,
                            },
                            end: lsp_types::Position {
                                line: h as u32,
                                character: y as u32,
                            },
                        },
                        selection_range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: h as u32,
                                character: x as u32,
                            },
                            end: lsp_types::Position {
                                line: h as u32,
                                character: y as u32,
                            },
                        },
                        children: None,
                    });
                }
            }
            _ => {}
        }
    }
    if asts.is_empty() {
        None
    } else {
        Some(DocumentSymbolResponse::Nested(asts))
    }
}
#[allow(deprecated)]
fn getsubast(input: tree_sitter::Node, source: &str) -> Option<Vec<DocumentSymbol>> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    let mut asts: Vec<DocumentSymbol> = vec![];
    for child in input.children(&mut course) {
        match child.kind() {
            "function_def" => {
                asts.push(DocumentSymbol {
                    name: "Function".to_string(),
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: getsubast(child, source),
                });
            }
            "if_condition" | "foreach_loop" => {
                asts.push(DocumentSymbol {
                    name: "Loop".to_string(),
                    detail: None,
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: child.start_position().row as u32,
                            character: child.start_position().column as u32,
                        },
                        end: lsp_types::Position {
                            line: child.end_position().row as u32,
                            character: child.end_position().column as u32,
                        },
                    },
                    children: getsubast(child, source),
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
                    //let name = &newsource[h][x..y];
                    asts.push(DocumentSymbol {
                        name: "variabel".to_string(),
                        detail: None,
                        kind: SymbolKind::VARIABLE,
                        tags: None,
                        deprecated: None,
                        range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: h as u32,
                                character: x as u32,
                            },
                            end: lsp_types::Position {
                                line: h as u32,
                                character: y as u32,
                            },
                        },
                        selection_range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: h as u32,
                                character: x as u32,
                            },
                            end: lsp_types::Position {
                                line: h as u32,
                                character: y as u32,
                            },
                        },
                        children: None,
                    });
                    //complete.push(CompletionItem {
                    //    label: name.to_string(),
                    //    kind: Some(CompletionItemKind::VALUE),
                    //    detail: Some("defined variable".to_string()),
                    //    ..Default::default()
                    //});
                }
            }
            _ => {}
        }
    }
    if asts.is_empty() {
        None
    } else {
        Some(asts)
    }
}
