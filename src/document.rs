use tower_lsp::lsp_types::Uri;
use tree_sitter::Parser;

#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO: Remove
pub(crate) struct Document {
    pub(crate) text: String,
    pub(crate) tree: tree_sitter::Tree,
    pub(crate) uri: Uri,
}

impl Document {
    pub(crate) fn new(text: String, uri: Uri) -> Option<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cmake::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&text, None)?;
        Some(Self { text, tree, uri })
    }
}
