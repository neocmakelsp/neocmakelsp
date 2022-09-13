//use lsp_types::CompletionItem;
use lsp_types::Url;
use super::JumpLocation;
use crate::utils;
pub(super) fn cmpfindpackage(input: String) -> Option<Vec<JumpLocation>> {
    match &*utils::CMAKE_PACKAGES_WITHKEY {
        Ok(keys) => keys.get(&input).map(|context| match context.filetype {
            utils::FileType::File => vec![JumpLocation {
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
                uri:Url::parse(&format!("file://{}", context.filepath.clone())).unwrap(),
            }],
            utils::FileType::Dir => std::fs::read_dir(&context.filepath)
                .unwrap()
                .into_iter()
                .map(|apath| JumpLocation {
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
                    uri: Url::parse(&format!(
                        "file://{}",
                        apath.unwrap().path().to_str().unwrap().to_string()
                    )).unwrap(),
                })
                .collect(),
        }),
        Err(_) => None,
    }
}
