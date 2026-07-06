use std::borrow::Cow;
use std::sync::LazyLock;

use tower_lsp::Client;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenTypes, SemanticTokens};

use crate::consts::TREESITTER_CMAKE_LANGUAGE;

use tree_sitter::{Node, Point, Query, QueryCursor, Range, StreamingIterator};

const NONE_TYPE: &str = "none";

const NONE_TYPE_COW: Cow<'static, str> = Cow::Borrowed(NONE_TYPE);

const NONE_SEMANTIC_TOKEN: SemanticTokenTypes = SemanticTokenTypes::Custom(NONE_TYPE_COW);

pub const LEGEND_TYPE: &[SemanticTokenTypes] = &[
    SemanticTokenTypes::Function,   // index 0
    SemanticTokenTypes::Method,     // index 1
    SemanticTokenTypes::Variable,   // index 2
    SemanticTokenTypes::String,     // index 3
    SemanticTokenTypes::Comment,    // index 4
    SemanticTokenTypes::Number,     // index 5
    SemanticTokenTypes::Keyword,    // index 6
    SemanticTokenTypes::Operator,   // index 7
    SemanticTokenTypes::Modifier,   // index 8
    SemanticTokenTypes::Parameter,  // index 9
    SemanticTokenTypes::EnumMember, // index 10
    NONE_SEMANTIC_TOKEN,            // index 11
];

fn get_token_position(tokentype: SemanticTokenTypes) -> u32 {
    LEGEND_TYPE
        .iter()
        .position(|data| *data == tokentype)
        .unwrap() as u32
}

static NUMBERREGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^\d+(?:\.+\d*)?$").unwrap());

static KEYWORDREGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^([A-Z-_]+)$").unwrap());

#[derive(Debug, PartialEq, Eq)]
struct HighLightNode<'a> {
    node: Node<'a>,
    highlights: Vec<&'a str>,
    children: Vec<HighLightNode<'a>>,
}

impl<'a> HighLightNode<'a> {
    fn hl_token_index(&self, source: &str) -> u32 {
        get_token_position(self.hl_token(source))
    }
    fn hl_token(&self, source: &str) -> SemanticTokenTypes {
        if self.highlights.contains(&"function") {
            return SemanticTokenTypes::Function;
        }
        if self.highlights.contains(&"string") {
            return SemanticTokenTypes::String;
        }
        if self.highlights.contains(&"comment") && self.highlights.contains(&"spell") {
            return SemanticTokenTypes::Comment;
        }
        if self.highlights.contains(&"constant") {
            match self.node.utf8_text(source.as_bytes()) {
                Ok(txt) if NUMBERREGEX.is_match(txt) => {
                    return SemanticTokenTypes::Number;
                }
                Ok(txt)
                    if KEYWORDREGEX.is_match(txt)
                        // NOTE: exclude the "-D" and "CMAKE_CXX" like
                        && !txt.starts_with("-")
                        && !txt.starts_with("CMAKE_") =>
                {
                    return SemanticTokenTypes::EnumMember;
                }
                _ => {}
            }
        }
        if self
            .highlights
            .iter()
            .any(|hl| hl.starts_with("punctuation") || hl.ends_with("operator"))
        {
            return SemanticTokenTypes::Operator;
        }
        if self.highlights.contains(&"keyword.modifier") {
            return SemanticTokenTypes::Modifier;
        }
        if self.highlights.iter().any(|hl| hl.starts_with("keyword")) {
            return SemanticTokenTypes::Keyword;
        }
        if self.highlights.contains(&"variable.parameter") {
            return SemanticTokenTypes::Parameter;
        }
        if self.highlights.contains(&"variable") {
            return SemanticTokenTypes::Variable;
        }
        NONE_SEMANTIC_TOKEN
    }
}

trait RangeContain {
    fn contain(&self, other: &Self) -> bool;
}

impl RangeContain for Range {
    fn contain(&self, other: &Self) -> bool {
        self.start_byte <= other.start_byte && self.end_byte >= other.end_byte
    }
}

