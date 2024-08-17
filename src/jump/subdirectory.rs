use super::Location;
use lsp_types::{MessageType, Url};
use std::path::Path;
use tower_lsp::lsp_types;
use tower_lsp::Client;
pub(super) async fn cmpsubdirectory(
    localpath: &Path,
    subpath: &str,
    client: &Client,
) -> Option<Vec<Location>> {
    let dir = localpath.parent().unwrap();
    let target = dir.join(subpath).join("CMakeLists.txt");
    if target.exists() {
        Some(vec![Location {
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
            uri: Url::from_file_path(target).unwrap(),
        }])
    } else {
        client
            .log_message(MessageType::INFO, "path not exist")
            .await;
        None
    }
}
