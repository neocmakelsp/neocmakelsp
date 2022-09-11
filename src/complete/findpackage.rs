use crate::utils;
use anyhow::Result;
use lsp_types::{CompletionItem, CompletionItemKind};
use once_cell::sync::Lazy;
pub static CMAKE_SOURCE: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    match &*utils::CMAKE_PACKAGES {
        Ok(messages) => Ok(messages
            .clone()
            .into_iter()
            .map(|package| CompletionItem {
                label: package.name.clone(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some(
                    format!("name:{}\nFiletype:{}", package.name, package.filetype).to_string(),
                ),
                ..Default::default()
            })
            .collect()),
        Err(_) => return Err(anyhow::anyhow!("Unreaded")),
    }
});
