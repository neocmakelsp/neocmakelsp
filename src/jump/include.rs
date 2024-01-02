use super::Location;
use lsp_types::{MessageType, Url};
use std::path::PathBuf;
use tower_lsp::lsp_types;
fn ismodule(tojump: &str) -> bool {
    tojump.split('.').count() == 1
}

pub(super) async fn cmpinclude(
    localpath: String,
    subpath: &str,
    client: &tower_lsp::Client,
) -> Option<Vec<Location>> {
    let path = PathBuf::from(localpath);
    let target = if !ismodule(subpath) {
        let root_dir = path.parent().unwrap();
        root_dir.join(subpath)
    } else {
        #[cfg(unix)]
        let glob_pattern = format!("/usr/share/cmake*/Modules/{subpath}.cmake");
        #[cfg(not(unix))]
        let glob_pattern = {
            let Ok(prefix) = std::env::var("CMAKE_PREFIX_PATH") else {
                return None;
            };
            format!("{prefix}/cmake*/Modules/{subpath}.cmake")
        };
        let Some(path) = glob::glob(glob_pattern.as_str())
            .into_iter()
            .flatten()
            .flatten()
            .next()
        else {
            return None;
        };
        path
    };

    if target.exists() {
        let target = target.to_str().unwrap();
        client
            .log_message(MessageType::INFO, format!("Jump Path is {target}"))
            .await;
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
        None
    }
}
#[test]
fn ut_ismodule() {
    assert_eq!(ismodule("GNUInstall"), true);
    assert_eq!(ismodule("test.cmake"), false);
}
