use crate::utils::CACHE_CMAKE_PACKAGES;
use std::sync::LazyLock;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation};

static FIND_PACKAGE_SPACE_KEYWORDS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    vec![
        CompletionItem {
            label: "COMPONENTS".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("use components to complete".to_string()),
            documentation: None,
            ..Default::default()
        },
        CompletionItem {
            label: "REQUIRED".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("show that this package is required".to_string()),
            documentation: None,
            ..Default::default()
        },
    ]
});

static PKGCONFIG_KEYWORDS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    vec![
        CompletionItem {
            label: "IMPORTED_TARGET".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("use imported_target way to use pkgconfig".to_string()),
            documentation: None,
            ..Default::default()
        },
        CompletionItem {
            label: "REQUIRED".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("show that this package is required".to_string()),
            documentation: None,
            ..Default::default()
        },
    ]
});
pub static CMAKE_SOURCE: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    CACHE_CMAKE_PACKAGES
        .iter()
        .map(|package| CompletionItem {
            label: package.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(match &package.version {
                None => format!(
                    "name: {}\nFiletype: {}\nFrom: {}\n",
                    package.name, package.packagetype, package.from
                ),
                Some(version) => format!(
                    "name: {}\nFiletype: {}\nFrom: {}\nversion: {}",
                    package.name, package.packagetype, package.from, version
                ),
            })),
            ..Default::default()
        })
        .collect()
});

pub(super) fn completion_items_with_prefix(space: &str) -> Vec<CompletionItem> {
    let mut data: Vec<CompletionItem> = CACHE_CMAKE_PACKAGES
        .iter()
        .filter(|package| package.name.starts_with(space))
        .map(|package| CompletionItem {
            label: package.name.strip_prefix(space).unwrap().to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(match &package.version {
                None => format!(
                    "name: {}\nFiletype: {}\nFrom: {}\n",
                    package.name, package.packagetype, package.from
                ),
                Some(version) => format!(
                    "name: {}\nFiletype: {}\nFrom: {}\nversion: {}",
                    package.name, package.packagetype, package.from, version
                ),
            })),
            ..Default::default()
        })
        .collect();
    data.append(&mut FIND_PACKAGE_SPACE_KEYWORDS.clone());
    data
}

#[cfg(unix)]
pub static PKGCONFIG_SOURCE: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    let mut data: Vec<CompletionItem> = crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES
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
        .collect();

    data.append(&mut PKGCONFIG_KEYWORDS.clone());
    data
});
