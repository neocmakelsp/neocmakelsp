use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    pub format: Option<FormatConfig>,
    pub scan_cmake_in_package: Option<bool>,
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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            format: Some(FormatConfig::default()),
            scan_cmake_in_package: Some(true),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct FormatConfig {
    pub enable: Option<bool>,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig { enable: Some(true) }
    }
}
