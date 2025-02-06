use std::io::Read;
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct CMakeLintConfig {
    pub command_upcase: String,
    pub enable_external_cmake_lint: bool,
    pub max_words: usize,
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
            max_words: 80,
        }
    }
}

pub static CMAKE_LINT_CONFIG: LazyLock<CMakeLintConfig> = LazyLock::new(|| {
    let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .open(".neocmakelint.toml")
    else {
        return CMakeLintConfig::default();
    };
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return CMakeLintConfig::default();
    }

    if let Ok(config) = toml::from_str::<CMakeLintConfig>(&buf) {
        return config;
    };
    CMakeLintConfig::default()
});

pub static CMAKE_LINT: LazyLock<LintSuggestion> =
    LazyLock::new(|| CMAKE_LINT_CONFIG.command_upcase.clone().into());

#[cfg(test)]
mod tests {
    use crate::config::CMAKE_LINT_CONFIG;

    #[test]
    fn tst_lint_config() {
        assert_eq!((*CMAKE_LINT_CONFIG).command_upcase, "ignore");
        assert_eq!((*CMAKE_LINT_CONFIG).enable_external_cmake_lint, false);
        assert_eq!((*CMAKE_LINT_CONFIG).max_words, 80);
    }
}
