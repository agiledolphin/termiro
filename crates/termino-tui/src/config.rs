//! 配置文件：`<配置目录>/termino/config.toml`。文件不存在时使用默认值。

use std::path::PathBuf;
use std::time::Duration;
use std::{fs, io};

use directories::ProjectDirs;
use serde::Deserialize;

/// 默认配置，也是 `--default-config` 输出的内容。
pub const DEFAULT_TOML: &str = include_str!("default_config.toml");

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub timing: Timing,
    pub keys: Keys,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    pub das_ms: u64,
    pub arr_ms: u64,
    pub soft_drop_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keys {
    pub move_left: Vec<String>,
    pub move_right: Vec<String>,
    pub soft_drop: Vec<String>,
    pub hard_drop: Vec<String>,
    pub rotate_cw: Vec<String>,
    pub rotate_ccw: Vec<String>,
    pub hold: Vec<String>,
    pub pause: Vec<String>,
    pub restart: Vec<String>,
    pub quit: Vec<String>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            das_ms: 167,
            arr_ms: 33,
            soft_drop_ms: 33,
        }
    }
}

impl Timing {
    pub fn das(&self) -> Duration {
        Duration::from_millis(self.das_ms)
    }

    pub fn arr(&self) -> Duration {
        Duration::from_millis(self.arr_ms)
    }

    pub fn soft_drop(&self) -> Duration {
        Duration::from_millis(self.soft_drop_ms)
    }
}

impl Default for Keys {
    fn default() -> Self {
        let keys = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        Self {
            move_left: keys(&["Left"]),
            move_right: keys(&["Right"]),
            soft_drop: keys(&["Down"]),
            hard_drop: keys(&["Space"]),
            rotate_cw: keys(&["Up", "x"]),
            rotate_ccw: keys(&["z"]),
            hold: keys(&["c"]),
            pause: keys(&["p"]),
            restart: keys(&["r"]),
            quit: keys(&["q", "Esc"]),
        }
    }
}

impl Config {
    /// 配置文件路径。macOS 为 `~/Library/Application Support/termino/config.toml`，
    /// Linux 为 `~/.config/termino/config.toml`，Windows 为 `%APPDATA%\termino\config\config.toml`。
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "termino").map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// 读取配置文件；文件不存在时返回默认配置。
    pub fn load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_matches_default_values() {
        assert_eq!(Config::parse(DEFAULT_TOML).unwrap(), Config::default());
    }

    #[test]
    fn missing_fields_use_defaults() {
        let config = Config::parse(
            r#"
            [timing]
            arr_ms = 0
            [keys]
            hold = ["Shift", "c"]
            "#,
        )
        .unwrap();
        assert_eq!(config.timing.arr_ms, 0);
        assert_eq!(config.timing.das_ms, 167);
        assert_eq!(config.keys.hold, ["Shift", "c"]);
        assert_eq!(config.keys.quit, Keys::default().quit);
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = Config::parse("[keys]\nfly = [\"f\"]").unwrap_err();
        assert!(err.to_string().contains("fly"), "{err}");
    }
}
