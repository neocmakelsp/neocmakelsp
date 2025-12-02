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
    pub(super) name: String,
    properties: Vec<CacheEntryProperties>,
    r#type: String,
    pub(super) value: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_completion_test() {
        let cache = CacheEntry {
            name: "test".to_string(),
            properties: vec![],
            r#type: "Path".to_string(),
            value: "/usr/share".to_string(),
        };

        assert_eq!(
            cache.gen_completion(),
            CompletionItem {
                label: "test".to_string(),
                documentation: Some(Documentation::String(
                    "type: Path, value: /usr/share".to_string()
                )),
                detail: Some("Cached Values".to_string()),
                kind: Some(CompletionItemKind::VALUE),
                ..Default::default()
            }
        );
    }
}
