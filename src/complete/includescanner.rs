use super::getsubcoplete;
use lsp_types::CompletionItem;
use std::fs;
use std::path::PathBuf;
pub fn scanner_include_coplete(path: &PathBuf) -> Option<Vec<CompletionItem>> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(tree_sitter_cmake::language()).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            getsubcoplete(tree.root_node(), content.as_str(), path)
        }
        Err(_) => None,
    }
}
