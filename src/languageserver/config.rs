use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct Config {
    pub format: Option<FormatConfig>,
    pub scan_cmake_in_package: Option<bool>,
    pub semantic_token: Option<bool>,
    pub lint: Option<LintConfig>,
    pub use_snippets: Option<bool>,
}

impl Config {
    pub fn is_format_enabled(&self) -> bool {
        self.format
            .as_ref()
            .map(|config| config.enable.unwrap_or(true))
            .unwrap_or(true)
    }
    pub fn is_scan_cmake_in_package(&self) -> bool {
        self.scan_cmake_in_package.unwrap_or(true)
    }

    pub fn enable_semantic_token(&self) -> bool {
        self.semantic_token.unwrap_or(false)
    }

    pub fn is_lint_enabled(&self) -> bool {
        self.lint
            .as_ref()
            .map(|config| config.enable.unwrap_or(true))
            .unwrap_or(true)
    }

    pub fn use_snippets(&self) -> bool {
        self.use_snippets.unwrap_or(true)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            format: Some(FormatConfig::default()),
            scan_cmake_in_package: Some(true),
            semantic_token: Some(false),
            lint: Some(LintConfig::default()),
            use_snippets: Some(true),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct FormatConfig {
    pub enable: Option<bool>,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig { enable: Some(true) }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct LintConfig {
    pub enable: Option<bool>,
}

impl Default for LintConfig {
    fn default() -> Self {
        LintConfig { enable: Some(true) }
    }
}
