//! 按键到命令的映射。
//!
//! M2 采用降级方案：每个按下或重复事件对应一次动作，长按依赖系统的键盘重复速率。
//! M4 接入 kitty 键盘协议后再自己实现 DAS/ARR。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use termino_core::Action;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Game(Action),
    Pause,
    Restart,
    Quit,
}

pub fn map_key(key: KeyEvent) -> Option<Command> {
    // Windows 上 crossterm 会同时上报按下和释放，只处理按下和重复
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return (key.code == KeyCode::Char('c')).then_some(Command::Quit);
    }

    let command = match key.code {
        KeyCode::Left => Command::Game(Action::MoveLeft),
        KeyCode::Right => Command::Game(Action::MoveRight),
        KeyCode::Down => Command::Game(Action::SoftDrop),
        KeyCode::Up => Command::Game(Action::RotateCw),
        KeyCode::Esc => Command::Quit,
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            ' ' => Command::Game(Action::HardDrop),
            'x' => Command::Game(Action::RotateCw),
            'z' => Command::Game(Action::RotateCcw),
            'c' => Command::Game(Action::Hold),
            'p' => Command::Pause,
            'r' => Command::Restart,
            'q' => Command::Quit,
            _ => return None,
        },
        _ => return None,
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventState;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn maps_game_keys() {
        assert_eq!(
            map_key(key(KeyCode::Left)),
            Some(Command::Game(Action::MoveLeft))
        );
        assert_eq!(
            map_key(key(KeyCode::Char(' '))),
            Some(Command::Game(Action::HardDrop))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('Z'))),
            Some(Command::Game(Action::RotateCcw))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('c'))),
            Some(Command::Game(Action::Hold))
        );
        assert_eq!(map_key(key(KeyCode::Char('?'))), None);
    }

    #[test]
    fn ignores_key_release() {
        let release = KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert_eq!(map_key(release), None);
    }

    #[test]
    fn ctrl_c_quits_but_other_ctrl_keys_do_nothing() {
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(map_key(ctrl('c')), Some(Command::Quit));
        assert_eq!(map_key(ctrl('z')), None);
    }
}
