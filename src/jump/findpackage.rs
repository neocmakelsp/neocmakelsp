use lsp_types::Uri;
use tower_lsp::lsp_types;

use super::Location;
use crate::utils::CACHE_CMAKE_PACKAGES_WITHKEYS;

pub(super) fn cmpfindpackage(input: &str) -> Option<Vec<Location>> {
    CACHE_CMAKE_PACKAGES_WITHKEYS.get(input).map(|context| {
        context
            .tojump
            .iter()
            .map(|apath| Location {
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
                uri: Uri::from_file_path(apath).unwrap(),
            })
            .collect()
    })
}

#[test]
fn test_find_package() {
    use std::path::Path;
    let location_fake = cmpfindpackage("bash-completion-fake").unwrap();
    #[cfg(unix)]
    let jump_path = Path::new("/usr/share/bash-completion-fake/bash_completion-fake-config.cmake")
        .to_path_buf();
    #[cfg(not(unix))]
    let jump_path = Path::new(r"C:\Develop\bash-completion-fake\bash-completion-fake-config.cmake")
        .to_path_buf();

    assert_eq!(
        location_fake,
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
            uri: Uri::from_file_path(jump_path).unwrap()
        }]
    );
}

#[test]
fn test_find_package_failed() {
    let location_fake = cmpfindpackage("bash-completion");
    assert_eq!(location_fake, None);
}