impl<'a> RangeContain for HighLightNode<'a> {
    fn contain(&self, other: &Self) -> bool {
        self.node.range().contain(&other.node.range())
    }
}

impl<'a> Ord for HighLightNode<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.range().start_byte >= other.range().end_byte {
            return std::cmp::Ordering::Greater;
        }
        if self.range().end_byte <= other.range().start_byte {
            return std::cmp::Ordering::Less;
        }
        std::cmp::Ordering::Equal
    }
}

impl<'a> HighLightNode<'a> {
    fn range(&self) -> Range {
        self.node.range()
    }
    fn insert_node(&mut self, node: Node<'a>, highlight: &'a str) {
        assert!(self.children.is_sorted());
        if let Some(hnode) = self
            .children
            .iter_mut()
            .find(|hnode| hnode.range() == node.range())
        {
            hnode.highlights.push(highlight);
            return;
        }
        if let Some(hnode) = self
            .children
            .iter_mut()
            .find(|hnode| hnode.range().contain(&node.range()))
        {
            return hnode.insert_node(node, highlight);
        }
        self.children.push(HighLightNode {
            node,
            highlights: vec![highlight],
            children: Vec::new(),
        });
        self.children.sort();
    }

    fn get_semantic_tokens(&self, cursor: &mut Point, source: &str) -> Vec<SemanticToken> {
        assert!(self.children.is_sorted());

        let otoken = self.hl_token_index(source);
        let mut tokens = vec![];
        let range = self.range();
        let start_byte = range.start_byte;
        let end_byte = range.end_byte;
        let end_point = range.end_point;
        let start_point = range.start_point;
        if start_point.row > cursor.row {
            cursor.column = 0;
        }

        let mut current_start_point = start_point;
        let mut current_byte = start_byte;
        for node in &self.children {
            let child_range = node.range();
            let child_start_point = child_range.start_point;
            let child_end_point = child_range.end_point;
            assert!(
                child_start_point.row > cursor.row
                    || (child_start_point.row == cursor.row
                        && child_start_point.column >= cursor.column)
            );

            // Insert the origin highlight
            if child_start_point.row != cursor.row || child_start_point.column >= cursor.column {
                if child_start_point.row > cursor.row {
                    cursor.column = 0;
                }
                let delta_start = (current_start_point.column - cursor.column) as u32;
                tokens.push(SemanticToken {
                    delta_line: (current_start_point.row - cursor.row) as u32,
                    delta_start,
                    length: (child_range.start_byte - current_byte) as u32,
                    token_type: otoken,
                    token_modifiers_bitset: 0,
                });
                *cursor = current_start_point;
            }

            tokens.extend(node.get_semantic_tokens(cursor, source));

            current_start_point = child_end_point;
            current_byte = child_range.end_byte;
        }

        if end_point.row > cursor.row
            || (end_point.row == cursor.row && end_point.column >= cursor.column)
        {
            let delta_start = (current_start_point.column - cursor.column) as u32;
            tokens.push(SemanticToken {
                delta_line: (start_point.row - cursor.row) as u32,
                delta_start,
                length: (end_byte - current_byte) as u32,
                token_type: otoken,
                token_modifiers_bitset: 0,
            });
        }

        *cursor = current_start_point;

        tokens
    }
}

#[derive(Debug, PartialEq, Eq)]
struct HighLightNodeContainer<'a> {
    nodes: Vec<HighLightNode<'a>>,
}

impl<'a> HighLightNodeContainer<'a> {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }
    fn insert_node(&mut self, node: Node<'a>, highlight: &'a str) {
        assert!(self.nodes.is_sorted());
        if let Some(hnode) = self
            .nodes
            .iter_mut()
            .find(|hnode| hnode.range() == node.range())
        {
            hnode.highlights.push(highlight);
            return;
        }
        if let Some(hnode) = self
            .nodes
            .iter_mut()
            .find(|hnode| hnode.range().contain(&node.range()))
        {
            hnode.insert_node(node, highlight);
            return;
        }
        self.nodes.push(HighLightNode {
            node,
            highlights: vec![highlight],
            children: Vec::new(),
        });
        self.nodes.sort();
    }

    fn get_semantic_tokens(&self, source: &str) -> Vec<SemanticToken> {
        assert!(self.nodes.is_sorted());
        let mut cursor = Point::new(0, 0);
        let mut tokens = vec![];
        for node in &self.nodes {
            tokens.extend(node.get_semantic_tokens(&mut cursor, source));
        }
        tokens
    }
}

