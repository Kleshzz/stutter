use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_focused_nice")]
    pub focused_nice: i32,

    #[serde(default = "default_default_nice")]
    pub default_nice: i32,
}

fn default_focused_nice() -> i32 { -5 }
fn default_default_nice() -> i32 { 0 }

impl Default for Config {
    fn default() -> Self {
        Self {
            focused_nice: default_focused_nice(),
            default_nice: default_default_nice(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config").join("stutter").join("config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
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
        return Config::default();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Config::default();
    };

    toml::from_str(&content).unwrap_or_default()
}