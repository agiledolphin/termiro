//! 最高分存档：`<数据目录>/termino/highscore.toml`。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub score: u64,
    pub lines: u32,
    pub level: u32,
}

/// 存档路径。macOS 为 `~/Library/Application Support/termino/highscore.toml`，
/// Linux 为 `~/.local/share/termino/highscore.toml`，Windows 为 `%APPDATA%\termino\data\highscore.toml`。
pub fn path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "termino").map(|dirs| dirs.data_dir().join("highscore.toml"))
}

/// 读取最高分。文件不存在或无法解析时视为没有纪录。
pub fn load() -> Option<Record> {
    load_from(&path()?)
}

pub fn save(record: &Record) -> io::Result<()> {
    let path = path().ok_or_else(|| io::Error::other("no data directory on this system"))?;
    save_to(&path, record)
}

fn load_from(path: &Path) -> Option<Record> {
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn save_to(path: &Path, record: &Record) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(record).map_err(io::Error::other)?;
    // 先写临时文件再改名，写到一半退出也不会弄坏原来的纪录
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("termino-test-{}-{name}", std::process::id()))
            .join("highscore.toml")
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = temp_path("round-trip");
        let record = Record {
            score: 12_345,
            lines: 42,
            level: 5,
        };
        save_to(&path, &record).unwrap();
        assert_eq!(load_from(&path), Some(record));

        let better = Record {
            score: 99_999,
            ..record
        };
        save_to(&path, &better).unwrap();
        assert_eq!(load_from(&path), Some(better));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_or_corrupt_file_means_no_record() {
        let path = temp_path("corrupt");
        assert_eq!(load_from(&path), None);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "score = \"lots\"").unwrap();
        assert_eq!(load_from(&path), None);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
