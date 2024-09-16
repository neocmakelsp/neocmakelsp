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
        CompletionItem {
            label: "CONFIG".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("set the config of the packages".to_string()),
            documentation: None,
            ..Default::default()
        },
    ]
});

#[cfg(unix)]
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
        .filter_map(|package| Some((package.name.strip_prefix(space)?, package)))
        .map(|(label, package)| CompletionItem {
            label: label.to_string(),
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

#[test]
fn test_prefix() {
    use crate::utils::{CMakePackage, CMakePackageFrom, PackageType};
    use crate::Url;
    use std::path::Path;
    let data = completion_items_with_prefix("bash");

    let data_package = CMakePackage {
        name: "bash-completion-fake".to_string(),
        packagetype: PackageType::Dir,
        #[cfg(unix)]
        location: Url::from_file_path("/usr/share/bash-completion").unwrap(),
        #[cfg(not(unix))]
        location: Url::from_file_path(r"C:\Develop\bash-completion-fake").unwrap(),
        version: None,
        #[cfg(unix)]
        tojump: vec![
            Path::new("/usr/share/bash-completion/bash_completion-fake-config.cmake").to_path_buf(),
        ],
        #[cfg(not(unix))]
        tojump: vec![Path::new(
            r"C:\Develop\bash-completion-fake\bash-completion-fake-config.cmake",
        )
        .to_path_buf()],
        from: CMakePackageFrom::System,
    };
    let result_item = CompletionItem {
        label: "-completion-fake".to_string(),
        kind: Some(CompletionItemKind::MODULE),
        detail: Some("Module".to_string()),
        documentation: Some(Documentation::String(match &data_package.version {
            None => format!(
                "name: {}\nFiletype: {}\nFrom: {}\n",
                data_package.name, data_package.packagetype, data_package.from
            ),
            Some(version) => format!(
                "name: {}\nFiletype: {}\nFrom: {}\nversion: {}",
                data_package.name, data_package.packagetype, data_package.from, version
            ),
        })),
        ..Default::default()
    };

    let mut result_data = vec![result_item];
    result_data.append(&mut FIND_PACKAGE_SPACE_KEYWORDS.clone());

    assert_eq!(data, result_data);
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
