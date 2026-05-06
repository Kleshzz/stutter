use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub focused_nice: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_focused_nice")]
    pub focused_nice: i32,

    #[serde(default = "default_default_nice")]
    pub default_nice: i32,

    #[serde(default)]
    pub apps: HashMap<String, AppConfig>,
}

const fn default_focused_nice() -> i32 {
    -5
}
const fn default_default_nice() -> i32 {
    0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            focused_nice: default_focused_nice(),
            default_nice: default_default_nice(),
            apps: HashMap::new(),
        }
    }
}

impl Config {
    fn validate(mut self) -> Self {
        let orig_focused = self.focused_nice;
        let orig_default = self.default_nice;

        self.focused_nice = self.focused_nice.clamp(-20, 19);
        self.default_nice = self.default_nice.clamp(-20, 19);

        if self.focused_nice != orig_focused {
            warn!(
                "focused_nice ({}) is out of bounds, clamped to {}",
                orig_focused, self.focused_nice
            );
        }
        if self.default_nice != orig_default {
            warn!(
                "default_nice ({}) is out of bounds, clamped to {}",
                orig_default, self.default_nice
            );
        }

        if self.focused_nice >= self.default_nice {
            warn!(
                "focused_nice ({}) is >= default_nice ({}). \
                 The focused window will not receive a higher priority.",
                self.focused_nice, self.default_nice
            );
        }

        for (name, app) in &mut self.apps {
            if let Some(ref mut n) = app.focused_nice {
                let orig = *n;
                *n = (*n).clamp(-20, 19);
                if *n != orig {
                    warn!("apps.{name}.focused_nice ({orig}) clamped to {n}");
                }
            }
        }

        self
    }

    pub fn focused_nice_for(&self, class: &str) -> i32 {
        self.apps
            .get(class)
            .and_then(|a| a.focused_nice)
            .unwrap_or(self.focused_nice)
    }
}

fn config_path() -> Option<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").ok()?;
        PathBuf::from(home).join(".config")
    };
    Some(base.join("stutter").join("config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        warn!("could not determine config path (HOME/XDG_CONFIG_HOME not set), using defaults");
        return Config::default().validate();
    };

    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("failed to create config dir: {e}");
        }
    }

    let default_content = "\
# stutter configuration file

# CPU priority of the focused window (lower = higher priority, min -20)
focused_nice = -5

# CPU priority of all other windows (restored when window loses focus)
default_nice = 0
";

    // atomically create the file only if it doesn't exist to avoid TOCTOU race
    if std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .is_ok()
    {
        if let Err(e) = std::fs::write(&path, default_content) {
            warn!("failed to write default config: {e}");
        }
        return Config::default().validate();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Config::default().validate();
    };

    let config: Config = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to parse config file: {e}");
            warn!("using default configuration");
            Config::default()
        }
    };
    config.validate()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn default_values_are_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.focused_nice, -5);
        assert_eq!(cfg.default_nice, 0);
    }

    #[test]
    fn validate_clamps_out_of_range() {
        let cfg = Config {
            focused_nice: -99,
            default_nice: 100,
            ..Default::default()
        }
        .validate();
        assert_eq!(cfg.focused_nice, -20);
        assert_eq!(cfg.default_nice, 19);
    }

    #[test]
    fn validate_leaves_valid_values_unchanged() {
        let cfg = Config {
            focused_nice: -5,
            default_nice: 0,
            ..Default::default()
        }
        .validate();
        assert_eq!(cfg.focused_nice, -5);
        assert_eq!(cfg.default_nice, 0);
    }

    #[test]
    fn parse_valid_toml() {
        let cfg: Config = toml::from_str("focused_nice = -10\ndefault_nice = 5").unwrap();
        assert_eq!(cfg.focused_nice, -10);
        assert_eq!(cfg.default_nice, 5);
    }

    #[test]
    fn parse_invalid_toml_falls_back_to_default() {
        let cfg: Config = toml::from_str("not valid toml ][").unwrap_or_default();
        assert_eq!(cfg.focused_nice, -5);
        assert_eq!(cfg.default_nice, 0);
    }

    #[test]
    fn parse_partial_toml_uses_defaults_for_missing_fields() {
        let cfg: Config = toml::from_str("focused_nice = -15").unwrap();
        assert_eq!(cfg.focused_nice, -15);
        assert_eq!(cfg.default_nice, 0);
    }

    #[test]
    fn focused_nice_equal_to_default_nice_is_warned_but_valid() {
        let cfg = Config {
            focused_nice: 0,
            default_nice: 0,
            ..Default::default()
        }
        .validate();
        assert_eq!(cfg.focused_nice, 0);
        assert_eq!(cfg.default_nice, 0);
    }

    #[test]
    fn focused_nice_greater_than_default_nice_is_warned_but_valid() {
        let cfg = Config {
            focused_nice: 5,
            default_nice: 0,
            ..Default::default()
        }
        .validate();
        assert_eq!(cfg.focused_nice, 5);
    }

    #[test]
    fn per_app_override_is_used() {
        let cfg: Config =
            toml::from_str("focused_nice = -5\ndefault_nice = 0\n[apps]\nfirefox = { focused_nice = -10 }")
                .unwrap();
        assert_eq!(cfg.focused_nice_for("firefox"), -10);
        assert_eq!(cfg.focused_nice_for("kitty"), -5);
    }

    #[test]
    fn per_app_without_focused_nice_falls_back_to_global() {
        let cfg: Config =
            toml::from_str("focused_nice = -5\ndefault_nice = 0\n[apps]\nfirefox = {}").unwrap();
        assert_eq!(cfg.focused_nice_for("firefox"), -5);
    }
}
