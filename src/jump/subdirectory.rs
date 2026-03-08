use std::path::Path;

use ls_types::Uri;
use tower_lsp::ls_types;

use super::Location;

pub(super) fn cmpsubdirectory<P: AsRef<Path>>(
    localpath: P,
    subpath: &str,
) -> Option<Vec<Location>> {
    let dir = localpath.as_ref().parent()?;
    let target = dir.join(subpath).join("CMakeLists.txt");
    if target.exists() {
        return Some(vec![Location {
            range: ls_types::Range {
                start: ls_types::Position {
                    line: 0,
                    character: 0,
                },
                end: ls_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            uri: Uri::from_file_path(target).unwrap(),
        }]);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_cmp_subdirectory() {
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
                range: ls_types::Range {
                    start: ls_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: ls_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                uri: Uri::from_file_path(subdir_file).unwrap(),
            }]
        );
    }
}