impl<'a> PartialOrd for HighLightNode<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.range().start_byte >= other.range().end_byte {
            return Some(std::cmp::Ordering::Greater);
        }
        if self.range().end_byte <= other.range().start_byte {
            return Some(std::cmp::Ordering::Less);
        }
        if self.range() == other.range() {
            return Some(std::cmp::Ordering::Equal);
        }
        None
    }
}

pub async fn semantic_token(_client: &Client, context: &str) -> Option<SemanticTokens> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree?;
    Some(SemanticTokens {
        result_id: None,
        data: get_tokens(tree.root_node(), &context),
    })
}

fn get_tokens(node: tree_sitter::Node, source: &str) -> Vec<SemanticToken> {
    let query_source = tree_sitter_cmake::HIGHLIGHTS_QUERY;
    let query = Query::new(&TREESITTER_CMAKE_LANGUAGE, query_source).unwrap();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, node, source.as_bytes());

    let mut container = HighLightNodeContainer::new();
    let names = query.capture_names();
    while let Some(m) = matches.next() {
        for e in m.captures {
            container.insert_node(e.node, names[e.index as usize]);
        }
    }
    container.get_semantic_tokens(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number() {
        assert!(NUMBERREGEX.is_match("1.1"));
        assert!(NUMBERREGEX.is_match("222"));
        assert!(!NUMBERREGEX.is_match("222abcd"));
    }
    #[test]
    fn test_keywordr() {
        assert!(!NUMBERREGEX.is_match("abcd"));
        assert!(!NUMBERREGEX.is_match("abcd_WORLD"));
        assert!(KEYWORDREGEX.is_match("HELLO_WORLD"));
        assert!(!KEYWORDREGEX.is_match("Qt6::WaylandClient"));
    }

    fn semantic_token_test(context: &str) -> Option<SemanticTokens> {
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None);
        let tree = thetree?;
        Some(SemanticTokens {
            result_id: None,
            data: get_tokens(tree.root_node(), context),
        })
    }

    #[test]
    fn test_hl() {
        let tokens = semantic_token_test(include_str!(
            "../assets_for_test/highlight/bracket_argument.cmake"
        ))
        .unwrap()
        .data;
        assert_eq!(
            tokens,
            vec![
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 3,
                    token_type: get_token_position(SemanticTokenTypes::Function),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 3,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Operator),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 1,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Variable),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 2,
                    length: 5,
                    token_type: get_token_position(SemanticTokenTypes::String),
                    token_modifiers_bitset: 0
                },
                // NOTE: it is for arguments, which should not have highlight
                // But our logic needs it
                SemanticToken {
                    delta_line: 0,
                    delta_start: 5,
                    length: 0,
                    token_type: get_token_position(NONE_SEMANTIC_TOKEN),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Operator),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 1,
                    length: 0,
                    token_type: get_token_position(NONE_SEMANTIC_TOKEN),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Operator),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 1,
                    length: 0,
                    token_type: get_token_position(NONE_SEMANTIC_TOKEN),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 4,
                    token_type: get_token_position(SemanticTokenTypes::Variable),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 4,
                    length: 0,
                    token_type: get_token_position(NONE_SEMANTIC_TOKEN),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Operator),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 1,
                    length: 0,
                    token_type: get_token_position(NONE_SEMANTIC_TOKEN),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 5,
                    token_type: get_token_position(SemanticTokenTypes::String),
                    token_modifiers_bitset: 0
                },
                SemanticToken {
                    delta_line: 0,
                    delta_start: 5,
                    length: 1,
                    token_type: get_token_position(SemanticTokenTypes::Operator),
                    token_modifiers_bitset: 0
                }
            ]
        );
    }
}
