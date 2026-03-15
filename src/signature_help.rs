use tower_lsp::lsp_types::{Position, SignatureHelp, SignatureInformation};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::CMakeNodeKinds;
use crate::complete::builtin::BUILTIN_COMMAND_SIGNATURE_RES;
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::{query::NORMAL_COMMAND_QUERY, treehelper::ToPoint};

pub fn get_signature_help(
    location: Position,
    root: Node<'_>,
    source: &str,
) -> Option<SignatureHelp> {
    let query_cmd = Query::new(&TREESITTER_CMAKE_LANGUAGE, NORMAL_COMMAND_QUERY).unwrap();
    let mut query_cursor = QueryCursor::new();
    query_cursor.set_point_range(location.to_point()..location.to_point());
    let mut match_cmd = query_cursor.matches(&query_cmd, root, source.as_bytes());
    while let Some(m) = match_cmd.next() {
        let node = m.nodes_for_capture_index(0).next().unwrap();
        let Some(identifier) = node.child(0) else {
            continue;
        };
        if identifier.kind() != CMakeNodeKinds::IDENTIFIER {
            continue;
        }
        if let Some(command) =
            BUILTIN_COMMAND_SIGNATURE_RES.get(identifier.utf8_text(source.as_bytes()).unwrap())
        {
            return Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: command.signature.to_string(),
                    documentation: command.gen_document(),
                    parameters: command.gen_parameters(),
                    active_parameter: None,
                }],
                active_signature: None,
                active_parameter: None,
            });
        }
    }
    None
}
