use crate::treehelper::{get_positon_string, point_to_position};
use lsp_types::{Position, Range};
use tree_sitter::Node;
pub fn godef(location: Position, root: Node, source: &str) -> Option<Range> {
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
fn godefsub(root: Node, source: &str, tofind: &str) -> Option<Range> {
    let newsource: Vec<&str> = source.lines().collect();
    let mut course = root.walk();
    for child in root.children(&mut course) {
        // if is inside same line
        if child.child_count() != 0 {
            let range = godefsub(child, source, tofind);
            if range.is_some() {
                return range;
            }
        } else {
            let h = child.start_position().row;
            let x = child.start_position().column;
            let y = child.end_position().column;

            let message = &newsource[h][x..y];
            if message == tofind {
                return Some(Range {
                    start: point_to_position(child.start_position()),
                    end: point_to_position(child.end_position()),
                });
            }
        }
    }
    None
}
