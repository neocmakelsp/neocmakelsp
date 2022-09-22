use crate::utils;
use lsp_types::{CompletionItem, CompletionItemKind};
use once_cell::sync::Lazy;
pub static CMAKE_SOURCE: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    utils::CMAKE_PACKAGES
        .iter()
        .map(|package| CompletionItem {
            label: package.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(match &package.version {
                None => format!("name:{}\nFiletype:{}\n", package.name, package.filetype),
                Some(version) => format!(
                    "name:{}\nFiletype:{}\nversion:{}",
                    package.name, package.filetype, version
                ),
            }),
            ..Default::default()
        })
        .collect()
});
