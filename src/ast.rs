/// Get the tree of ast
use crate::utils::treehelper::point_to_position;
use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolKind};
#[allow(deprecated)]
pub fn getast(input: tree_sitter::Node, source: &str) -> Option<DocumentSymbolResponse> {
    //match getsubast(input, source) {
    //    Some(asts) => Some(DocumentSymbolResponse::Nested(asts)),
    //    None => None,
    //}
    getsubast(input, source).map(DocumentSymbolResponse::Nested)
}
#[allow(deprecated)]
fn getsubast(input: tree_sitter::Node, source: &str) -> Option<Vec<DocumentSymbol>> {
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
                    name: "Closure".to_string(),
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
                let start = point_to_position(child.start_position());
                let end = point_to_position(child.end_position());
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
                        if x != y {
                            let name = &newsource[h][x..y];
                            asts.push(DocumentSymbol {
                                name: name.to_string(),
                                detail: None,
                                kind: SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                range: lsp_types::Range { start, end },
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
