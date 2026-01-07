use std::path::PathBuf;
use std::sync::LazyLock;

use dirs::config_dir;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    #[serde(default = "default_command_upcase")]
    pub command_upcase: String,
    #[serde(default = "default_external_cmake_lint")]
    pub enable_external_cmake_lint: bool,
    #[serde(default = "default_max_words")]
    pub line_max_words: usize,
    #[serde(default = "default_format")]
    pub format: FormatConfig,
}
fn default_command_upcase() -> String {
    "ignore".to_owned()
}
const fn default_external_cmake_lint() -> bool {
    false
}
const fn default_max_words() -> usize {
    80
}

fn default_format() -> FormatConfig {
    FormatConfig::default()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            command_upcase: "ignore".to_string(),
            enable_external_cmake_lint: false,
            line_max_words: 80,
            format: FormatConfig::default(),
        }
    }
}

pub struct LintSuggestion {
    pub command_upcase: String,
    pub hint: String,
}

impl LintSuggestion {
    pub fn lint_match(&self, upcase: bool) -> bool {
        matches!(
            (self.command_upcase.as_str(), upcase),
            ("upcase", true) | ("lowercase", false) | ("ignore", _)
        )
    }
}

impl From<String> for LintSuggestion {
    fn from(command_upcase: String) -> Self {
        match command_upcase.as_str() {
            "upcase" => Self {
                command_upcase,
                hint: "suggested to use upcase".to_owned(),
            },
            "lowercase" => Self {
                command_upcase,
                hint: "suggested to use lowercase".to_owned(),
            },
            _ => Self::default(),
        }
    }
}

impl Default for LintSuggestion {
    fn default() -> Self {
        Self {
            command_upcase: "ignore".to_string(),
            hint: "".to_owned(),
        }
    }
}

#[derive(Default, Deserialize, PartialEq, Eq, Debug)]
pub struct FormatConfig {
    pub program: Option<String>,
    args: Option<Vec<String>>,
}

impl FormatConfig {
    pub fn get_args(&self) -> Vec<&str> {
        let Some(args) = &self.args else {
            return vec![];
        };

        args.iter().map(|arg| arg.as_str()).collect()
    }
}

fn find_config_file() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;

    for file in [".neocmake.toml", ".neocmakelint.toml"] {
        let path = current_dir.join(file);
        if path.exists() {
            tracing::info!("Using project-level config file: {:?}", path);
            return Some(path);
        }
    }

    if let Some(config_dir) = config_dir() {
        for file in ["config.toml", "lint.toml"] {
            let path = config_dir.join("neocmakelsp").join(file);
            if path.exists() {
                tracing::info!("Using user-level config file: {:?}", path);
                return Some(path);
            }
        }
    }

    None
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    if let Some(path) = find_config_file()
        && let Ok(buf) = std::fs::read_to_string(path)
        && let Ok(config) = toml::from_str::<Config>(&buf)
    {
        return config;
    }

    Config::default()
});

pub static CMAKE_LINT: LazyLock<LintSuggestion> =
    LazyLock::new(|| CONFIG.command_upcase.clone().into());

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn empty_config() {
        let config_file = "";
        let config: Config = toml::from_str(config_file).unwrap();
        assert_eq!(config, Config::default());
    }
    #[test]
    fn empty_args() {
        let config_file = r#"
[format]
program = "cmake-format"
"#;
        let config: Config = toml::from_str(config_file).unwrap();
        let args = config.format.get_args();
        assert_eq!(config.format.program, Some("cmake-format".to_owned()));
        assert_eq!(args.len(), 0);
    }
    #[test]
    fn has_args() {
        let config_file = r#"
[format]
program = "cmake-format"
args = ["--hello"]
"#;
        let config: Config = toml::from_str(config_file).unwrap();
        let args = config.format.get_args();
        assert_eq!(config.format.program, Some("cmake-format".to_owned()));
        assert_eq!(args, vec!["--hello"]);
    }
}
