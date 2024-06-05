use std::io::Read;

use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct CMakeLintConfig {
    pub command_upcase: String,
}

pub struct LintSuggestion {
    pub command_upcase: String,
    pub hint: String,
}

impl LintSuggestion {
    pub fn lint_match(&self, upcase: bool) -> bool {
        matches!(
            (self.command_upcase.as_str(), upcase),
            ("upcase", true) | ("lowcase", false) | ("ignore", _)
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
            "lowcase" => Self {
                command_upcase,
                hint: "suggested to use lowcase".to_owned(),
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

pub static CMAKE_LINT: Lazy<LintSuggestion> = Lazy::new(|| {
    let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .open(".neocmakelint.toml")
    else {
        return LintSuggestion::default();
    };
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return LintSuggestion::default();
    }
    let Ok(CMakeLintConfig { command_upcase }) = toml::from_str(&buf) else {
        return LintSuggestion::default();
    };
    command_upcase.into()
});
