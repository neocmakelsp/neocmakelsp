use serde::{Deserialize, Serialize};

use serde_json::Value;

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
}

#[cfg(test)]
mod api_test {
    use super::QueryJson;

    #[test]
    fn test_serde() {
        let origin_json = include_str!("../assert/fileapi/api.json");
        let json = QueryJson::new(origin_json).unwrap();

        let final_json = include_str!("../assert/fileapi/fileapifinal.json");

        let json_target: QueryJson = serde_json::from_str(&final_json).unwrap();

        assert_eq!(json_target, json);
    }
}
