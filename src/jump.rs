/// privide go to definition
use crate::utils::treehelper::{get_position_string, point_to_position};
use lsp_types::{MessageType, Position, Range, Url};
use tree_sitter::Node;
mod findpackage;
mod include;
mod subdirectory;
use crate::utils::treehelper::{get_pos_type, PositionType};
use lsp_types::Location;
/// find the definition
pub async fn godef(
    location: Position,
    source: &str,
    originuri: String,
    client: &tower_lsp::Client,
) -> Option<Vec<Location>> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let positionstring = get_position_string(location, tree.root_node(), source);
    match positionstring {
        Some(tofind) => {
            if &tofind != "(" && &tofind != ")" {
                let jumptype =
                    get_pos_type(location, tree.root_node(), source, PositionType::Variable);
                match jumptype {
                    // TODO: maybe can hadle Include?
                    PositionType::Variable => {
                        godefsub(tree.root_node(), source, &tofind, originuri)
                    }
                    PositionType::FindPackage
                    | PositionType::TargetLink
                    | PositionType::TargetInclude => {
                        let tofind = tofind.split('_').collect::<Vec<&str>>()[0].to_string();
                        findpackage::cmpfindpackage(tofind, client).await
                    }
                    PositionType::NotFind => None,
                    #[cfg(unix)]
                    PositionType::FindPkgConfig => None,
                    PositionType::Include => include::cmpinclude(originuri, &tofind, client).await,
                    PositionType::SubDir => {
                        subdirectory::cmpsubdirectory(originuri, &tofind, client).await
                    }
                }
            } else {
                client.log_message(MessageType::INFO, "Empty").await;
                None
            }
        }
        None => None,
    }
}

/// sub get the def
fn godefsub(root: Node, source: &str, tofind: &str, originuri: String) -> Option<Vec<Location>> {
    let mut definitions: Vec<Location> = vec![];
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        //
        if child.child_count() != 0 {
            //let range = godefsub(child, source, tofind);
            if let Some(mut context) = godefsub(child, source, tofind, originuri.clone()) {
                definitions.append(&mut context);
            }
        } else if child.start_position().row == child.end_position().row {
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;
            let message = &newsource[h][x..y];
            if message == tofind {
                definitions.push(Location {
                    uri: Url::from_file_path(&originuri).unwrap(),
                    range: Range {
                        start: point_to_position(child.start_position()),
                        end: point_to_position(child.end_position()),
                    },
                })
            };
        }
    }
    if definitions.is_empty() {
        None
    } else {
        Some(definitions)
    }
}
