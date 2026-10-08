use std::time::Duration;

use termino_core::{Action, Clear, Event, Game, GameView, Rules};

use crate::keymap::Command;

/// 消行提示的显示时长。
const BANNER_TIME: Duration = Duration::from_secs(2);

/// 应用状态：一局游戏，加上暂停、退出标记和消行提示。
pub struct App {
    seed: u64,
    game: Game,
    paused: bool,
    quit: bool,
    /// 自上一帧以来收到的动作，在下一个逻辑帧统一交给 core。
    pending: Vec<Action>,
    /// 最近一次消行或 T-Spin，以及剩余显示时间。
    banner: Option<(Clear, Duration)>,
}

impl App {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            game: Game::new(seed, Rules::default()),
            paused: false,
            quit: false,
            pending: Vec::new(),
            banner: None,
        }
    }

    pub fn handle(&mut self, command: Command) {
        let over = self.game.view().game_over.is_some();
        match command {
            Command::Quit => self.quit = true,
            Command::Pause if !over => self.paused = !self.paused,
            // 只在暂停或结束时允许重开，避免游戏中误触
            Command::Restart if self.paused || over => *self = Self::new(self.seed.wrapping_add(1)),
            Command::Game(action) if !self.paused && !over => self.pending.push(action),
            _ => {}
        }
    }

    /// 暂停游戏；已暂停或已结束时无效果。
    pub fn pause(&mut self) {
        if self.game.view().game_over.is_none() {
            self.paused = true;
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        if self.paused {
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

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    fn top(app: &App) -> i32 {
        app.view().active.unwrap().origin.y
    }

    /// 一直硬降直到游戏结束。
    fn play_until_over(app: &mut App) {
        while app.view().game_over.is_none() {
            app.handle(Command::Game(Action::HardDrop));
            app.tick(Duration::ZERO);
        }
    }

    #[test]
    fn actions_apply_on_next_tick() {
        let mut app = App::new(1);
        let x = app.view().active.unwrap().origin.x;
        app.handle(Command::Game(Action::MoveLeft));
        assert_eq!(app.view().active.unwrap().origin.x, x);
        app.tick(Duration::ZERO);
        assert_eq!(app.view().active.unwrap().origin.x, x - 1);
    }

    #[test]
    fn pause_freezes_game_and_drops_input() {
        let mut app = App::new(1);
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
    fn restart_only_when_paused_or_over() {
        let mut app = App::new(1);
        app.handle(Command::Game(Action::HardDrop));
        app.tick(Duration::ZERO);
        app.handle(Command::Restart);
        assert_eq!(app.view().board.filled_count(), 4);

        app.handle(Command::Pause);
        app.handle(Command::Restart);
        assert_eq!(app.view().board.filled_count(), 0);
        assert!(!app.paused());

        play_until_over(&mut app);
        app.handle(Command::Pause);
        assert!(!app.paused());
        app.handle(Command::Restart);
        assert!(app.view().game_over.is_none());
    }

    #[test]
    fn pause_does_not_toggle() {
        let mut app = App::new(1);
        app.pause();
        app.pause();
        assert!(app.paused());

        let mut over = App::new(1);
        play_until_over(&mut over);
        over.pause();
        assert!(!over.paused());
    }

    #[test]
    fn quit_works_in_any_state() {
        let mut app = App::new(1);
        app.handle(Command::Pause);
        app.handle(Command::Quit);
        assert!(app.should_quit());
    }

    #[test]
    fn banner_expires_but_not_while_paused() {
        let mut app = App::new(1);
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
