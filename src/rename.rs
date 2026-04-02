use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::{Location, Position, TextEdit, WorkspaceEdit};

use crate::{Backend, jump};

impl Backend {
    pub async fn rename_symbol<P: AsRef<Path>>(
        &self,
        edited: &str,
        location: Position,
        originuri: P,
        client: &tower_lsp::Client,
        source: &str,
    ) -> Option<WorkspaceEdit> {
        let definitions = jump::godef(
            location,
            source,
            originuri,
            client,
            false,
            true,
            &self.documents,
        )
        .await?;

        let changes =
            definitions
                .into_iter()
                .fold(HashMap::new(), |mut map, Location { uri, range }| {
                    let edit = TextEdit::new(range, edited.to_string());
                    map.entry(uri).or_insert_with(Vec::new).push(edit);
                    map
                });

        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}
