use crate::grammar::ErrorType;
use crate::utils::query::try_get_argument_list;
use crate::utils::treehelper::ToPosition;
use crate::{config::CommandCase, consts::TREESITTER_CMAKE_LANGUAGE};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionResponse, Diagnostic, DocumentChange, Edit,
    OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

pub fn lint_fix_action(
    context: &str,
    diagnosticses: &[&Diagnostic],
    uri: tower_lsp::lsp_types::Uri,
) -> Option<Vec<CodeActionResponse>> {
    let mut responses = vec![];

    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(context, None)?;
    let root = tree.root_node();
    for diagnostic in diagnosticses {
        let error_type: ErrorType =
            serde_json::from_value(diagnostic.data.as_ref().unwrap().clone()).unwrap();
        match error_type {
            ErrorType::Length { max: longest, .. }
                if let Some(response) =
                    fix_too_long(root, context, diagnostic, longest as usize, &uri) =>
            {
                responses.push(response);
            }
            ErrorType::UpLowerCase { command_case, name } => {
                responses.push(fix_uplowercase(diagnostic, command_case, &name, &uri));
            }
            _ => {
                continue;
            }
        }
    }
    if responses.is_empty() {
        None
    } else {
        Some(responses)
    }
}

fn fix_uplowercase(
    diagnostic: &Diagnostic,
    command_case: CommandCase,
    name: &str,
    uri: &tower_lsp::lsp_types::Uri,
) -> CodeActionResponse {
    let range = diagnostic.range;
    let new_text = command_case.operator(name);
    CodeActionResponse::CodeAction(CodeAction {
        title: "UpLowerCase fix".to_string(),
        kind: Some(CodeActionKind::QuickFix),
        diagnostics: Some(vec![diagnostic.clone()]),
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
    })
}
fn fix_too_long(
    input: tree_sitter::Node,
    source: &str,
    diagnostic: &Diagnostic,
    longest: usize,
    uri: &tower_lsp::lsp_types::Uri,
) -> Option<CodeActionResponse> {
    let argument_list = try_get_argument_list(source.as_bytes(), input, diagnostic.range)?;

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
    Some(CodeActionResponse::CodeAction(CodeAction {
        title: "too long lint fix".to_string(),
        kind: Some(CodeActionKind::QuickFix),
        diagnostics: Some(vec![diagnostic.clone()]),
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
    }))
}
