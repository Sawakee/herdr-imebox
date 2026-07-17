//! User configuration from `~/.config/herdr-imebox/config.toml`.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Height of the text box as a fraction of the target pane.
    pub ratio: f64,
    /// Number of consecutive Enter presses that send the message
    /// (the blank newlines they typed are stripped first); 0 disables.
    pub enter_send_count: usize,
    /// Maximum number of sent messages kept in the history file.
    pub history_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ratio: 0.25,
            enter_send_count: 2,
            history_size: 100,
        }
    }
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    base.join("herdr-imebox").join("config.toml")
}

impl Config {
    /// Load the config file; missing or invalid files fall back to defaults.
    pub fn load() -> Self {
        let Ok(s) = std::fs::read_to_string(config_path()) else {
            return Self::default();
        };
        toml::from_str(&s).unwrap_or_else(|e| {
            eprintln!("imebox: ignoring invalid config: {e}");
            Self::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn partial_override() {
        let cfg: Config = toml::from_str("ratio = 0.4\nenter_send_count = 3").unwrap();
        assert_eq!(cfg.ratio, 0.4);
        assert_eq!(cfg.enter_send_count, 3);
        assert_eq!(cfg.history_size, 100);
    }

    #[test]
    fn default_is_double_enter() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.enter_send_count, 2);
    }

    #[test]
    fn unknown_keys_ignored() {
        let cfg: Config = toml::from_str("future_option = true").unwrap();
        assert_eq!(cfg, Config::default());
    }
}
