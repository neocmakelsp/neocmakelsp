use std::{collections::HashMap, path::Path};

use tower_lsp::lsp_types::{Location, Position, TextEdit, Uri, WorkspaceEdit};

use crate::jump;

pub async fn rename<P: AsRef<Path>>(
    edited: &str,
    location: Position,
    originuri: P,
    client: &tower_lsp::Client,
    source: &str,
) -> Option<WorkspaceEdit> {
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    let defs = jump::godef(location, source, originuri, client, false, true).await?;

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
