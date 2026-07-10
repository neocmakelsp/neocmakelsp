use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolKind};
use tower_lsp::{Client, lsp_types};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::query::{AstNode, AstNodeContainer, ToLspRange};

const COMMAND_KEYWORDS: [&str; 5] = [
    "set",
    "option",
    "project",
    "target_link_libraries",
    "target_include_directories",
];

pub async fn get_symbol(_client: &Client, context: &str) -> Option<DocumentSymbolResponse> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    let symbols = get_symbols(tree.root_node(), context);
    if symbols.is_empty() {
        None
    } else {
        Some(DocumentSymbolResponse::DocumentSymbolList(symbols))
    }
}

const QUERY_SOURCE: &str = include_str!("../misc/document_symbol.scm");

#[derive(Debug, Default)]
enum SymbolData<'a> {
    #[default]
    Block,
    IFBlock,
    Function {
        name: &'a str,
    },
    Command {
        name: &'a str,
        target: &'a str,
        target_node: Node<'a>,
    },
}

trait NodeGetSymbol {
    fn dom_symbol(&self) -> DocumentSymbol;
}

trait ContainerGetSymbols {
    fn dom_symbols(&self) -> Vec<DocumentSymbol>;
}

impl<'a> ContainerGetSymbols for AstNodeContainer<'a, SymbolData<'a>> {
    fn dom_symbols(&self) -> Vec<DocumentSymbol> {
        self.nodes.iter().map(NodeGetSymbol::dom_symbol).collect()
    }
}

impl<'a> NodeGetSymbol for AstNode<'a, SymbolData<'a>> {
    fn dom_symbol(&self) -> DocumentSymbol {
        let range = self.range();
        match self.data {
            SymbolData::Command {
                name,
                target,
                target_node,
            } => DocumentSymbol {
                name: format!("{name}: {target}"),
                detail: None,
                kind: SymbolKind::Variable,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
                range: range.lsp_range(),
                selection_range: target_node.range().lsp_range(),
                children: None,
            },
            SymbolData::Block => {
                let children: Vec<DocumentSymbol> = self
                    .children
                    .iter()
                    .map(NodeGetSymbol::dom_symbol)
                    .collect();
                DocumentSymbol {
                    name: "Block".to_owned(),
                    detail: None,
                    kind: SymbolKind::Namespace,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range: range.lsp_range(),
                    selection_range: range.lsp_range(),
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                }
            }
            SymbolData::IFBlock => {
                let children: Vec<DocumentSymbol> = self
                    .children
                    .iter()
                    .map(NodeGetSymbol::dom_symbol)
                    .collect();
                DocumentSymbol {
                    name: "If Condition".to_owned(),
                    detail: None,
                    kind: SymbolKind::Namespace,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range: range.lsp_range(),
                    selection_range: range.lsp_range(),
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                }
            }
            SymbolData::Function { name } => {
                let children: Vec<DocumentSymbol> = self
                    .children
                    .iter()
                    .map(NodeGetSymbol::dom_symbol)
                    .collect();
                DocumentSymbol {
                    name: name.to_owned(),
                    detail: None,
                    kind: SymbolKind::Function,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range: range.lsp_range(),
                    selection_range: range.lsp_range(),
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                }
            }
        }
    }
}

