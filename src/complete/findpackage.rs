use anyhow::Result;
use lsp_types::{CompletionItem, CompletionItemKind};
use once_cell::sync::Lazy;
pub static CMAKE_SOURCE: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    let paths = std::fs::read_dir("/usr/lib/cmake/")?;
    Ok(paths
        .into_iter()
        .map(|apath| {
            let message = apath.unwrap().path().to_str().unwrap().to_string();
            CompletionItem {
                label: message.clone(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some(message.clone()),
                ..Default::default()
            }
        })
        .collect())
});
