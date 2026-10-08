//! 把按键事件变成命令，并在能收到按键释放事件时自己实现长按连发（DAS/ARR）。
//!
//! 两种模式：
//! - 完整模式：终端支持 kitty 键盘协议，或在 Windows 上。能知道键何时松开，
//!   左右移动和软降的连发由这里按配置的时间控制，旋转等按键不会连发。
//! - 降级模式：收不到释放事件，每个按下或重复事件对应一次命令，连发速度取决于系统设置。

use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use termino_core::{Action, HEIGHT, WIDTH};

use crate::config::Timing;
use crate::keymap::{Command, Keymap, normalize};

pub struct Input {
    keymap: Keymap,
    das: Duration,
    arr: Duration,
    soft_drop: Duration,
    /// 能否收到按键释放事件。
    key_release: bool,
    /// 按住的左右方向键，最后按下的在末尾、优先生效。
    horizontal: Vec<(KeyCode, Action)>,
    shift: Repeater,
    /// 按住的软降键。
    soft_drop_keys: Vec<KeyCode>,
    drop: Repeater,
}

/// 长按连发计时：先等 `delay`，之后每隔 `interval` 触发一次。
#[derive(Default)]
struct Repeater {
    elapsed: Duration,
    charged: bool,
}

impl Repeater {
    /// 推进 `dt`，返回这段时间内应触发的次数。`interval` 为 0 时一次触发 `instant` 次。
    fn advance(
        &mut self,
        dt: Duration,
        delay: Duration,
        interval: Duration,
        instant: usize,
    ) -> usize {
        self.elapsed += dt;
        let mut count = 0;
        if !self.charged {
            if self.elapsed < delay {
                return 0;
            }
            self.elapsed -= delay;
            self.charged = true;
            count = 1;
        }
        if interval.is_zero() {
            self.elapsed = Duration::ZERO;
            return instant;
        }
        while self.elapsed >= interval {
            self.elapsed -= interval;
            count += 1;
        }
        count
    }
}

