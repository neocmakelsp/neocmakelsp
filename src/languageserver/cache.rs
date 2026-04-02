use tower_lsp::lsp_types::Uri;

use crate::Backend;

impl Backend {
    /// Get the cloned value for an [`Uri`] key if it exists.
    #[inline]
    pub fn get_cached_buffer(&self, uri: &Uri) -> Option<String> {
        self.documents.get(uri).map(|v| v.value().clone())
    }

    /// Update a cache entry.
    #[inline]
    pub fn update_cache(&self, uri: Uri, text: impl Into<String>) {
        self.documents.insert(uri, text.into());
    }
}
