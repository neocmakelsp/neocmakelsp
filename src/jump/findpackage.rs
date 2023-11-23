//use lsp_types::CompletionItem;
use super::Location;
use crate::utils;
use lsp_types::{MessageType, Url};
use tower_lsp::Client;
pub(super) async fn cmpfindpackage(input: String, client: &Client) -> Option<Vec<Location>> {
    client
        .log_message(MessageType::LOG, "Go to Find Package")
        .await;
    utils::CMAKE_PACKAGES_WITHKEY.get(&input).map(|context| {
        context
            .tojump
            .iter()
            .map(|apath| Location {
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                uri: Url::from_file_path(apath).unwrap(),
            })
            .collect()
    })
}
