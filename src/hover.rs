use lsp_types::Position;
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::Node;

use crate::fileapi;
use crate::jump::JUMP_CACHE;
#[cfg(unix)]
use crate::utils::packagepkgconfig::PkgConfig;
#[cfg(unix)]
use crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES_WITHKEY;
use crate::utils::treehelper::{
    get_point_string, get_pos_type, PositionType, ToPoint, MESSAGE_STORAGE,
};
use crate::utils::{get_the_packagename, CMakePackage, PackageType, CACHE_CMAKE_PACKAGES_WITHKEYS};

#[inline]
#[cfg(unix)]
fn vcpkg_document_fmt(context: &PkgConfig) -> String {
    format!(
        "
PackageName: {}
PackagePath: {}
",
        context.libname, context.path,
    )
}

#[inline]
fn cmakepackage_document_fmt(context: &CMakePackage) -> String {
    let package_type = if context.packagetype == PackageType::Dir {
        "PackageDir"
    } else {
        "PackagePath"
    };
    format!(
        "
PackageName: {}
{}: {}
PackageVersion: {}
",
        context.name,
        package_type,
        context.location.path(),
        context.version.clone().unwrap_or("Undefined".to_string())
    )
}

/// get the doc for on hover
pub async fn get_hovered_doc(location: Position, root: Node<'_>, source: &str) -> Option<String> {
    let current_point = location.to_point();
    let message = get_point_string(current_point, root, &source.lines().collect())?;
    let inner_result = match get_pos_type(current_point, root, source) {
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            let package = get_the_packagename(message);
            let value = PKG_CONFIG_PACKAGES_WITHKEY.get(package);
            value.map(vcpkg_document_fmt)
        }
        PositionType::FindPackageSpace(spacename) => {
            let space_package_name = format!("{spacename}{message}");
            let mut value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(&space_package_name);
            if value.is_none() {
                value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(message);
            }
            value.map(cmakepackage_document_fmt)
        }
        PositionType::FindPackage | PositionType::TargetInclude | PositionType::TargetLink => {
            let package = get_the_packagename(message);
            let mut value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(package);
            if value.is_none() {
                value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(&package.to_lowercase());
            }
            value.map(cmakepackage_document_fmt)
        }
        _ => {
            let mut value = MESSAGE_STORAGE.get(message);
            if value.is_none() {
                value = MESSAGE_STORAGE.get(&message.to_lowercase());
            }
            value.map(|context| context.to_string())
        }
    };
    if inner_result.is_some() {
        return inner_result;
    }

    let jump_cache = JUMP_CACHE.lock().await;
    let cached_info = jump_cache.get(message)?.document_info.clone();
    // use cache_data to show info first
    if let Some(cache_data) = fileapi::get_entries_data() {
        if let Some(value) = cache_data.get(message) {
            return Some(format!("current cached value : {value}\n\n{cached_info}"));
        }
    }
    Some(cached_info)
}

#[tokio::test]
async fn tst_hover() {
    use crate::consts::TREESITTER_CMAKE_LANGUAGE;
    use crate::utils::{FindPackageFunsFake, FindPackageFunsTrait};

    let fake_data = FindPackageFunsFake.get_cmake_packages_withkeys();
    let fake_package = fake_data.get("bash-completion-fake").unwrap();
    let content = r#"
find_package(bash-completion-fake)
    "#;
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(content, None).unwrap();
    let document = get_hovered_doc(
        Position {
            line: 1,
            character: 15,
        },
        thetree.root_node(),
        &content,
    )
    .await
    .unwrap();
    assert_eq!(document, cmakepackage_document_fmt(&fake_package));
}
