use crate::fileapi::ApiVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    path: String,

    #[serde(flatten)]
    _others: HashMap<String, serde_json::Value>,
}

mod compile_group {

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommandFragment {
        fragment: String,
        #[serde(flatten)]
        _others: HashMap<String, serde_json::Value>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileGroup {
    compile_command_fragments: Vec<compile_group::CommandFragment>,
    language: String,
    #[serde(flatten)]
    _others: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    backtrace: i32,
    backtraces: Vec<i32>,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    artifacts: Vec<Artifact>,
    codemode_version: ApiVersion,
    compile_groups: Option<Vec<CompileGroup>>,
    #[serde(rename = "type")]
    type_: String,
    sources: Vec<Source>,
    #[serde(flatten)]
    _others: HashMap<String, serde_json::Value>,
}

impl Target {
    pub fn target_type(&self) -> TargetType {
        if self.type_ == "EXECUTABLE" {
            return TargetType::Executable;
        }
        return TargetType::Library;
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
enum TargetType {
    Library,
    Executable,
}
