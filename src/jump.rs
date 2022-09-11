/// privide go to definition
use crate::treehelper::{get_positon_string, point_to_position};
use lsp_types::{Position, Range};
use tree_sitter::Node;

/// find the definition
pub fn godef(location: Position, root: Node, source: &str) -> Option<Vec<Range>> {
    match get_positon_string(location, root, source) {
        Some(tofind) => {
            if &tofind != "(" && &tofind != ")" {
                godefsub(root, source, &tofind)
            } else {
                None
            }
        }
        None => None,
    }
}

/// sub get the def
fn godefsub(root: Node, source: &str, tofind: &str) -> Option<Vec<Range>> {
    let mut definitions : Vec<Range> = vec![];
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if child.child_count() != 0 {
            //let range = godefsub(child, source, tofind);
            if let Some(mut context) = godefsub(child, source, tofind) {
                definitions.append(&mut context);
            }
        } else if child.start_position().row == child.end_position().row{
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;
            let message = &newsource[h][x..y];
            if message == tofind {
                definitions.push(Range {
                    start: point_to_position(child.start_position()),
                    end: point_to_position(child.end_position()),
                });
            }
        }
    }
    if definitions.is_empty() {
        None
    } else {
        Some(definitions)
    }
}

// TODO jump to file
