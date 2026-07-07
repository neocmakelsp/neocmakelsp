use lsp_types::{DocumentSymbol, DocumentSymbolResponse, MessageType, SymbolKind};
use tower_lsp::{Client, lsp_types};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::CMakeNodeKinds;
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::query::{AstNode, AstNodeContainer};
use crate::utils::treehelper::ToPosition;

const COMMAND_KEYWORDS: [&str; 5] = [
    "set",
    "option",
    "project",
    "target_link_libraries",
    "target_include_directories",
];

pub async fn get_symbol(client: &Client, context: &str) -> Option<DocumentSymbolResponse> {
    let line = context.lines().count();
    if line > 10000 {
        client
            .log_message(MessageType::Info, "use simple ast")
            .await;
    }
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    get_sub_symbol(tree.root_node(), context.as_bytes(), line > 10000)
        .map(DocumentSymbolResponse::DocumentSymbolList)
}

const QUERY_SOURCE: &str = include_str!("../misc/document_symbol.scm");

#[derive(Debug, Default)]
enum SymbolData<'a> {
    #[default]
    Block,
    Function {
        name: &'a str,
    },
    Command {
        name: &'a str,
        target: &'a str,
    },
}

fn get_symbols(node: tree_sitter::Node, source: &str) -> Vec<DocumentSymbol> {
    let query_source = tree_sitter_cmake::HIGHLIGHTS_QUERY;
    let query = Query::new(&TREESITTER_CMAKE_LANGUAGE, query_source).unwrap();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, node, source.as_bytes());

    let mut container: AstNodeContainer<'_, SymbolData<'_>> = AstNodeContainer::new();
    let names = query.capture_names();
    'out: while let Some(m) = matches.next() {
        let mut data_name = None;
        let mut identifier = None;
        let mut first_argument = None;
        let mut node = None;
        for e in m.captures {
            let name = names[e.index as usize];
            if name == "block" {
                let ast_node = AstNode::new(e.node, name).with_data(SymbolData::Block);
                container.insert_node(ast_node);
                continue 'out;
            }
            if matches!(name, "function" | "command") {
                data_name = Some(name);
                node = Some(e.node);
                continue;
            }
            if name == "identifier" {
                identifier = Some(e.node.utf8_text(source.as_bytes()).unwrap());
                continue;
            }
            if name == "first_arg" {
                first_argument = Some(e.node.utf8_text(source.as_bytes()).unwrap());
            }
        }
        let (Some(name), Some(identifier), Some(node)) = (data_name, identifier, node) else {
            continue;
        };

        if name != "command" {
            let ast_node =
                AstNode::new(node, name).with_data(SymbolData::Function { name: identifier });
            container.insert_node(ast_node);
            continue;
        }
        let Some(first_arg) = first_argument else {
            continue;
        };

        let ast_node = AstNode::new(node, name).with_data(SymbolData::Command {
            name: identifier,
            target: first_arg,
        });
        container.insert_node(ast_node);
    }
    todo!()
}

#[allow(deprecated)]
fn get_sub_symbol(
    input: tree_sitter::Node,
    source: &[u8],
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
                let Ok(name) = function_name.utf8_text(source) else {
                    continue;
                };

                asts.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: SymbolKind::Function,
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
                        get_sub_symbol(child, source, simple)
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
                let Ok(name) = marco_name.utf8_text(source) else {
                    continue;
                };
                asts.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: SymbolKind::Function,
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
                        get_sub_symbol(child, source, simple)
                    },
                });
            }
            CMakeNodeKinds::BODY => {
                let Some(mut bodycontent) = get_sub_symbol(child, source, simple) else {
                    continue;
                };
                asts.append(&mut bodycontent);
            }
            CMakeNodeKinds::IF_CONDITION | CMakeNodeKinds::FOREACH_LOOP => {
                asts.push(DocumentSymbol {
                    name: "Closure".to_string(),
                    detail: None,
                    kind: SymbolKind::Namespace,
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
                        get_sub_symbol(child, source, simple)
                    },
                });
            }
            CMakeNodeKinds::NORMAL_COMMAND => {
                let start = child.start_position().to_position();
                let end = child.end_position().to_position();
                let Some(ids) = child.child(0) else {
                    continue;
                };
                let Ok(command_name) = ids.utf8_text(source) else {
                    continue;
                };
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
                        let Ok(varname) = ids.utf8_text(source) else {
                            continue;
                        };
                        asts.push(DocumentSymbol {
                            name: format!("{command_name}: {varname}"),
                            detail: None,
                            kind: SymbolKind::Variable,
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
    if asts.is_empty() { None } else { Some(asts) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_1() {
        let context = include_str!("../assets_for_test/ast_test/bast_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(get_sub_symbol(thetree.root_node(), context.as_bytes(), false).is_some());
    }

    #[test]
    fn test_ast_2() {
        let context = include_str!("../assets_for_test/ast_test/nheko_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(get_sub_symbol(thetree.root_node(), context.as_bytes(), false).is_some());
    }

    #[test]
    fn test_ast_3() {
        let context = r"
# Just comment here
";
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(get_sub_symbol(thetree.root_node(), context.as_bytes(), false).is_none());
    }
}
