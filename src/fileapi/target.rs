use crate::fileapi::ApiVersion;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path, sync::LazyLock};

pub static TARGET_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(target-(?P<name>[0-9a-zA-Z\-]+)-.+.json)").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub build_type: BuildType,
    pub info: TargetInfo,
    pub name: String,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.build_type)
    }
}
impl Target {
    pub fn hover(&self) -> String {
        let mut hover_info = self.name.to_owned();
        hover_info.push('\n');
        hover_info.push_str(&self.info.hover());
        hover_info
    }
    pub fn read<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();
        let file_name = path.file_name()?.to_str()?;

        let caps = TARGET_REGEX.captures(file_name)?;
        let mut name = &caps["name"];
        let mut build_type = BuildType::None;
        if let Some(after_name) = name.strip_suffix("-Debug") {
            name = after_name;
            build_type = BuildType::Debug;
        } else if let Some(after_name) = name.strip_suffix("-Release") {
            name = after_name;
            build_type = BuildType::Release;
        } else if let Some(after_name) = name.strip_suffix("-RelWithDebInfo") {
            name = after_name;
            build_type = BuildType::RelWithDebInfo;
        } else if let Some(after_name) = name.strip_suffix("-MinSizeRel") {
            name = after_name;
            build_type = BuildType::RelWithDebInfo;
        }

        let data = std::fs::read_to_string(path).ok()?;
        let info = serde_json::from_str(&data).ok()?;
        Some(Self {
            build_type,
            info,
            name: name.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum BuildType {
    Debug,
    Release,
    #[default]
    None,
    RelWithDebInfo,
    MinSizeRel,
}

impl std::fmt::Display for BuildType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "Debug"),
            Self::Release => write!(f, "Release"),
            Self::None => write!(f, "None"),
            Self::RelWithDebInfo => write!(f, "RelWithDebInfo"),
            Self::MinSizeRel => write!(f, "MinSizeRel"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,

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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    artifacts: Vec<Artifact>,
    codemodel_version: ApiVersion,
    compile_groups: Option<Vec<CompileGroup>>,
    #[serde(rename = "type")]
    type_: String,
    sources: Vec<Source>,
    #[serde(flatten)]
    _others: HashMap<String, serde_json::Value>,
}

impl TargetInfo {
    pub fn hover(&self) -> String {
        let mut hover_info = "".to_owned();
        hover_info.push_str(&format!("type: {}", self.type_));
        hover_info.push('\n');
        hover_info.push_str("artifacts:\n");
        for Artifact { path, .. } in &self.artifacts {
            hover_info.push_str(&format!("  path: {path}\n"));
        }
        hover_info.push('\n');
        hover_info.push_str("source:\n");
        for Source { path, .. } in &self.sources {
            hover_info.push_str(&format!("  path: {path}\n"));
        }
        hover_info
    }
    pub fn target_type(&self) -> TargetType {
        if self.type_ == "EXECUTABLE" {
            return TargetType::Executable;
        }
        TargetType::Library
    }
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetType {
    Library,
    Executable,
}

#[cfg(test)]
mod test {
    use crate::fileapi::target::{TARGET_REGEX, TargetInfo, TargetType};

    #[test]
    fn target_serde() {
        let file = include_str!("../../assets_for_test/waycrate.json");
        let target: TargetInfo = serde_json::from_str(file).unwrap();
        assert_eq!(target.target_type(), TargetType::Executable);
        assert_eq!(target.artifacts[0].path, "waycratelock");
    }

    #[test]
    fn build_type_read() {
        let file_name = "target-waycratelock-Debug-7d1c13a099b19b474ca1.json";
        let caps = TARGET_REGEX.captures(file_name).unwrap();

        assert_eq!(&caps["name"], "waycratelock-Debug");
        let file_name = "target-waycratelock-7d1c13a099b19b474ca1.json";
        let caps = TARGET_REGEX.captures(file_name).unwrap();
        assert_eq!(&caps["name"], "waycratelock");
    }
}
