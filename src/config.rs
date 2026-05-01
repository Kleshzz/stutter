use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_focused_nice")]
    pub focused_nice: i32,

    #[serde(default = "default_default_nice")]
    pub default_nice: i32,
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
            eprintln!(
                "[stutter] warning: focused_nice ({}) is out of bounds, clamped to {}",
                orig_focused, self.focused_nice
            );
        }
        if self.default_nice != orig_default {
            eprintln!(
                "[stutter] warning: default_nice ({}) is out of bounds, clamped to {}",
                orig_default, self.default_nice
            );
        }

        self
    }
}

fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("stutter")
            .join("config.toml"),
    )
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default().validate();
    };

    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let default_content = "\
# stutter configuration file

# CPU priority of the focused window (lower = higher priority, min -20)
focused_nice = -5

# CPU priority of all other windows
default_nice = 0
";
        let _ = std::fs::write(&path, default_content);
        return Config::default().validate();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Config::default().validate();
    };

    let config: Config = toml::from_str(&content).unwrap_or_default();
    config.validate()
}

#[cfg(test)]
mod tests {
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
}
