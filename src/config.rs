use std::path::PathBuf;
use std::sync::LazyLock;

use etcetera::{BaseStrategy, choose_base_strategy};
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    /// Check letter case of commands.
    #[serde(default, alias = "command_upcase")]
    pub command_case: Option<CommandCase>,
    /// Use `cmake-lint` to provide more lints.
    #[serde(default)]
    pub enable_external_cmake_lint: bool,
    /// Max line length.
    #[serde(default = "default_max_words")]
    pub line_max_words: usize,
    #[serde(default)]
    pub format: FormatConfig,
}

const fn default_max_words() -> usize {
    80
}

impl Default for Config {
    fn default() -> Self {
        Self {
            command_case: None,
            enable_external_cmake_lint: false,
            line_max_words: default_max_words(),
            format: FormatConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandCase {
    #[serde(alias = "upcase", alias = "upper_case")]
    Upper,
    #[serde(alias = "lowercase", alias = "lower_case")]
    Lower,
}

impl CommandCase {
    pub(crate) fn check(&self, command: &str) -> Option<&'static str> {
        let is_all_uppercase = command.chars().all(char::is_uppercase);
        let is_all_lowercase = command.chars().all(char::is_lowercase);

        match (self, is_all_uppercase, is_all_lowercase) {
            (CommandCase::Upper, false, _) => Some("command name should be uppercased"),
            (CommandCase::Lower, _, false) => Some("command name should be lowercased"),
            _ => None,
        }
    }
}

#[derive(Default, Deserialize, PartialEq, Eq, Debug)]
pub struct FormatConfig {
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
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

    let strategy = choose_base_strategy().ok()?;
    let config_dir = strategy.config_dir();

    for file in ["config.toml", "lint.toml"] {
        let path = config_dir.join("neocmakelsp").join(file);
        if path.exists() {
            tracing::info!("Using user-level config file: {:?}", path);
            return Some(path);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_case_config_names() {
        let config = indoc::indoc! { r#"
            command_upcase = "lower_case"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());

        let config = indoc::indoc! { r#"
            command_case = "upcase"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());
        let config = indoc::indoc! { r#"
            command_case = "lowercase"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());

        let config = indoc::indoc! { r#"
            command_case = "upper"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());
        let config = indoc::indoc! { r#"
            command_case = "lower"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());

        let config = indoc::indoc! { r#"
            command_case = "upper_case"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());
        let config = indoc::indoc! { r#"
            command_case = "lower_case"
        "#};
        assert!(toml::from_str::<Config>(config).is_ok());
    }

    #[test]
    fn empty_config() {
        let config_file = "";
        let config: Config = toml::from_str(config_file).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn empty_args() {
        let config_file = indoc::indoc! {r#"
            [format]
            program = "cmake-format"
        "#};
        let config: Config = toml::from_str(config_file).unwrap();
        let args = config.format.args;
        assert_eq!(config.format.program, Some("cmake-format".to_owned()));
        assert_eq!(args, None);
    }

    #[test]
    fn has_args() {
        let config_file = indoc::indoc! {r#"
            [format]
            program = "cmake-format"
            args = ["--hello"]
        "#};
        let config: Config = toml::from_str(config_file).unwrap();
        let args = config.format.args;
        assert_eq!(config.format.program, Some("cmake-format".to_owned()));
        assert_eq!(args, Some(vec!["--hello".to_owned()]));
    }

    #[test]
    fn check_lower_case_word() {
        assert_eq!(
            CommandCase::Upper.check("add_executable"),
            Some("command name should be uppercased")
        );
    }

    #[test]
    fn check_upper_case_word() {
        assert_eq!(
            CommandCase::Lower.check("ADD_EXECUTABLE"),
            Some("command name should be lowercased")
        );
    }

    #[test]
    fn check_mixed_case_word() {
        assert_eq!(
            CommandCase::Lower.check("Add_Executable"),
            Some("command name should be lowercased")
        );
        assert_eq!(
            CommandCase::Upper.check("Add_Executable"),
            Some("command name should be uppercased")
        );
    }
}
