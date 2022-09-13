use super::JumpLocation;
use lsp_types::Url;
use std::path::{Path, PathBuf};
pub(super) fn cmpinclude(localpath: String, subpath: &str) -> Option<Vec<JumpLocation>> {
    let path = PathBuf::from(localpath);
    let dir = path.parent().unwrap();
    let target = format!("{}/{}", dir.to_str().unwrap(), subpath);
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
