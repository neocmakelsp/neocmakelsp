use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::grammar::ErrorType;
use crate::utils::query::try_get_argument_list;
use crate::utils::treehelper::ToPosition;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionResponse, Diagnostic, DocumentChange, Edit,
    OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

pub fn lint_fix_action(
    context: &str,
    diagnose: &Diagnostic,
    error_type: ErrorType,
    uri: tower_lsp::lsp_types::Uri,
) -> Option<Vec<CodeActionResponse>> {
    let ErrorType::Length { max: longest, .. } = error_type else {
        return None;
    };

    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    let root = tree.root_node();
    get_fix_action(root, context, diagnose, longest as usize, &uri)
}

fn get_fix_action(
    input: tree_sitter::Node,
    source: &str,
    diagnose: &Diagnostic,
    longest: usize,
    uri: &tower_lsp::lsp_types::Uri,
) -> Option<Vec<CodeActionResponse>> {
    let argument_list = try_get_argument_list(source.as_bytes(), input, diagnose.range)?;

    let start_node = argument_list.main_node;
    let start = start_node.start_position().to_position();
    let end = start_node.end_position().to_position();
    let range = Range { start, end };
    let mut start_row = start.character as usize;
    let start_space_len = start.character as usize;
    let start_space: String = vec![' '; start_space_len].iter().collect();
    let mut new_text = "".to_string();
    for arg in argument_list.arguments {
        let current_row = arg.start_position().row;
        // I mean I cannot fix this problem
        if current_row != arg.end_position().row {
            return None;
        }
        let len = arg.end_position().column - arg.start_position().column;
        let arg = arg.utf8_text(source.as_bytes()).unwrap();
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
    Some(vec![CodeActionResponse::CodeAction(CodeAction {
        title: "too long lint fix".to_string(),
        kind: Some(CodeActionKind::QuickFix),
        diagnostics: Some(vec![diagnose.clone()]),
        edit: Some(WorkspaceEdit {
            changes: None,
            change_annotations: None,
            document_changes: Some(vec![DocumentChange::TextDocumentEdit(TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    version: None,
                    text_document_identifier: tower_lsp::lsp_types::TextDocumentIdentifier {
                        uri: uri.clone(),
                    },
                },
                edits: vec![Edit::TextEdit(TextEdit { range, new_text })],
            })]),
        }),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
        tags: None,
    })])
}
