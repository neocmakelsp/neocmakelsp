use std::collections::HashMap;

use tower_lsp::lsp_types::{Location, Position, TextEdit, Uri, WorkspaceEdit};

use crate::{Document, DocumentCache, jump};

pub async fn rename(
    edited: &str,
    location: Position,
    document: &Document,
    client: &tower_lsp::Client,
    documents: &DocumentCache,
) -> Option<WorkspaceEdit> {
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    let defs = jump::godef(location, document, client, false, true, documents).await?;

    for Location { uri, range } in defs {
        let edits = changes.entry(uri).or_default();
        edits.push(TextEdit {
            range,
            new_text: edited.to_string(),
        });
    }
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
