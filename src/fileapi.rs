pub mod cache;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use cache::Cache;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::lsp_types::CompletionItem;

static CACHE_DATA: LazyLock<Mutex<Option<Cache>>> = LazyLock::new(|| Mutex::new(None));

pub fn update_cache_data<P: AsRef<Path>>(cache_file: P) -> Option<Cache> {
    use std::fs::File;
    let file = File::open(cache_file).ok()?;

    let cache: Cache = serde_json::from_reader(file).ok()?;

    set_cache_data(cache)
}

pub fn get_cache_data() -> Option<Cache> {
    let data = CACHE_DATA.lock().ok()?;
    data.clone()
}
pub fn set_cache_data(cache: Cache) -> Option<Cache> {
    let mut data = CACHE_DATA.lock().ok()?;
    let old_data = data.take();
    *data = Some(cache);
    old_data
}

#[allow(dead_code)]
pub fn clear_cache_data() -> Option<Cache> {
    let mut data = CACHE_DATA.lock().ok()?;
    data.take()
}

#[inline]
pub fn get_complete_data() -> Option<Vec<CompletionItem>> {
    Some(get_cache_data()?.gen_completions())
}

#[inline]
pub fn get_entries_data() -> Option<HashMap<String, String>> {
    let entries = get_cache_data()?.entries;
    let mut map = HashMap::new();

    for entry in entries {
        map.insert(entry.name.clone(), entry.value.clone());
    }
    Some(map)
}

pub static DEFAULT_QUERY: LazyLock<Option<QueryJson>> = LazyLock::new(QueryJson::from_command);

pub const REGISTERED_NAME: &str = "client-neocmake";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct ApiVersion {
    major: u32,
    minor: u32,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct ApiRequest {
    kind: String,
    version: Vec<ApiVersion>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct QueryJson {
    requests: Vec<ApiRequest>,
}

impl QueryJson {
    pub fn new(context: &str) -> Option<Self> {
        let origin_data: Value = serde_json::from_str(context).ok()?;

        serde_json::from_value(origin_data.get("fileApi")?.clone()).ok()
    }

    pub fn from_command() -> Option<Self> {
        use std::process::Command;

        let context_data = Command::new("cmake")
            .arg("-E")
            .arg("capabilities")
            .output()
            .ok()?
            .stdout;

        let context = String::from_utf8_lossy(&context_data);

        Self::new(&context)
    }

    pub fn write_to_build_dir(&self, build_dir: &Path) -> std::io::Result<()> {
        use std::fs;
        let registered_dir = build_dir
            .join(".cmake")
            .join("api")
            .join("v1")
            .join("query")
            .join(REGISTERED_NAME);
        fs::create_dir_all(&registered_dir)?;
        let file_path = registered_dir.join("query.json");
        let file = fs::File::create(file_path)?;
        serde_json::to_writer(file, self)?;

        Ok(())
    }
}

#[cfg(test)]
mod api_test {
    use super::{Cache, QueryJson};

    #[test]
    fn test_serde() {
        let origin_json = include_str!("../assets_for_test/fileapi/api.json");
        let json = QueryJson::new(origin_json).unwrap();

        let final_json = include_str!("../assets_for_test/fileapi/fileapifinal.json");

        let json_target: QueryJson = serde_json::from_str(final_json).unwrap();

        assert_eq!(json_target, json);

        let _cache: Cache = serde_json::from_str(include_str!(
            "../assets_for_test/fileapi/cache-v2-c1f0b50299da00258c61.json"
        ))
        .unwrap();
    }
}
