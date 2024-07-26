use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation};

use super::ApiVersion;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub entries: Vec<CacheEntry>,
    kind: String,
    version: ApiVersion,
}

impl Cache {
    pub fn gen_completions(&self) -> Vec<CompletionItem> {
        self.entries
            .iter()
            .map(|entry| entry.gen_completion())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryProperties {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    name: String,
    properties: Vec<CacheEntryProperties>,
    r#type: String,
    value: String,
}

impl CacheEntry {
    fn gen_completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.name.clone(),
            documentation: Some(Documentation::String(format!(
                "type: {}, value: {}",
                self.r#type, self.value
            ))),
            detail: Some("Cached Values".to_string()),
            kind: Some(CompletionItemKind::VALUE),
            ..Default::default()
        }
    }
}
