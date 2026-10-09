use std::time::Duration;

use termino_core::{Action, Clear, Event, Game, GameView, Rules};

use crate::keymap::Command;
use crate::party::Party;
use crate::storage::Record;

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
    /// 彩蛋的播放进度；`None` 表示没有在播放。
    party: Option<Party>,
    /// 写在彩蛋蛋糕上的名字。
    name: Option<String>,
    /// 彩蛋数字蜡烛上的年龄。
    age: Option<u8>,
    quit: bool,
    /// 自上一帧以来收到的动作，在下一个逻辑帧统一交给 core。
    pending: Vec<Action>,
    /// 最近一次消行或 T-Spin，以及剩余显示时间。
    banner: Option<(Clear, Duration)>,
    /// 历史最高纪录，包括本次运行中刚创下的。
    best: Option<Record>,
    /// 本局已经计入纪录，避免结束后再退出或重开时重复计入。
    recorded: bool,
    /// 本局打破了纪录。
    new_record: bool,
    /// 还没写入磁盘的新纪录，由主循环取走保存。
    unsaved: Option<Record>,
    save_failed: bool,
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
            party: None,
            name: None,
            age: None,
            quit: false,
            pending: Vec::new(),
            banner: None,
            best: None,
            recorded: false,
            new_record: false,
            unsaved: None,
            save_failed: false,
        }
    }

    /// 设置写在彩蛋蛋糕上的名字。去掉控制字符和首尾空白，空名字表示不写。
    pub fn with_name(mut self, name: &str) -> Self {
        let name: String = name.chars().filter(|c| !c.is_control()).collect();
        let name = name.trim();
        self.name = (!name.is_empty()).then(|| name.to_string());
        self
    }

    /// 设置彩蛋数字蜡烛上的年龄；不在 1 到 99 之间时不显示。
    pub fn with_age(mut self, age: u32) -> Self {
        self.age = u8::try_from(age).ok().filter(|age| (1..=99).contains(age));
        self
    }

    /// 设置启动时读到的历史最高纪录。
    pub fn with_best(mut self, best: Option<Record>) -> Self {
        self.best = best;
        self
    }

    pub fn handle(&mut self, command: Command) {
        if command == Command::ForceQuit {
            self.finish();
            self.quit = true;
            return;
        }
        if let Some(party) = &mut self.party {
            match command {
                Command::Dismiss => self.party = None,
                Command::Blow => party.blow(),
                _ => {}
            }
            return;
        }
        if let Some(confirm) = self.confirm {
            if let Command::Answer(yes) = command {
                self.confirm = None;
                if yes {
                    match confirm {
                        Confirm::Quit => {
                            self.finish();
                            self.quit = true;
                        }
                        Confirm::Restart => self.restart(),
                    }
                }
            }
            return;
        }
        if command == Command::Quit {
            // 开始界面没有进行中的游戏，直接退出
            if self.title {
                self.quit = true;
            } else {
                self.confirm = Some(Confirm::Quit);
            }
            return;
        }
        if self.title {
            match command {
                // 这次按键只用来开始，不会把第一个方块砸下去
                Command::Game(Action::HardDrop) => self.title = false,
                Command::EasterEgg => self.party = Some(Party::default()),
                _ => {}
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

    /// 开始新的一局，不回开始界面。被放弃的这一局分数照样计入纪录。
    fn restart(&mut self) {
        self.finish();
        let unsaved = self.unsaved.take();
        let (name, age) = (self.name.take(), self.age);
        *self = Self::new(self.seed.wrapping_add(1)).with_best(self.best);
        self.unsaved = unsaved;
        self.name = name;
        self.age = age;
        self.title = false;
    }

    /// 一局结束（游戏结束、重开或退出）时调用：分数超过纪录就记下来，等待保存。
    fn finish(&mut self) {
        if self.title || self.recorded {
            return;
        }
        self.recorded = true;
        let view = self.game.view();
        let record = Record {
            score: view.score,
            lines: view.lines,
            level: view.level,
        };
        if record.score > 0 && self.best.is_none_or(|best| record.score > best.score) {
            self.best = Some(record);
            self.new_record = true;
            self.unsaved = Some(record);
        }
    }

    /// 暂停游戏；在开始界面、已暂停或已结束时无效果。
    pub fn pause(&mut self) {
        if !self.title && self.game.view().game_over.is_none() {
            self.paused = true;
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        if let Some(party) = &mut self.party {
            party.tick(dt);
            return;
        }
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
            match event {
                Event::Clear(clear) => self.banner = Some((clear, BANNER_TIME)),
                Event::GameOver(_) => self.finish(),
                Event::Locked(_) => {}
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

    /// 显示用的最高分：历史纪录和当前分数取大者。
    pub fn best_score(&self) -> u64 {
        let current = if self.title {
            0
        } else {
            self.game.view().score
        };
        self.best.map_or(0, |best| best.score).max(current)
    }

    pub fn new_record(&self) -> bool {
        self.new_record
    }

    pub fn take_unsaved(&mut self) -> Option<Record> {
        self.unsaved.take()
    }

    pub fn set_save_failed(&mut self) {
        self.save_failed = true;
    }

    pub fn save_failed(&self) -> bool {
        self.save_failed
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn age(&self) -> Option<u8> {
        self.age
    }

    /// 彩蛋的播放进度。
    pub fn party(&self) -> Option<&Party> {
        self.party.as_ref()
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
    use crate::party::{INTRO, Stage};

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
    fn easter_egg_plays_on_title_until_dismissed() {
        let mut app = App::new(1);
        app.handle(Command::EasterEgg);
        app.tick(INTRO + SECOND);
        assert_eq!(app.party().map(Party::stage), Some(Stage::Lit(SECOND)));
        // 播放期间游戏命令无效
        app.handle(Command::Game(Action::HardDrop));
        app.handle(Command::Quit);
        assert!(app.on_title());
        assert!(!app.should_quit());

        app.handle(Command::Blow);
        assert!(matches!(
            app.party().map(Party::stage),
            Some(Stage::Blown(_))
        ));

        app.handle(Command::Dismiss);
        assert!(app.party().is_none());
        assert!(app.on_title());
    }

    #[test]
    fn name_and_age_are_cleaned_and_survive_restart() {
        assert_eq!(App::new(1).with_name("  \t ").name(), None);
        assert_eq!(App::new(1).with_age(0).age(), None);
        assert_eq!(App::new(1).with_age(100).age(), None);
        let mut app = playing(1).with_name(" 小明\n ").with_age(51);
        assert_eq!(app.name(), Some("小明"));
        app.handle(Command::Restart);
        app.handle(Command::Answer(true));
        assert_eq!(app.name(), Some("小明"));
        assert_eq!(app.age(), Some(51));
    }

    #[test]
    fn easter_egg_only_on_title() {
        let mut app = playing(1);
        app.handle(Command::EasterEgg);
        assert!(app.party().is_none());
    }

    #[test]
    fn force_quit_during_easter_egg() {
        let mut app = App::new(1);
        app.handle(Command::EasterEgg);
        app.handle(Command::ForceQuit);
        assert!(app.should_quit());
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
    fn quit_from_title_is_immediate() {
        let mut app = App::new(1);
        app.handle(Command::Quit);
        assert_eq!(app.confirming(), None);
        assert!(app.should_quit());
    }

    #[test]
    fn quit_asks_first_once_playing() {
        let mut game = playing(1);
        let mut over = playing(1);
        play_until_over(&mut over);
        for app in [&mut game, &mut over] {
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
    fn game_over_sets_new_record_once() {
        let mut app = playing(1).with_best(Some(Record {
            score: 1,
            lines: 0,
            level: 1,
        }));
        play_until_over(&mut app);
        let score = app.view().score;
        assert!(app.new_record());
        assert_eq!(app.best_score(), score);
        assert_eq!(app.take_unsaved().map(|r| r.score), Some(score));
        // 结束后再退出不会重复计入
        app.handle(Command::Quit);
        app.handle(Command::Answer(true));
        assert_eq!(app.take_unsaved(), None);
    }

    #[test]
    fn lower_score_is_not_a_record() {
        let best = Record {
            score: u64::MAX,
            lines: 0,
            level: 1,
        };
        let mut app = playing(1).with_best(Some(best));
        play_until_over(&mut app);
        assert!(!app.new_record());
        assert_eq!(app.take_unsaved(), None);
        assert_eq!(app.best_score(), u64::MAX);
    }

    #[test]
    fn abandoned_game_still_counts() {
        let mut app = playing(1);
        app.handle(Command::Game(Action::HardDrop));
        app.tick(Duration::ZERO);
        let score = app.view().score;
        assert!(score > 0);

        app.handle(Command::Restart);
        app.handle(Command::Answer(true));
        assert!(!app.new_record(), "新的一局还没有破纪录");
        assert_eq!(app.best_score(), score);
        assert_eq!(app.take_unsaved().map(|r| r.score), Some(score));
    }

    #[test]
    fn quitting_from_title_records_nothing() {
        let mut app = App::new(1);
        app.handle(Command::ForceQuit);
        assert_eq!(app.take_unsaved(), None);
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
