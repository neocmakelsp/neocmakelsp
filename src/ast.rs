use lsp_types::{DocumentSymbol, DocumentSymbolResponse, MessageType, SymbolKind};
use tower_lsp::{lsp_types, Client};

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
/// Get the tree of ast
use crate::utils::treehelper::ToPosition;
use crate::CMakeNodeKinds;

const COMMAND_KEYWORDS: [&str; 5] = [
    "set",
    "option",
    "project",
    "target_link_libraries",
    "target_include_directories",
];

pub async fn getast(client: &Client, context: &str) -> Option<DocumentSymbolResponse> {
    let line = context.lines().count();
    if line > 10000 {
        client
            .log_message(MessageType::INFO, "use simple ast")
            .await;
    }
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    getsubast(tree.root_node(), &context.lines().collect(), line > 10000)
        .map(DocumentSymbolResponse::Nested)
}

#[allow(deprecated)]
fn getsubast(
    input: tree_sitter::Node,
    source: &Vec<&str>,
    simple: bool,
) -> Option<Vec<DocumentSymbol>> {
    let mut course = input.walk();
    let mut asts: Vec<DocumentSymbol> = vec![];
    for child in input.children(&mut course) {
        match child.kind() {
            CMakeNodeKinds::FUNCTION_DEF => {
                let Some(ids) = child.child(0) else {
                    continue;
                };
                let Some(argumentlists) = ids.child(2) else {
                    continue;
                };
                let Some(function_name) = argumentlists.child(0) else {
                    continue;
                };
                let x = function_name.start_position().column;
                let y = function_name.end_position().column;
                let h = function_name.start_position().row;
                let Some(name) = &source[h][x..y].split(' ').next() else {
                    continue;
                };

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
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            CMakeNodeKinds::MACRO_DEF => {
                let Some(ids) = child.child(0) else {
                    continue;
                };
                let Some(argumentlists) = ids.child(2) else {
                    continue;
                };
                let Some(marco_name) = argumentlists.child(0) else {
                    continue;
                };
                let x = marco_name.start_position().column;
                let y = marco_name.end_position().column;
                let h = marco_name.start_position().row;
                let Some(name) = &source[h][x..y].split(' ').next() else {
                    continue;
                };
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
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            CMakeNodeKinds::BODY => {
                let Some(mut bodycontent) = getsubast(child, source, simple) else {
                    continue;
                };
                asts.append(&mut bodycontent);
            }
            CMakeNodeKinds::IF_CONDITION | CMakeNodeKinds::FOREACH_LOOP => {
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
                    children: if simple {
                        None
                    } else {
                        getsubast(child, source, simple)
                    },
                });
            }
            CMakeNodeKinds::NORMAL_COMMAND => {
                let start = child.start_position().to_position();
                let end = child.end_position().to_position();
                let h = child.start_position().row;
                let Some(ids) = child.child(0) else {
                    continue;
                };
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let command_name = &source[h][x..y];
                if COMMAND_KEYWORDS.contains(&command_name.to_lowercase().as_str()) {
                    let Some(argumentlists) = child.child(2) else {
                        continue;
                    };
                    let Some(ids) = argumentlists.child(0) else {
                        continue;
                    };
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let h2 = ids.end_position().row;
                        if h != h2 {
                            continue;
                        }
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let varname = &source[h][x..y];
                        asts.push(DocumentSymbol {
                            name: format!("{command_name}: {varname}"),
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
            _ => {}
        }
    }
    if asts.is_empty() {
        None
    } else {
        Some(asts)
    }
}

#[cfg(test)]
mod ast_test {
    use super::*;
    #[test]
    fn test_ast_1() {
        let context = include_str!("../assert/ast_test/bast_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(getsubast(thetree.root_node(), &context.lines().collect(), false).is_some());
    }

    #[test]
    fn test_ast_2() {
        let context = include_str!("../assert/ast_test/nheko_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(getsubast(thetree.root_node(), &context.lines().collect(), false).is_some());
    }

    #[test]
    fn test_ast_3() {
        let context = r#"
# Just comment here
"#;
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(getsubast(thetree.root_node(), &context.lines().collect(), false).is_none());
    }
}
