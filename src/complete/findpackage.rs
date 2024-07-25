use crate::utils;
use lsp_types::{CompletionItem, CompletionItemKind, Documentation};
use std::sync::LazyLock;
use tower_lsp::lsp_types;
pub static CMAKE_SOURCE: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    utils::CMAKE_PACKAGES
        .iter()
        .map(|package| CompletionItem {
            label: package.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(match &package.version {
                None => format!("name:{}\nFiletype:{}\n", package.name, package.filetype),
                Some(version) => format!(
                    "name:{}\nFiletype:{}\nversion:{}",
                    package.name, package.filetype, version
                ),
            })),
            ..Default::default()
        })
        .collect()
});

#[cfg(unix)]
pub static PKGCONFIG_SOURCE: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    utils::packagepkgconfig::PKG_CONFIG_PACKAGES
        .iter()
        .map(|package| CompletionItem {
            label: package.libname.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(format!(
                "{}\n{}",
                package.libname, package.path
            ))),
            ..Default::default()
        })
        .collect()
});
