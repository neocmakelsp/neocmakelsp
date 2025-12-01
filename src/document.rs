use tower_lsp::lsp_types::Uri;
use tree_sitter::Parser;

#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub tree: tree_sitter::Tree,
    pub uri: Uri,
}

impl Document {
    pub fn new(text: String, uri: Uri) -> Option<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cmake::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&text, None)?;
        Some(Self { text, tree, uri })
    }
}
