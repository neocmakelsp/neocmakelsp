use super::JumpLocation;
use lsp_types::Url;
use std::path::{Path, PathBuf};
fn ismodule(tojump: &str) -> bool {
    tojump.split(".").collect::<Vec<&str>>().len() == 1
}
pub(super) fn cmpinclude(localpath: String, subpath: &str) -> Option<Vec<JumpLocation>> {
    let path = PathBuf::from(localpath);
    let target = if !ismodule(subpath) {
        let dir = path.parent().unwrap();
        format!("{}/{}", dir.to_str().unwrap(), subpath)
    } else {
        format!("/usr/share/cmake/Modules/{}.cmake", subpath)
    };
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
