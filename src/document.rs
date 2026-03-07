use std::fs;
use std::path::{Path, PathBuf};

use dashmap::DashMap;
use tower_lsp::lsp_types::Uri;
use tree_sitter::{Parser, Tree};

pub type DocumentCache = DashMap<Uri, Document>;

#[derive(Debug, Clone)]
pub struct Document {
    source: String,
    tree: Tree,
    uri: Uri,
    path: PathBuf,
}

impl Document {
    pub fn with_source_and_uri(source: impl Into<String>, uri: Uri) -> Option<Self> {
        let source = source.into();
        let tree = Self::parse(&source)?;
        let path = uri.to_file_path().ok()?;
        Some(Self {
            source,
            tree,
            uri,
            path,
        })
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Option<Self> {
        let path = path.into();
        let source = fs::read_to_string(&path).ok()?;
        let tree = Self::parse(&source)?;
        let uri = Uri::from_file_path(&path).ok()?;
        Some(Self {
            source,
            tree,
            uri,
            path,
        })
    }

    fn parse(source: &str) -> Option<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cmake::LANGUAGE.into())
            .unwrap();
        parser.parse(&source, None)
    }
}

impl Document {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
