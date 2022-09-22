//use lsp_types::CompletionItem;
use super::JumpLocation;
use crate::utils;
use lsp_types::{MessageType, Url};
use tower_lsp::Client;
pub(super) async fn cmpfindpackage(input: String, client: &Client) -> Option<Vec<JumpLocation>> {
    client
        .log_message(MessageType::LOG, "Go to Find Package")
        .await;
    match &*utils::CMAKE_PACKAGES_WITHKEY {
        Ok(keys) => keys.get(&input).map(|context| {
            context
                .tojump
                .iter()
                .map(|apath| JumpLocation {
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
                    uri: Url::parse(apath).unwrap(),
                })
                .collect()
        }),
        Err(_) => None,
    }
}
