#[cfg(unix)]
use crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES_WITHKEY;
use crate::utils::treehelper::get_pos_type;
use crate::utils::treehelper::get_position_string;
use crate::utils::treehelper::PositionType;
use crate::utils::treehelper::MESSAGE_STORAGE;
use crate::utils::CACHE_CMAKE_PACKAGES_WITHKEYS;
use lsp_types::Position;
/// Some tools for treesitter  to lsp_types
use tower_lsp::lsp_types;
use tree_sitter::Node;

use crate::jump::JUMP_CACHE;
/// get the doc for on hover
pub async fn get_hovered_doc(location: Position, root: Node<'_>, source: &str) -> Option<String> {
    let message = get_position_string(location, root, source)?;
    let inner_result = match get_pos_type(location, root, source, PositionType::NotFind) {
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
            let message = message.split('_').collect::<Vec<&str>>()[0];
            let mut value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(message);
            if value.is_none() {
                value = CACHE_CMAKE_PACKAGES_WITHKEYS.get(&message.to_lowercase());
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
    Some(jump_cache.get(&message)?.1.clone())
}
