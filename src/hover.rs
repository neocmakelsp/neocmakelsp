use crate::fileapi;
#[cfg(unix)]
use crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES_WITHKEY;
use crate::utils::treehelper::get_point_string;
use crate::utils::treehelper::get_pos_type;
use crate::utils::treehelper::position_to_point;
use crate::utils::treehelper::PositionType;
use crate::utils::treehelper::MESSAGE_STORAGE;
use crate::utils::CACHE_CMAKE_PACKAGES_WITHKEYS;
use lsp_types::Position;
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::Node;

use crate::jump::JUMP_CACHE;

const LIBRARIES_END: &str = "_LIBRARIES";
const INCLUDE_DIRS_END: &str = "_INCLUDE_DIRS";

fn get_the_packagename(package: &str) -> &str {
    if let Some(after) = package.strip_suffix(LIBRARIES_END) {
        return after;
    }
    if let Some(after) = package.strip_suffix(INCLUDE_DIRS_END) {
        return after;
    }
    package
}

#[test]
fn package_name_check_tst() {
    let package_names = vec!["abc", "def_LIBRARIES", "ghi_INCLUDE_DIRS"];
    let output: Vec<&str> = package_names
        .iter()
        .map(|name| get_the_packagename(name))
        .collect();
    assert_eq!(output, vec!["abc", "def", "ghi"]);
}

/// get the doc for on hover
pub async fn get_hovered_doc(location: Position, root: Node<'_>, source: &str) -> Option<String> {
    let current_point = position_to_point(location);
    let message = get_point_string(current_point, root, &source.lines().collect())?;
    let inner_result = match get_pos_type(current_point, root, source) {
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            let message = message.split('_').collect::<Vec<&str>>()[0];
            let value = PKG_CONFIG_PACKAGES_WITHKEY.get(message);
            value.map(|context| {
                format!(
                    "
Packagename: {}
Packagepath: {}
",
                    context.libname, context.path,
                )
            })
        }

        PositionType::FindPackage | PositionType::TargetInclude | PositionType::TargetLink => {
            let package = get_the_packagename(&message);
            let mut value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(package);
            if value.is_none() {
                value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(&package.to_lowercase());
            }
            value.map(|context| {
                format!(
                    "
Packagename: {}
Packagepath: {}
PackageVersion: {}
",
                    context.name,
                    context.tojump[0].display(),
                    context.version.clone().unwrap_or("Undefined".to_string())
                )
            })
        }
        _ => {
            let mut value = MESSAGE_STORAGE.get(&message);
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
    let cached_info = jump_cache.get(&message)?.1.clone();
    // use cache_data to show info first
    if let Some(cache_data) = fileapi::get_entries_data() {
        if let Some(value) = cache_data.get(&message) {
            return Some(format!("current cached value : {value}\n\n{cached_info}"));
        }
    }
    Some(cached_info)
}