impl Input {
    pub fn new(keymap: Keymap, timing: &Timing, key_release: bool) -> Self {
        Self {
            keymap,
            das: timing.das(),
            arr: timing.arr(),
            soft_drop: timing.soft_drop(),
            key_release,
            horizontal: Vec::new(),
            shift: Repeater::default(),
            soft_drop_keys: Vec::new(),
            drop: Repeater::default(),
        }
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// 是否由本模块控制长按连发。
    pub fn has_auto_repeat(&self) -> bool {
        self.key_release
    }

    /// 处理一个按键事件，返回需要立即执行的命令。
    pub fn key(&mut self, key: KeyEvent) -> Option<Command> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let ctrl_c = key.code == KeyCode::Char('c') && key.kind != KeyEventKind::Release;
            return ctrl_c.then_some(Command::ForceQuit);
        }
        let command = self.keymap.get(key.code)?;
        if !self.key_release {
            return (key.kind != KeyEventKind::Release).then_some(command);
        }
        let code = normalize(key.code);
        match key.kind {
            KeyEventKind::Press => self.press(code, command),
            KeyEventKind::Release => {
                self.release(code);
                None
            }
            // 连发由自己控制，忽略终端的重复事件
            KeyEventKind::Repeat => None,
        }
    }

    /// 确认框打开时的按键处理：Y/Enter 确认，N/Esc 取消，不经过按键配置。
    /// 同时松开所有按键，避免确认框期间漏掉的释放事件让方向键卡住。
    pub fn confirm_key(&mut self, key: KeyEvent) -> Option<Command> {
        self.release_all();
        if key.kind != KeyEventKind::Press {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return (key.code == KeyCode::Char('c')).then_some(Command::ForceQuit);
        }
        match normalize(key.code) {
            KeyCode::Char('y') | KeyCode::Enter => Some(Command::Answer(true)),
            KeyCode::Char('n') | KeyCode::Esc => Some(Command::Answer(false)),
            _ => None,
        }
    }

    /// 推进 `dt`，返回长按连发产生的动作。
    pub fn tick(&mut self, dt: Duration) -> Vec<Action> {
        let mut actions = Vec::new();
        if let Some(&(_, direction)) = self.horizontal.last() {
            let count = self.shift.advance(dt, self.das, self.arr, WIDTH as usize);
            actions.extend(std::iter::repeat_n(direction, count));
        }
        if !self.soft_drop_keys.is_empty() {
            let count = self
                .drop
                .advance(dt, self.soft_drop, self.soft_drop, HEIGHT as usize);
            actions.extend(std::iter::repeat_n(Action::SoftDrop, count));
        }
        actions
    }

    /// 松开所有按键。终端失去焦点时调用，因为之后的释放事件可能收不到。
    pub fn release_all(&mut self) {
        self.horizontal.clear();
        self.soft_drop_keys.clear();
    }

    fn press(&mut self, code: KeyCode, command: Command) -> Option<Command> {
        match command {
            Command::Game(direction @ (Action::MoveLeft | Action::MoveRight)) => {
                // Windows 会把按住时的重复上报为按下，这里去重
                if self.horizontal.iter().any(|&(c, _)| c == code) {
                    return None;
                }
                self.horizontal.push((code, direction));
                self.shift = Repeater::default();
            }
            Command::Game(Action::SoftDrop) => {
                if self.soft_drop_keys.contains(&code) {
                    return None;
                }
                if self.soft_drop_keys.is_empty() {
                    self.drop = Repeater::default();
                }
                self.soft_drop_keys.push(code);
            }
            _ => {}
        }
        Some(command)
    }

    fn release(&mut self, code: KeyCode) {
        let was_active = self.horizontal.last().is_some_and(|&(c, _)| c == code);
        self.horizontal.retain(|&(c, _)| c != code);
        // 松开后面按的方向键时，回到仍按着的那个方向，重新等待 DAS
        if was_active {
            self.shift = Repeater::default();
        }
        self.soft_drop_keys.retain(|&c| c != code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventState;

    const LEFT: Command = Command::Game(Action::MoveLeft);
    const RIGHT: Command = Command::Game(Action::MoveRight);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn event(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        }
    }

    fn press(input: &mut Input, code: KeyCode) -> Option<Command> {
        input.key(event(code, KeyEventKind::Press))
    }

    fn release(input: &mut Input, code: KeyCode) {
        assert_eq!(input.key(event(code, KeyEventKind::Release)), None);
    }

    fn native(das: u64, arr: u64, soft_drop: u64) -> Input {
        let timing = Timing {
            das_ms: das,
            arr_ms: arr,
            soft_drop_ms: soft_drop,
        };
        Input::new(Keymap::default(), &timing, true)
    }

    fn fallback() -> Input {
        Input::new(Keymap::default(), &Timing::default(), false)
    }

    #[test]
    fn das_then_arr() {
        let mut input = native(167, 33, 33);
        assert_eq!(press(&mut input, KeyCode::Left), Some(LEFT));
        assert!(input.tick(ms(166)).is_empty());
        assert_eq!(input.tick(ms(1)), [Action::MoveLeft]);
        assert!(input.tick(ms(32)).is_empty());
        assert_eq!(input.tick(ms(1)), [Action::MoveLeft]);
        assert_eq!(input.tick(ms(66)), [Action::MoveLeft; 2]);

        release(&mut input, KeyCode::Left);
        assert!(input.tick(ms(1000)).is_empty());
    }

    #[test]
    fn zero_arr_moves_to_the_wall() {
        let mut input = native(100, 0, 33);
        press(&mut input, KeyCode::Right);
        assert_eq!(input.tick(ms(100)).len(), WIDTH as usize);
    }

    #[test]
    fn last_pressed_direction_wins() {
        let mut input = native(100, 50, 33);
        press(&mut input, KeyCode::Left);
        input.tick(ms(150));
        // 按住左的同时按右：立即右移，并重新计算 DAS
        assert_eq!(press(&mut input, KeyCode::Right), Some(RIGHT));
        assert!(input.tick(ms(99)).is_empty());
        assert_eq!(input.tick(ms(1)), [Action::MoveRight]);

        // 松开右：回到左，不立即移动，重新等待 DAS
        release(&mut input, KeyCode::Right);
        assert!(input.tick(ms(99)).is_empty());
        assert_eq!(input.tick(ms(1)), [Action::MoveLeft]);
    }

    #[test]
    fn duplicate_press_is_ignored() {
        let mut input = native(100, 50, 33);
        press(&mut input, KeyCode::Left);
        input.tick(ms(90));
        assert_eq!(press(&mut input, KeyCode::Left), None);
        assert_eq!(input.tick(ms(10)), [Action::MoveLeft]);
    }

    #[test]
    fn soft_drop_repeats_while_held() {
        let mut input = native(167, 33, 20);
        assert_eq!(
            press(&mut input, KeyCode::Down),
            Some(Command::Game(Action::SoftDrop))
        );
        assert!(input.tick(ms(19)).is_empty());
        assert_eq!(input.tick(ms(21)), [Action::SoftDrop; 2]);
        release(&mut input, KeyCode::Down);
        assert!(input.tick(ms(100)).is_empty());
    }

    #[test]
    fn terminal_repeats_are_ignored_when_releases_are_known() {
        let mut input = native(167, 33, 33);
        assert_eq!(
            press(&mut input, KeyCode::Up),
            Some(Command::Game(Action::RotateCw))
        );
        assert_eq!(input.key(event(KeyCode::Up, KeyEventKind::Repeat)), None);
    }

    #[test]
    fn release_all_stops_repeating() {
        let mut input = native(100, 50, 33);
        press(&mut input, KeyCode::Left);
        press(&mut input, KeyCode::Down);
        input.release_all();
        assert!(input.tick(ms(1000)).is_empty());
    }

    #[test]
    fn fallback_maps_every_press_and_never_repeats() {
        let mut input = fallback();
        assert_eq!(press(&mut input, KeyCode::Left), Some(LEFT));
        assert_eq!(press(&mut input, KeyCode::Left), Some(LEFT));
        assert_eq!(input.key(event(KeyCode::Left, KeyEventKind::Release)), None);
        assert!(input.tick(ms(1000)).is_empty());
    }

    #[test]
    fn confirm_keys() {
        let mut input = native(100, 50, 33);
        assert_eq!(
            input.confirm_key(event(KeyCode::Char('Y'), KeyEventKind::Press)),
            Some(Command::Answer(true))
        );
        assert_eq!(
            input.confirm_key(event(KeyCode::Enter, KeyEventKind::Press)),
            Some(Command::Answer(true))
        );
        assert_eq!(
            input.confirm_key(event(KeyCode::Esc, KeyEventKind::Press)),
            Some(Command::Answer(false))
        );
        // 打开确认框的那个键松开或连发时不应当被当作回答
        assert_eq!(
            input.confirm_key(event(KeyCode::Esc, KeyEventKind::Release)),
            None
        );
        assert_eq!(
            input.confirm_key(event(KeyCode::Char('q'), KeyEventKind::Press)),
            None
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(input.confirm_key(ctrl_c), Some(Command::ForceQuit));
    }

    #[test]
    fn confirm_dialog_releases_held_keys() {
        let mut input = native(100, 50, 33);
        press(&mut input, KeyCode::Left);
        input.confirm_key(event(KeyCode::Char('n'), KeyEventKind::Press));
        assert!(input.tick(ms(1000)).is_empty());
    }

    #[test]
    fn ctrl_c_always_quits() {
        let mut input = native(167, 33, 33);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(input.key(ctrl('c')), Some(Command::ForceQuit));
        assert_eq!(input.key(ctrl('z')), None);
    }
}
