use std::time::Duration;

use termino_core::{Action, Clear, Event, Game, GameView, Rules};

use crate::keymap::Command;

/// 需要玩家确认的操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Quit,
    /// 放弃进行中的这一局，重新开始。
    Restart,
}

/// 消行提示的显示时长。
const BANNER_TIME: Duration = Duration::from_secs(2);

/// 应用状态：开始界面或一局游戏，加上暂停、退出标记和消行提示。
pub struct App {
    seed: u64,
    game: Game,
    /// 还停在开始界面，游戏尚未开始。
    title: bool,
    paused: bool,
    /// 正在等待玩家确认的操作；期间游戏不计时。
    confirm: Option<Confirm>,
    quit: bool,
    /// 自上一帧以来收到的动作，在下一个逻辑帧统一交给 core。
    pending: Vec<Action>,
    /// 最近一次消行或 T-Spin，以及剩余显示时间。
    banner: Option<(Clear, Duration)>,
}

impl App {
    /// 停在开始界面，按硬降键开始。
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            game: Game::new(seed, Rules::default()),
            title: true,
            paused: false,
            confirm: None,
            quit: false,
            pending: Vec::new(),
            banner: None,
        }
    }

    pub fn handle(&mut self, command: Command) {
        if command == Command::ForceQuit {
            self.quit = true;
            return;
        }
        if let Some(confirm) = self.confirm {
            if let Command::Answer(yes) = command {
                self.confirm = None;
                if yes {
                    match confirm {
                        Confirm::Quit => self.quit = true,
                        Confirm::Restart => self.restart(),
                    }
                }
            }
            return;
        }
        if command == Command::Quit {
            self.confirm = Some(Confirm::Quit);
            return;
        }
        if self.title {
            // 这次按键只用来开始，不会把第一个方块砸下去
            if command == Command::Game(Action::HardDrop) {
                self.title = false;
            }
            return;
        }
        let over = self.game.view().game_over.is_some();
        match command {
            Command::Pause if !over => self.paused = !self.paused,
            // 已经结束就直接重开；否则会丢掉进行中的这一局，先确认
            Command::Restart if over => self.restart(),
            Command::Restart => self.confirm = Some(Confirm::Restart),
            Command::Game(action) if !self.paused && !over => self.pending.push(action),
            _ => {}
        }
    }

    /// 开始新的一局，不回开始界面。
    fn restart(&mut self) {
        *self = Self::new(self.seed.wrapping_add(1));
        self.title = false;
    }

    /// 暂停游戏；在开始界面、已暂停或已结束时无效果。
    pub fn pause(&mut self) {
        if !self.title && self.game.view().game_over.is_none() {
            self.paused = true;
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        if self.title || self.paused || self.confirm.is_some() {
            return;
        }
        if let Some((_, remaining)) = &mut self.banner {
            *remaining = remaining.saturating_sub(dt);
            if remaining.is_zero() {
                self.banner = None;
            }
        }
        for event in self.game.update(dt, &self.pending) {
            if let Event::Clear(clear) = event {
                self.banner = Some((clear, BANNER_TIME));
            }
        }
        self.pending.clear();
    }

    pub fn view(&self) -> GameView<'_> {
        self.game.view()
    }

    pub fn banner(&self) -> Option<Clear> {
        self.banner.map(|(clear, _)| clear)
    }

    pub fn on_title(&self) -> bool {
        self.title
    }

    pub fn confirming(&self) -> Option<Confirm> {
        self.confirm
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    fn top(app: &App) -> i32 {
        app.view().active.unwrap().origin.y
    }

    /// 从开始界面进入游戏。
    pub(crate) fn playing(seed: u64) -> App {
        let mut app = App::new(seed);
        app.handle(Command::Game(Action::HardDrop));
        app
    }

    /// 一直硬降直到游戏结束。
    fn play_until_over(app: &mut App) {
        while app.view().game_over.is_none() {
            app.handle(Command::Game(Action::HardDrop));
            app.tick(Duration::ZERO);
        }
    }

    #[test]
    fn title_screen_waits_for_hard_drop_key() {
        let mut app = App::new(1);
        let start = app.view().active;
        app.handle(Command::Game(Action::MoveLeft));
        app.handle(Command::Pause);
        app.pause();
        app.tick(SECOND * 5);
        assert!(app.on_title());
        assert!(!app.paused());
        assert_eq!(app.view().active, start);

        // 开始游戏的这次按键不会硬降
        app.handle(Command::Game(Action::HardDrop));
        app.tick(Duration::ZERO);
        assert!(!app.on_title());
        assert_eq!(app.view().board.filled_count(), 0);
    }

    #[test]
    fn actions_apply_on_next_tick() {
        let mut app = playing(1);
        let x = app.view().active.unwrap().origin.x;
        app.handle(Command::Game(Action::MoveLeft));
        assert_eq!(app.view().active.unwrap().origin.x, x);
        app.tick(Duration::ZERO);
        assert_eq!(app.view().active.unwrap().origin.x, x - 1);
    }

    #[test]
    fn pause_freezes_game_and_drops_input() {
        let mut app = playing(1);
        let y = top(&app);
        app.handle(Command::Pause);
        app.handle(Command::Game(Action::HardDrop));
        app.tick(SECOND);
        assert_eq!(top(&app), y);

        app.handle(Command::Pause);
        app.tick(SECOND);
        assert_eq!(top(&app), y - 1);
    }

    #[test]
    fn restart_during_game_asks_first() {
        let mut app = playing(1);
        app.handle(Command::Game(Action::HardDrop));
        app.tick(Duration::ZERO);

        app.handle(Command::Restart);
        assert_eq!(app.confirming(), Some(Confirm::Restart));
        // 确认框期间游戏冻结，其它命令无效
        let y = top(&app);
        app.handle(Command::Game(Action::HardDrop));
        app.handle(Command::Pause);
        app.tick(SECOND * 3);
        assert_eq!(top(&app), y);
        assert!(!app.paused());

        app.handle(Command::Answer(false));
        assert_eq!(app.confirming(), None);
        assert_eq!(app.view().board.filled_count(), 4);

        app.handle(Command::Pause);
        app.handle(Command::Restart);
        app.handle(Command::Answer(true));
        assert_eq!(app.view().board.filled_count(), 0);
        assert!(!app.paused());
        assert!(!app.on_title());
    }

    #[test]
    fn restart_after_game_over_is_immediate() {
        let mut app = playing(1);
        play_until_over(&mut app);
        app.handle(Command::Pause);
        assert!(!app.paused());
        app.handle(Command::Restart);
        assert_eq!(app.confirming(), None);
        assert!(app.view().game_over.is_none());
    }

    #[test]
    fn quit_asks_first_everywhere() {
        let mut title = App::new(1);
        let mut game = playing(1);
        let mut over = playing(1);
        play_until_over(&mut over);
        for app in [&mut title, &mut game, &mut over] {
            app.handle(Command::Quit);
            assert_eq!(app.confirming(), Some(Confirm::Quit));
            app.handle(Command::Answer(false));
            assert!(!app.should_quit());
            app.handle(Command::Quit);
            app.handle(Command::Answer(true));
            assert!(app.should_quit());
        }
    }

    #[test]
    fn answer_without_dialog_is_ignored() {
        let mut app = playing(1);
        app.handle(Command::Answer(true));
        assert!(!app.should_quit());
        assert_eq!(app.view().board.filled_count(), 0);
    }

    #[test]
    fn pause_does_not_toggle() {
        let mut app = playing(1);
        app.pause();
        app.pause();
        assert!(app.paused());

        let mut over = playing(1);
        play_until_over(&mut over);
        over.pause();
        assert!(!over.paused());
    }

    #[test]
    fn force_quit_works_in_any_state() {
        let mut app = playing(1);
        app.handle(Command::Pause);
        app.handle(Command::Restart);
        app.handle(Command::ForceQuit);
        assert!(app.should_quit());
    }

    #[test]
    fn banner_expires_but_not_while_paused() {
        let mut app = playing(1);
        let tetris = Clear {
            lines: 4,
            t_spin: None,
            back_to_back: false,
            combo: 0,
            points: 800,
        };
        app.banner = Some((tetris, BANNER_TIME));
        app.tick(SECOND);
        app.handle(Command::Pause);
        app.tick(SECOND * 5);
        assert_eq!(app.banner(), Some(tetris));
        app.handle(Command::Pause);
        app.tick(SECOND);
        assert_eq!(app.banner(), None);
    }
}
