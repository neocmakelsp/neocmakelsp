use std::path::PathBuf;
use std::sync::LazyLock;

use dirs::config_dir;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    pub command_upcase: String,
    pub enable_external_cmake_lint: bool,
    pub line_max_words: usize,
    pub format: FormatConfig,
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
    pub args: Vec<String>,
}

fn find_config_file() -> Option<PathBuf> {
    let mut path = std::env::current_dir().unwrap(); // should never fail
    path = path.join(".neocmakelint.toml");

    if path.exists() {
        tracing::info!("Using project-level config file: {:?}", path);
        return Some(path);
    };

    if let Some(mut path) = config_dir() {
        path = path.join("neocmakelsp").join("lint.toml");
        if path.exists() {
            tracing::info!("Using user-level config file: {:?}", path);
            return Some(path);
        }
    };

    None
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    if let Some(path) = find_config_file() {
        if let Ok(buf) = std::fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<Config>(&buf) {
                return config;
            }
        }
    }

    Config::default()
});

pub static CMAKE_LINT: LazyLock<LintSuggestion> =
    LazyLock::new(|| CONFIG.command_upcase.clone().into());
