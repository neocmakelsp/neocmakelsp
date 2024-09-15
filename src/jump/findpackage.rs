//use lsp_types::CompletionItem;
use super::Location;
use crate::utils::CACHE_CMAKE_PACKAGES_WITHKEYS;
use lsp_types::Url;
use tower_lsp::lsp_types;

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
                uri: Url::from_file_path(apath).unwrap(),
            })
            .collect()
    })
}
