use crate::utils;
use anyhow::Result;
use lsp_types::{CompletionItem, CompletionItemKind};
use once_cell::sync::Lazy;
pub static CMAKE_SOURCE: Lazy<Result<Vec<CompletionItem>>> =
    Lazy::new(|| match &*utils::CMAKE_PACKAGES {
        Ok(messages) => Ok(messages
            .iter()
            .map(|package| CompletionItem {
                label: package.name.clone(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some(format!(
                    "name:{}\nFiletype:{}",
                    package.name, package.filetype
                )),
                ..Default::default()
            })
            .collect()),
        Err(_) => Err(anyhow::anyhow!("Unreaded")),
    });
