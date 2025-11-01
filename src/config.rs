use std::path::PathBuf;
use std::sync::LazyLock;

use dirs::config_dir;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct CMakeLintConfig {
    pub command_upcase: String,
    pub enable_external_cmake_lint: bool,
    pub line_max_words: usize,
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

impl Default for CMakeLintConfig {
    fn default() -> Self {
        Self {
            command_upcase: "ignore".to_string(),
            enable_external_cmake_lint: false,
            line_max_words: 80,
        }
    }
}

#[derive(Default, Deserialize, PartialEq, Eq, Debug)]
pub struct CMakeFormatConfig {
    pub enable_external: bool,
    pub external_program: String,
    pub external_args: Vec<String>,
}

fn find_user_config() -> Option<PathBuf> {
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

fn find_format_user_config() -> Option<PathBuf> {
    let mut path = std::env::current_dir().unwrap(); // should never fail
    path = path.join(".neocmakeformat.toml");

    if path.exists() {
        tracing::info!("Using project-level format config file: {:?}", path);
        return Some(path);
    }

    if let Some(mut path) = config_dir() {
        path = path.join("neocmakelsp").join("format.toml");
        if path.exists() {
            tracing::info!("Using user-level format config file: {:?}", path);
            return Some(path);
        }
    }

    None
}

pub static CMAKE_LINT_CONFIG: LazyLock<CMakeLintConfig> = LazyLock::new(|| {
    if let Some(path) = find_user_config() {
        if let Ok(buf) = std::fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<CMakeLintConfig>(&buf) {
                return config;
            }
        }
    }

    CMakeLintConfig::default()
});

pub static CMAKE_FORMAT_CONFIG: LazyLock<CMakeFormatConfig> = LazyLock::new(|| {
    if let Some(path) = find_format_user_config() {
        if let Ok(buf) = std::fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<CMakeFormatConfig>(&buf) {
                return config;
            }
        }
    }

    CMakeFormatConfig::default()
});

pub static CMAKE_LINT: LazyLock<LintSuggestion> =
    LazyLock::new(|| CMAKE_LINT_CONFIG.command_upcase.clone().into());

#[cfg(test)]
mod tests {
    use crate::config::CMAKE_FORMAT_CONFIG;
    use crate::config::CMAKE_LINT_CONFIG;

    #[test]
    fn tst_format_config() {
        assert!(!CMAKE_FORMAT_CONFIG.enable_external);
        assert_eq!(CMAKE_FORMAT_CONFIG.external_program, "");
        assert_eq!(CMAKE_FORMAT_CONFIG.external_args.len(), 0);
    }

    #[test]
    fn tst_lint_config() {
        assert_eq!(CMAKE_LINT_CONFIG.command_upcase, "ignore");
        assert!(!CMAKE_LINT_CONFIG.enable_external_cmake_lint);
        assert_eq!(CMAKE_LINT_CONFIG.line_max_words, 80);
    }
}
