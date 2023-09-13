use serde::Deserialize;
use std::io::Read;

const CONFIGFILE: &str = ".neocmake.toml";

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    pub enable_format: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enable_format: Some(true),
        }
    }
}

impl Config {
    pub fn config_from_file() -> Self {
        let Ok(mut file) = std::fs::OpenOptions::new().read(true).open(CONFIGFILE) else {
            return Self::default();
        };

        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            return Self::default();
        }
        toml::from_str(&buf).unwrap_or(Self::default())
    }
}
