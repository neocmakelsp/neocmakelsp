use std::path::Path;

use lsp_types::Uri;
use tower_lsp::lsp_types;

use super::Location;

pub(super) fn cmpsubdirectory<P: AsRef<Path>>(
    localpath: P,
    subpath: &str,
) -> Option<Vec<Location>> {
    let dir = localpath.as_ref().parent()?;
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
            uri: Uri::from_file_path(target).unwrap(),
        }]);
    }
    None
}

#[test]
fn tst_cmp_subdirectory() {
    use std::fs;
    use std::fs::File;

    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let top_cmake = dir.path().join("CMakeLists.txt");
    File::create_new(&top_cmake).unwrap();
    let subdir = dir.path().join("abcd_test");
    fs::create_dir_all(&subdir).unwrap();
    let subdir_file = subdir.join("CMakeLists.txt");
    File::create_new(&subdir_file).unwrap();

    let locations = cmpsubdirectory(&top_cmake, "abcd_test").unwrap();

    assert_eq!(
        locations,
        vec![Location {
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
            uri: Uri::from_file_path(subdir_file).unwrap(),
        }]
    );
}