fn get_symbols(node: tree_sitter::Node, source: &str) -> Vec<DocumentSymbol> {
    let query = Query::new(&TREESITTER_CMAKE_LANGUAGE, QUERY_SOURCE).unwrap();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, node, source.as_bytes());

    let mut container: AstNodeContainer<'_, SymbolData<'_>> = AstNodeContainer::new();
    let names = query.capture_names();
    'out: while let Some(m) = matches.next() {
        let mut data_name = None;
        let mut identifier = None;
        let mut first_argument = None;
        let mut node = None;
        let mut arg_node = None;
        for e in m.captures {
            let name = names[e.index as usize];
            if name == "block" {
                let ast_node = AstNode::new(e.node, name).with_data(SymbolData::Block);
                container.insert_node(ast_node);
                continue 'out;
            }
            if name == "if_block" {
                let ast_node = AstNode::new(e.node, name).with_data(SymbolData::IFBlock);
                container.insert_node(ast_node);
                continue 'out;
            }
            if matches!(name, "function" | "command") {
                data_name = Some(name);
                node = Some(e.node);
                continue;
            }
            if name == "identifier" {
                let cmd_name = e.node.utf8_text(source.as_bytes()).unwrap();
                identifier = Some(cmd_name);
                continue;
            }
            if name == "first_arg" {
                first_argument = Some(e.node.utf8_text(source.as_bytes()).unwrap());
                arg_node = Some(e.node);
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
        let (Some(first_arg), Some(target_node)) = (first_argument, arg_node) else {
            continue;
        };

        let identifier_lower = identifier.to_lowercase();
        if !COMMAND_KEYWORDS.contains(&identifier_lower.as_str()) {
            continue;
        }
        let ast_node = AstNode::new(node, name).with_data(SymbolData::Command {
            name: identifier,
            target: first_arg,
            target_node,
        });
        container.insert_node(ast_node);
    }
    container.sort_to_finish();
    container.dom_symbols()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    #[allow(deprecated)]
    #[test]
    fn test_ast_1() {
        let context = include_str!("../assets_for_test/ast_test/bast_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert_eq!(
            get_symbols(thetree.root_node(), context),
            vec![
                DocumentSymbol {
                    name: "set: A".to_owned(),
                    detail: None,
                    kind: SymbolKind::Variable,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0
                        },
                        end: Position {
                            line: 2,
                            character: 1
                        }
                    },
                    selection_range: Range {
                        start: Position {
                            line: 1,
                            character: 1
                        },
                        end: Position {
                            line: 1,
                            character: 2
                        }
                    },
                    children: None
                },
                DocumentSymbol {
                    name: "set: B".to_owned(),
                    detail: None,
                    kind: SymbolKind::Variable,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: 4,
                            character: 0
                        },
                        end: Position {
                            line: 4,
                            character: 9
                        }
                    },
                    selection_range: Range {
                        start: Position {
                            line: 4,
                            character: 4
                        },
                        end: Position {
                            line: 4,
                            character: 5
                        }
                    },
                    children: None
                },
                DocumentSymbol {
                    name: "abc".to_owned(),
                    detail: None,
                    kind: SymbolKind::Function,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: 6,
                            character: 0
                        },
                        end: Position {
                            line: 7,
                            character: 13
                        }
                    },
                    selection_range: Range {
                        start: Position {
                            line: 6,
                            character: 0
                        },
                        end: Position {
                            line: 7,
                            character: 13
                        }
                    },
                    children: None
                },
                DocumentSymbol {
                    name: "If Condition".to_owned(),
                    detail: None,
                    kind: SymbolKind::Namespace,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: 9,
                            character: 0
                        },
                        end: Position {
                            line: 11,
                            character: 7
                        }
                    },
                    selection_range: Range {
                        start: Position {
                            line: 9,
                            character: 0
                        },
                        end: Position {
                            line: 11,
                            character: 7
                        }
                    },
                    children: Some(vec![DocumentSymbol {
                        name: "Block".to_owned(),
                        detail: None,
                        kind: SymbolKind::Namespace,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: 9,
                                character: 8
                            },
                            end: Position {
                                line: 11,
                                character: 0
                            }
                        },
                        selection_range: Range {
                            start: Position {
                                line: 9,
                                character: 8
                            },
                            end: Position {
                                line: 11,
                                character: 0
                            }
                        },
                        children: None
                    }])
                }
            ]
        );
    }

    #[test]
    fn test_ast_2() {
        let context = include_str!("../assets_for_test/ast_test/nheko_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert!(!get_symbols(thetree.root_node(), context).is_empty());
    }

    #[test]
    fn test_ast_3() {
        let context = r"
# Just comment here
";
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None).unwrap();

        assert_eq!(get_symbols(thetree.root_node(), context), vec![]);
    }
}
