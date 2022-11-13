use super::JumpLocation;
use lsp_types::{MessageType, Url};
use std::path::PathBuf;
use tower_lsp::Client;
pub(super) async fn cmpsubdirectory(
    localpath: String,
    subpath: &str,
    client: &Client,
) -> Option<Vec<JumpLocation>> {
    let path = PathBuf::from(localpath);
    let dir = path.parent().unwrap();
    let target = dir.join(subpath).join("CMakeLists.txt");
    if target.exists() {
        Some(vec![JumpLocation {
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
            uri: Url::parse(&format!("file://{}", target.to_str().unwrap())).unwrap(),
        }])
    } else {
        client
            .log_message(MessageType::INFO, "path not exist")
            .await;
        None
    }
}
