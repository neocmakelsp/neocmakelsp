use std::sync::LazyLock;

use regex::Regex;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionResponse, Diagnostic,
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit,
    TextEdit, WorkspaceEdit,
};

use crate::CMakeNodeKinds;
use crate::{consts::TREESITTER_CMAKE_LANGUAGE, utils::treehelper::ToPosition};

static LINT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"((?<length>\d+)/(?<max>\d+))"#).unwrap());

#[test]
fn lint_regex_text() {
    let information = "[C0301] Line too long (92/80)";
    let caps = LINT_REGEX.captures(&information).unwrap();
    assert_eq!(&caps["length"], "92");
    assert_eq!(&caps["max"], "80");
}

pub fn lint_fix_action(
    context: &str,
    line: u32,
    diagnose: &Diagnostic,
    uri: tower_lsp::lsp_types::Uri,
) -> Option<CodeActionResponse> {
    let caps = LINT_REGEX.captures(&diagnose.message)?;
    let longest = caps["max"].parse().unwrap();
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    let root = tree.root_node();
    sub_lint_fix_action(
        root,
        &context.lines().collect(),
        line as usize,
        diagnose,
        longest,
        &uri,
    )
}

fn sub_lint_fix_action(
    input: tree_sitter::Node,
    source: &Vec<&str>,
    line: usize,
    diagnose: &Diagnostic,
    longest: usize,
    uri: &tower_lsp::lsp_types::Uri,
) -> Option<CodeActionResponse> {
    let mut cursor = input.walk();
    for child in input.children(&mut cursor) {
        if child.end_position().row < line {
            continue;
        }
        if child.start_position().row > line {
            break;
        }
        match child.kind() {
            CMakeNodeKinds::ARGUMENT_LIST => {
                let start = child.start_position().to_position();
                let end = child.end_position().to_position();
                let range = Range { start, end };
                let mut arg_cursor = child.walk();
                let mut start_row = start.character as usize;
                let start_space_len = start.character as usize;
                let start_space: String = vec![' '; start_space_len].iter().collect();
                let mut new_text = "".to_string();
                for arg in child.children(&mut arg_cursor) {
                    let current_row = arg.start_position().row;
                    // I mean I cannot fix this problem
                    if current_row != arg.end_position().row {
                        return None;
                    }
                    let start_col = arg.start_position().column;
                    let end_col = arg.end_position().column;
                    let len = arg.end_position().column - arg.start_position().column;
                    let arg = &source[current_row][start_col..end_col];
                    if start_row + len + 1 > longest {
                        start_row = start_space_len + len + 1;
                        new_text.push('\n');
                        new_text.push_str(&start_space);
                    } else {
                        start_row += len + 1;
                        if !new_text.is_empty() {
                            new_text.push(' ');
                        }
                    }
                    new_text.push_str(arg);
                }
                return Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
                    title: "too long lint fix".to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnose.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: None,
                        change_annotations: None,
                        document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                            text_document: OptionalVersionedTextDocumentIdentifier {
                                uri: uri.clone(),
                                version: None,
                            },
                            edits: vec![OneOf::Left(TextEdit { range, new_text })],
                        }])),
                    }),
                    command: None,
                    is_preferred: None,
                    disabled: None,
                    data: None,
                })]);
            }
            _ => {
                let try_data = sub_lint_fix_action(child, source, line, diagnose, longest, uri);
                if try_data.is_some() {
                    return try_data;
                }
            }
        }
    }
    None
}
