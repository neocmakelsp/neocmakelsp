use super::JumpLocation;
use lsp_types::{MessageType, Url};
use std::path::{Path, PathBuf};
fn ismodule(tojump: &str) -> bool {
    tojump.split('.').count() == 1
}

pub(super) async fn cmpinclude(
    localpath: String,
    subpath: &str,
    client: &tower_lsp::Client,
) -> Option<Vec<JumpLocation>> {
    let path = PathBuf::from(localpath);
    let target = if !ismodule(subpath) {
        let dir = path.parent().unwrap();
        format!("{}/{}", dir.to_str().unwrap(), subpath)
    } else {
        format!("/usr/share/cmake/Modules/{}.cmake", subpath)
    };
    client
        .log_message(MessageType::INFO, format!("Jump Path is {}", target))
        .await;
    if Path::new(&target).exists() {
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
            uri: Url::parse(&format!("file://{}", target)).unwrap(),
        }])
    } else {
        None
    }
}
#[test]
fn ut_ismodule() {
    assert_eq!(ismodule("GNUInstall"), true);
    assert_eq!(ismodule("test.cmake"), false);
}
