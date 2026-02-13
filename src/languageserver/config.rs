use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct Config {
    #[serde(default)]
    pub format: FormatConfig,
    #[serde(default = "scan_cmake_in_package_default")]
    pub scan_cmake_in_package: bool,
    #[serde(default)]
    pub semantic_token: bool,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub use_snippets: bool,
}

const fn scan_cmake_in_package_default() -> bool {
    true
}

impl Config {
    pub fn is_format_enabled(&self) -> bool {
        self.format.enable
    }
    pub fn is_scan_cmake_in_package(&self) -> bool {
        self.scan_cmake_in_package
    }

    pub fn enable_semantic_token(&self) -> bool {
        self.semantic_token
    }

    pub fn is_lint_enabled(&self) -> bool {
        self.lint.enable
    }

    pub fn use_snippets(&self) -> bool {
        self.use_snippets
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            format: FormatConfig::default(),
            scan_cmake_in_package: true,
            semantic_token: false,
            lint: LintConfig::default(),
            use_snippets: true,
        }
    }
}

const fn default_enable() -> bool {
    true
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct FormatConfig {
    #[serde(default = "default_enable")]
    pub enable: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig { enable: true }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct LintConfig {
    #[serde(default = "default_enable")]
    pub enable: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        LintConfig { enable: true }
    }
}

#[cfg(test)]
mod test {
    use super::Config;
    #[test]
    fn config_test() {
        let data = r#"{}"#;
        let config: Config = serde_json::from_str(data).unwrap();
        assert!(config.scan_cmake_in_package);
        assert!(!config.use_snippets);
        assert!(config.is_lint_enabled());
        assert!(config.is_format_enabled());
    }
}
