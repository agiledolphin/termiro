//! 按键名与命令之间的映射。

use ratatui::crossterm::event::KeyCode;
use termino_core::Action;

use crate::config::Keys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Game(Action),
    Pause,
    Restart,
    Quit,
}

#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: Vec<(KeyCode, Command)>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new(&Keys::default()).expect("默认按键配置应当合法")
    }
}

impl Keymap {
    /// 解析配置里的按键名。按键名无法识别、或同一个键绑定了两个不同命令时报错。
    pub fn new(keys: &Keys) -> Result<Self, String> {
        // 第一列是配置文件里的字段名，用于报错
        let groups = [
            (
                "move_left",
                &keys.move_left,
                Command::Game(Action::MoveLeft),
            ),
            (
                "move_right",
                &keys.move_right,
                Command::Game(Action::MoveRight),
            ),
            (
                "soft_drop",
                &keys.soft_drop,
                Command::Game(Action::SoftDrop),
            ),
            (
                "hard_drop",
                &keys.hard_drop,
                Command::Game(Action::HardDrop),
            ),
            (
                "rotate_cw",
                &keys.rotate_cw,
                Command::Game(Action::RotateCw),
            ),
            (
                "rotate_ccw",
                &keys.rotate_ccw,
                Command::Game(Action::RotateCcw),
            ),
            ("hold", &keys.hold, Command::Game(Action::Hold)),
            ("pause", &keys.pause, Command::Pause),
            ("restart", &keys.restart, Command::Restart),
            ("quit", &keys.quit, Command::Quit),
        ];
        let mut bindings: Vec<(KeyCode, Command)> = Vec::new();
        let mut fields: Vec<&str> = Vec::new();
        for (field, names, command) in groups {
            for name in names {
                let code = parse_key(name).map_err(|e| format!("keys.{field}: {e}"))?;
                match bindings.iter().position(|(c, _)| *c == code) {
                    Some(i) if bindings[i].1 != command => {
                        return Err(format!(
                            "key {name:?} is bound to both keys.{} and keys.{field}",
                            fields[i]
                        ));
                    }
                    Some(_) => {}
                    None => {
                        bindings.push((code, command));
                        fields.push(field);
                    }
                }
            }
        }
        Ok(Self { bindings })
    }

    pub fn get(&self, code: KeyCode) -> Option<Command> {
        let code = normalize(code);
        self.bindings
            .iter()
            .find(|(c, _)| *c == code)
            .map(|&(_, command)| command)
    }

    /// 某命令绑定的所有按键，用于界面提示，例如 `Up X`。
    pub fn label(&self, command: Command) -> String {
        self.bindings
            .iter()
            .filter(|(_, c)| *c == command)
            .map(|&(code, _)| key_name(code))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// 字母不区分大小写：按住 Shift 时终端报告的是大写字母。
pub fn normalize(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other => other,
    }
}

fn parse_key(name: &str) -> Result<KeyCode, String> {
    let code = match name.to_ascii_lowercase().as_str() {
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "space" => KeyCode::Char(' '),
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "esc" | "escape" => KeyCode::Esc,
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if !c.is_control() && c != ' ' => KeyCode::Char(c),
                _ => return Err(format!("unknown key name {name:?}")),
            }
        }
    };
    Ok(code)
}

fn key_name(code: KeyCode) -> String {
    match code {
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Backspace => "Bksp".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys_with(f: impl FnOnce(&mut Keys)) -> Keys {
        let mut keys = Keys::default();
        f(&mut keys);
        keys
    }

    #[test]
    fn default_bindings() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.get(KeyCode::Char(' ')),
            Some(Command::Game(Action::HardDrop))
        );
        assert_eq!(
            keymap.get(KeyCode::Char('Z')),
            Some(Command::Game(Action::RotateCcw))
        );
        assert_eq!(keymap.get(KeyCode::Char('?')), None);
        assert_eq!(keymap.label(Command::Game(Action::RotateCw)), "Up X");
        assert_eq!(keymap.label(Command::Quit), "Q Esc");
    }

    #[test]
    fn custom_bindings_are_case_insensitive() {
        let keymap = Keymap::new(&keys_with(|k| {
            k.move_left = vec!["A".into(), "left".into()]
        }))
        .unwrap();
        assert_eq!(
            keymap.get(KeyCode::Char('a')),
            Some(Command::Game(Action::MoveLeft))
        );
        assert_eq!(
            keymap.get(KeyCode::Left),
            Some(Command::Game(Action::MoveLeft))
        );
    }

    #[test]
    fn rejects_unknown_key_names() {
        let err = Keymap::new(&keys_with(|k| k.hold = vec!["Shift".into()])).unwrap_err();
        assert!(err.contains("keys.hold") && err.contains("Shift"), "{err}");
    }

    #[test]
    fn rejects_conflicting_bindings() {
        let err = Keymap::new(&keys_with(|k| k.hold = vec!["z".into()])).unwrap_err();
        assert!(
            err.contains("keys.rotate_ccw") && err.contains("keys.hold"),
            "{err}"
        );
    }
}
