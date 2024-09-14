use super::Location;
use lsp_types::Url;
use std::path::Path;
use tower_lsp::lsp_types;
pub(super) fn cmpsubdirectory(localpath: &Path, subpath: &str) -> Option<Vec<Location>> {
    let dir = localpath.parent()?;
    let target = dir.join(subpath).join("CMakeLists.txt");
    if target.exists() {
        return Some(vec![Location {
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
        }]);
    }
    None
}
