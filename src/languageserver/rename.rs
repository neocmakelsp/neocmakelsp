use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::{Location, Position, TextEdit, Uri, WorkspaceEdit};

use crate::jump;
use crate::languageserver::Backend;

impl Backend {
    pub(crate) async fn rename<P: AsRef<Path>>(
        &self,
        edited: &str,
        location: Position,
        originuri: P,
        source: &str,
    ) -> Option<WorkspaceEdit> {
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        let defs = jump::godef(
            location,
            source,
            originuri,
            &self.client,
            false,
            true,
            &self.documents,
        )
        .await?;

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
}
