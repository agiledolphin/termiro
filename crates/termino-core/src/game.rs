use std::time::Duration;

use crate::board::{Board, VISIBLE_HEIGHT};
use crate::piece::{Piece, PieceKind, Pos, Rotation};
use crate::randomizer::SevenBag;
use crate::rotation;
use crate::scoring::{self, Clear, Scoring, TSpin};

/// 玩家动作。core 只认识动作；按键映射和 DAS/ARR 由输入层负责。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    MoveLeft,
    MoveRight,
    /// 下移一行。已落地时无效果，软降不会触发锁定。
    SoftDrop,
    /// 直接落到底并立即锁定。
    HardDrop,
    RotateCw,
    RotateCcw,
    /// 把当前方块放进暂存区。每个方块锁定前只能用一次。
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// 方块锁定到盘面。
    Locked(PieceKind),
    /// 消行或 T-Spin，紧跟在 `Locked` 之后。
    Clear(Clear),
    GameOver(GameOverReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOverReason {
    /// 新方块出生时与已有方块重叠。
    BlockOut,
    /// 方块整个锁定在可见区域之上。
    LockOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    /// 起始等级。之后每消 10 行升一级，重力随等级加快。
    pub start_level: u32,
    /// 方块落地后到锁定的延迟。
    pub lock_delay: Duration,
    /// 落地后移动或旋转能重置锁定延迟的最大次数，防止无限拖延。
    pub max_lock_resets: u32,
    /// 预览队列长度。
    pub preview: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            start_level: 1,
            lock_delay: Duration::from_millis(500),
            max_lock_resets: 15,
            preview: 5,
        }
    }
}

/// 给渲染层用的只读快照。
#[derive(Clone, Copy, Debug)]
pub struct GameView<'a> {
    pub board: &'a Board,
    pub active: Option<Piece>,
    /// 硬降落点预览。
    pub ghost: Option<Piece>,
    pub next: &'a [PieceKind],
    pub hold: Option<PieceKind>,
    /// 当前方块是否还能暂存。
    pub can_hold: bool,
    pub score: u64,
    pub level: u32,
    pub lines: u32,
    pub game_over: Option<GameOverReason>,
}

#[derive(Clone, Debug)]
pub struct Game {
    rules: Rules,
    board: Board,
    bag: SevenBag,
    queue: Vec<PieceKind>,
    /// 当前方块；只在游戏结束后为 `None`。
    active: Option<Piece>,
    hold: Option<PieceKind>,
    can_hold: bool,
    gravity_timer: Duration,
    /// 落地后累计的时间，离开地面时清零。
    lock_timer: Duration,
    lock_resets: u32,
    /// 当前方块到过的最低行。到达新低点时 `lock_resets` 清零。
    lowest_y: i32,
    /// 当前方块最后一次操作若是旋转，记录所用的踢墙序号；任何位移都会清除它。
    /// 用于 T-Spin 判定。
    last_kick: Option<usize>,
    scoring: Scoring,
    lines: u32,
    over: Option<GameOverReason>,
}

/// Guideline 规定的出生位置：出生朝向、水平居中偏左，位于可见区上方的两行。
fn spawn_piece(kind: PieceKind) -> Piece {
    let origin = match kind {
        PieceKind::I => Pos::new(3, VISIBLE_HEIGHT - 2),
        PieceKind::O => Pos::new(4, VISIBLE_HEIGHT),
        _ => Pos::new(3, VISIBLE_HEIGHT - 1),
    };
    Piece {
        kind,
        rotation: Rotation::Spawn,
        origin,
    }
}

impl Game {
    pub fn new(seed: u64, rules: Rules) -> Self {
        Self::with_board(seed, rules, Board::new())
    }

    /// 从指定盘面开局，用于残局模式和测试。
    /// 如果第一个方块就放不下，返回的游戏直接处于结束状态。
    pub fn with_board(seed: u64, rules: Rules, board: Board) -> Self {
        let mut bag = SevenBag::new(seed);
        let queue = bag.by_ref().take(rules.preview).collect();
        let mut game = Self {
            rules,
            board,
            bag,
            queue,
            active: None,
            hold: None,
            can_hold: true,
            gravity_timer: Duration::ZERO,
            lock_timer: Duration::ZERO,
            lock_resets: 0,
            lowest_y: 0,
            last_kick: None,
            scoring: Scoring::default(),
            lines: 0,
            over: None,
        };
        game.spawn(&mut Vec::new());
        game
    }

    /// 推进一个逻辑帧：先依次执行动作，再结算重力和锁定。
    ///
    /// `dt` 由调用方提供，core 不读系统时钟，所以结果只取决于种子、动作序列和 `dt`。
    /// 游戏结束后调用不再有任何效果。
    pub fn update(&mut self, dt: Duration, actions: &[Action]) -> Vec<Event> {
        let mut events = Vec::new();
        for &action in actions {
            if self.over.is_some() {
                break;
            }
            self.apply(action, &mut events);
        }
        if self.over.is_none() {
            self.tick(dt, &mut events);
        }
        events
    }

    pub fn view(&self) -> GameView<'_> {
        GameView {
            board: &self.board,
            active: self.active,
            ghost: self.active.map(|p| self.drop_position(p)),
            next: &self.queue,
            hold: self.hold,
            can_hold: self.can_hold,
            score: self.scoring.score,
            level: self.level(),
            lines: self.lines,
            game_over: self.over,
        }
    }

    fn level(&self) -> u32 {
        scoring::level(self.rules.start_level, self.lines)
    }

    fn apply(&mut self, action: Action, events: &mut Vec<Event>) {
        let Some(piece) = self.active else { return };
        let moved = match action {
            Action::MoveLeft => Some((piece.shifted(-1, 0), None)),
            Action::MoveRight => Some((piece.shifted(1, 0), None)),
            Action::RotateCw => rotation::rotate(&self.board, piece, piece.rotation.cw())
                .map(|(p, kick)| (p, Some(kick))),
            Action::RotateCcw => rotation::rotate(&self.board, piece, piece.rotation.ccw())
                .map(|(p, kick)| (p, Some(kick))),
            Action::SoftDrop => {
                let down = piece.shifted(0, -1);
                if self.fits(&down) {
                    self.fell(down);
                    self.scoring.soft_drop(1);
                    self.gravity_timer = Duration::ZERO;
                }
                return;
            }
            Action::HardDrop => {
                let dropped = self.drop_position(piece);
                self.scoring
                    .hard_drop((piece.origin.y - dropped.origin.y) as u32);
                self.fell(dropped);
                self.lock(events);
                return;
            }
            Action::Hold => {
                self.hold(events);
                return;
            }
        };
        if let Some((after, kick)) = moved.filter(|(p, _)| self.fits(p)) {
            self.player_moved(piece, after);
            self.last_kick = kick;
        }
    }

    fn tick(&mut self, dt: Duration, events: &mut Vec<Event>) {
        let Some(mut piece) = self.active else { return };
        if self.grounded(piece) {
            self.gravity_timer = Duration::ZERO;
            self.lock_timer += dt;
            if self.lock_timer >= self.rules.lock_delay {
                self.lock(events);
            }
            return;
        }
        self.lock_timer = Duration::ZERO;
        self.gravity_timer += dt;
        let gravity = scoring::gravity(self.level());
        while self.gravity_timer >= gravity {
            self.gravity_timer -= gravity;
            piece = piece.shifted(0, -1);
            self.fell(piece);
            if self.grounded(piece) {
                self.gravity_timer = Duration::ZERO;
                break;
            }
        }
    }

    /// 玩家移动或旋转成功后调用。方块原本已落地时会重置锁定延迟，但有次数上限。
    fn player_moved(&mut self, before: Piece, after: Piece) {
        let was_grounded = self.grounded(before);
        self.active = Some(after);
        if self.reached_new_low(after) {
            return;
        }
        if was_grounded && self.lock_resets < self.rules.max_lock_resets {
            self.lock_resets += 1;
            self.lock_timer = Duration::ZERO;
        }
    }

    /// 方块因重力、软降或硬降下落后调用。
    fn fell(&mut self, piece: Piece) {
        if self.active != Some(piece) {
            self.last_kick = None;
        }
        self.active = Some(piece);
        self.reached_new_low(piece);
    }

    /// 方块到达新的最低行时，重置次数和锁定计时都清零。
    fn reached_new_low(&mut self, piece: Piece) -> bool {
        let bottom = piece.bottom();
        if bottom >= self.lowest_y {
            return false;
        }
        self.lowest_y = bottom;
        self.lock_resets = 0;
        self.lock_timer = Duration::ZERO;
        true
    }

    fn hold(&mut self, events: &mut Vec<Event>) {
        let Some(piece) = self.active else { return };
        if !self.can_hold {
            return;
        }
        self.can_hold = false;
        match self.hold.replace(piece.kind) {
            Some(kind) => self.spawn_kind(kind, events),
            None => self.spawn(events),
        }
    }

    fn lock(&mut self, events: &mut Vec<Event>) {
        let Some(piece) = self.active.take() else {
            return;
        };
        let t_spin = self.t_spin(piece);
        self.board.fill(piece.cells(), piece.kind);
        events.push(Event::Locked(piece.kind));
        if piece.bottom() >= VISIBLE_HEIGHT {
            return self.end(GameOverReason::LockOut, events);
        }

        let lines = self.board.clear_full_rows();
        if let Some(clear) = self.scoring.lock(lines, t_spin, self.level()) {
            events.push(Event::Clear(clear));
        }
        self.lines += u32::from(lines);
        self.can_hold = true;
        self.spawn(events);
    }

    /// 三角判定：T 的最后一次操作是旋转，且中心四个斜角中至少三个被占（墙和地板也算）。
    /// 朝向一侧的两个角都被占才算完整 T-Spin，否则是 Mini；
    /// 但用第 5 个踢墙偏移转进去的一律算完整 T-Spin。
    fn t_spin(&self, piece: Piece) -> Option<TSpin> {
        let kick = self.last_kick?;
        if piece.kind != PieceKind::T {
            return None;
        }
        let center = piece.origin + Pos::new(1, 1);
        let occupied = |(dx, dy): (i32, i32)| !self.board.is_free(center + Pos::new(dx, dy));

        let corners = [(-1, 1), (1, 1), (1, -1), (-1, -1)];
        if corners.into_iter().filter(|&c| occupied(c)).count() < 3 {
            return None;
        }
        let front = match piece.rotation {
            Rotation::Spawn => [(-1, 1), (1, 1)],
            Rotation::Right => [(1, 1), (1, -1)],
            Rotation::Reverse => [(1, -1), (-1, -1)],
            Rotation::Left => [(-1, -1), (-1, 1)],
        };
        if front.into_iter().all(occupied) || kick == rotation::LAST_KICK {
            Some(TSpin::Full)
        } else {
            Some(TSpin::Mini)
        }
    }

    /// 从预览队列取下一个方块出生。
    fn spawn(&mut self, events: &mut Vec<Event>) {
        self.queue.push(self.bag.next_piece());
        let kind = self.queue.remove(0);
        self.spawn_kind(kind, events);
    }

    fn spawn_kind(&mut self, kind: PieceKind, events: &mut Vec<Event>) {
        let piece = spawn_piece(kind);
        if !self.fits(&piece) {
            return self.end(GameOverReason::BlockOut, events);
        }
        self.gravity_timer = Duration::ZERO;
        self.lock_timer = Duration::ZERO;
        self.lock_resets = 0;
        self.lowest_y = piece.bottom();
        self.active = Some(piece);
        self.last_kick = None;
        // Guideline：出生后如果下方有空间，立即下落一行
        let down = piece.shifted(0, -1);
        if self.fits(&down) {
            self.fell(down);
        }
    }

    fn end(&mut self, reason: GameOverReason, events: &mut Vec<Event>) {
        self.active = None;
        self.over = Some(reason);
        events.push(Event::GameOver(reason));
    }

    fn fits(&self, piece: &Piece) -> bool {
        self.board.fits(piece.cells())
    }

    fn grounded(&self, piece: Piece) -> bool {
        !self.fits(&piece.shifted(0, -1))
    }

    fn drop_position(&self, mut piece: Piece) -> Piece {
        while !self.grounded(piece) {
            piece = piece.shifted(0, -1);
        }
        piece
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn piece(kind: PieceKind, rotation: Rotation, x: i32, y: i32) -> Piece {
        Piece {
            kind,
            rotation,
            origin: Pos::new(x, y),
        }
    }

    fn clear(lines: u8, t_spin: Option<TSpin>, points: u32) -> Event {
        Event::Clear(Clear {
            lines,
            t_spin,
            back_to_back: false,
            combo: 0,
            points,
        })
    }

    /// 在指定盘面上放置指定的当前方块，绕开随机器。
    fn game_with(board: &str, active: Piece) -> Game {
        let mut game = Game::new(0, Rules::default());
        game.board = Board::from_ascii(board);
        game.active = Some(active);
        game.lowest_y = active.bottom();
        game.lock_resets = 0;
        game.lock_timer = Duration::ZERO;
        game.gravity_timer = Duration::ZERO;
        game
    }

    fn active(game: &Game) -> Piece {
        game.active.expect("游戏不应结束")
    }

    /// 平放在地板上的 T。
    fn t_on_floor() -> Piece {
        piece(PieceKind::T, Rotation::Spawn, 3, -1)
    }

    #[test]
    fn spawn_positions_follow_guideline() {
        let cells = |kind| {
            let mut c = spawn_piece(kind).cells().map(|p| (p.x, p.y));
            c.sort();
            c
        };
        assert_eq!(cells(PieceKind::I), [(3, 20), (4, 20), (5, 20), (6, 20)]);
        assert_eq!(cells(PieceKind::O), [(4, 20), (4, 21), (5, 20), (5, 21)]);
        assert_eq!(cells(PieceKind::T), [(3, 20), (4, 20), (4, 21), (5, 20)]);
    }

    #[test]
    fn new_piece_drops_into_top_visible_row() {
        for seed in 0..20 {
            let game = Game::new(seed, Rules::default());
            assert_eq!(active(&game).bottom(), VISIBLE_HEIGHT - 1);
        }
    }

    #[test]
    fn next_queue_feeds_active_piece() {
        let mut game = Game::new(3, Rules::default());
        for _ in 0..10 {
            let upcoming = game.view().next[0];
            game.update(Duration::ZERO, &[Action::HardDrop]);
            assert_eq!(active(&game).kind, upcoming);
            assert_eq!(game.view().next.len(), 5);
        }
    }

    #[test]
    fn gravity_drops_one_row_per_interval() {
        let mut game = Game::new(0, Rules::default());
        let y = active(&game).origin.y;
        game.update(ms(999), &[]);
        assert_eq!(active(&game).origin.y, y);
        game.update(ms(1), &[]);
        assert_eq!(active(&game).origin.y, y - 1);
        game.update(ms(3000), &[]);
        assert_eq!(active(&game).origin.y, y - 4);
    }

    #[test]
    fn soft_drop_moves_down_and_restarts_gravity() {
        let mut game = Game::new(0, Rules::default());
        let y = active(&game).origin.y;
        game.update(ms(900), &[]);
        game.update(Duration::ZERO, &[Action::SoftDrop]);
        assert_eq!(active(&game).origin.y, y - 1);
        game.update(ms(900), &[]);
        assert_eq!(active(&game).origin.y, y - 1);
        game.update(ms(100), &[]);
        assert_eq!(active(&game).origin.y, y - 2);
    }

    #[test]
    fn moves_are_blocked_by_walls() {
        let mut game = game_with("", piece(PieceKind::T, Rotation::Spawn, 0, 10));
        game.update(Duration::ZERO, &[Action::MoveLeft]);
        assert_eq!(active(&game).origin, Pos::new(0, 10));
        game.update(Duration::ZERO, &[Action::MoveRight; 20]);
        assert_eq!(active(&game).origin, Pos::new(7, 10));
    }

    #[test]
    fn locks_after_lock_delay() {
        let mut game = game_with("", t_on_floor());
        assert!(game.update(ms(499), &[]).is_empty());
        assert_eq!(game.update(ms(1), &[]), [Event::Locked(PieceKind::T)]);
    }

    #[test]
    fn soft_drop_on_floor_does_not_lock() {
        let mut game = game_with("", t_on_floor());
        assert!(game.update(Duration::ZERO, &[Action::SoftDrop]).is_empty());
        assert_eq!(active(&game), t_on_floor());
    }

    #[test]
    fn moving_on_floor_resets_lock_delay() {
        let mut game = game_with("", t_on_floor());
        game.update(ms(400), &[]);
        game.update(Duration::ZERO, &[Action::MoveLeft]);
        assert!(game.update(ms(499), &[]).is_empty());
        assert_eq!(game.update(ms(1), &[]), [Event::Locked(PieceKind::T)]);
    }

    #[test]
    fn lock_resets_are_capped() {
        let mut game = game_with("", t_on_floor());
        for i in 0..15 {
            let step = if i % 2 == 0 {
                Action::MoveLeft
            } else {
                Action::MoveRight
            };
            assert!(game.update(ms(100), &[step]).is_empty());
        }
        // 第 16 次移动不再重置，计时从 100ms 继续累计
        game.update(Duration::ZERO, &[Action::MoveLeft]);
        assert!(game.update(ms(399), &[]).is_empty());
        assert_eq!(game.update(ms(1), &[]), [Event::Locked(PieceKind::T)]);
    }

    #[test]
    fn falling_to_a_new_low_restores_lock_resets() {
        // 在台阶上耗尽重置次数，然后移出台阶掉到更低处，重置次数应恢复
        let mut game = game_with(
            "
            ZZZZZ.....
            ",
            piece(PieceKind::O, Rotation::Spawn, 0, 1),
        );
        for i in 0..15 {
            let step = if i % 2 == 0 {
                Action::MoveRight
            } else {
                Action::MoveLeft
            };
            game.update(ms(10), &[step]);
        }
        assert_eq!(game.lock_resets, 15);
        game.update(Duration::ZERO, &[Action::MoveRight; 4]);
        game.update(ms(1000), &[]);
        assert_eq!(active(&game).origin, Pos::new(5, 0));
        assert_eq!(game.lock_resets, 0);
    }

    #[test]
    fn hard_drop_locks_and_clears_line() {
        let mut game = game_with(
            "
            IIII....II
            ",
            piece(PieceKind::I, Rotation::Spawn, 4, 10),
        );
        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(events, [Event::Locked(PieceKind::I), clear(1, None, 100)]);
        assert_eq!(game.board, Board::new());
        assert_eq!(game.lines, 1);
    }

    #[test]
    fn vertical_i_clears_four_lines() {
        let mut game = game_with(
            "
            .JJJJJJJJ.
            IIIIIIIII.
            IIIIIIIII.
            IIIIIIIII.
            IIIIIIIII.
            ",
            piece(PieceKind::I, Rotation::Right, 7, 10),
        );
        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(events, [Event::Locked(PieceKind::I), clear(4, None, 800)]);
        assert_eq!(game.board, Board::from_ascii(".JJJJJJJJ."));
    }

    #[test]
    fn t_spin_triple_uses_fifth_kick() {
        // 前 4 个偏移都被挡住，只有第 5 个偏移 (-1, -2) 能把 T 塞进槽里
        let mut game = game_with(
            "
            ZZZZ.ZZZZZ
            ZZZ...ZZZZ
            ZZZ.ZZZZZZ
            ZZZ..ZZZZZ
            ZZZ.ZZZZZZ
            ",
            piece(PieceKind::T, Rotation::Spawn, 3, 2),
        );
        game.update(Duration::ZERO, &[Action::RotateCw]);
        assert_eq!(active(&game), piece(PieceKind::T, Rotation::Right, 2, 0));

        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(
            events,
            [
                Event::Locked(PieceKind::T),
                clear(3, Some(TSpin::Full), 1600)
            ]
        );
        assert_eq!(
            game.board,
            Board::from_ascii(
                "
                ZZZZ.ZZZZZ
                ZZZ...ZZZZ
                "
            )
        );
    }

    /// T 贴左墙、朝右，左侧两个角是墙，右下角是方块，右上角空着。
    const MINI_SLOT: &str = ".ZZZZZZZZZ";

    #[test]
    fn t_spin_mini_when_one_front_corner_is_open() {
        // 原地旋转被挡，第 2 个偏移 (-1, 0) 把 T 推到墙边
        let mut game = game_with(MINI_SLOT, piece(PieceKind::T, Rotation::Spawn, 0, 0));
        game.update(Duration::ZERO, &[Action::RotateCw]);
        assert_eq!(active(&game), piece(PieceKind::T, Rotation::Right, -1, 0));

        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(
            events,
            [
                Event::Locked(PieceKind::T),
                clear(1, Some(TSpin::Mini), 200)
            ]
        );
    }

    #[test]
    fn t_spin_requires_rotation_as_last_move() {
        // 同样的位置，但直接落进去而不是转进去
        let mut game = game_with(MINI_SLOT, piece(PieceKind::T, Rotation::Right, -1, 5));
        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(events, [Event::Locked(PieceKind::T), clear(1, None, 100)]);
    }

    #[test]
    fn drops_add_score() {
        let mut game = game_with("", piece(PieceKind::T, Rotation::Spawn, 3, 10));
        game.update(Duration::ZERO, &[Action::SoftDrop, Action::SoftDrop]);
        assert_eq!(game.view().score, 2);
        // 从 y = 8 硬降到 y = -1，共 9 行
        game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(game.view().score, 2 + 18);
    }

    #[test]
    fn level_rises_every_ten_lines_and_speeds_up_gravity() {
        let mut game = game_with("IIII....II", piece(PieceKind::I, Rotation::Spawn, 4, 10));
        game.lines = 9;
        game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(game.view().level, 2);

        let y = active(&game).origin.y;
        game.update(ms(792), &[]);
        assert_eq!(active(&game).origin.y, y);
        game.update(ms(1), &[]);
        assert_eq!(active(&game).origin.y, y - 1);
    }

    #[test]
    fn hold_swaps_once_per_piece() {
        let mut game = Game::new(5, Rules::default());
        let first = active(&game).kind;
        let second = game.view().next[0];

        game.update(Duration::ZERO, &[Action::Hold]);
        assert_eq!(active(&game).kind, second);
        assert_eq!(game.view().hold, Some(first));
        assert!(!game.view().can_hold);

        // 锁定前再按无效
        game.update(Duration::ZERO, &[Action::Hold]);
        assert_eq!(active(&game).kind, second);

        game.update(Duration::ZERO, &[Action::HardDrop]);
        assert!(game.view().can_hold);
        let third = active(&game).kind;
        game.update(Duration::ZERO, &[Action::Hold]);
        assert_eq!(active(&game).kind, first);
        assert_eq!(game.view().hold, Some(third));
    }

    #[test]
    fn held_piece_returns_at_spawn_position() {
        let mut game = Game::new(5, Rules::default());
        game.update(
            Duration::ZERO,
            &[Action::RotateCw, Action::MoveLeft, Action::Hold],
        );
        game.update(Duration::ZERO, &[Action::HardDrop, Action::Hold]);
        let returned = active(&game);
        assert_eq!(returned.rotation, Rotation::Spawn);
        assert_eq!(returned.bottom(), VISIBLE_HEIGHT - 1);
    }

    #[test]
    fn block_out_ends_game() {
        let mut rows = vec![".........."; 22];
        rows[0] = ".ZZZZZZZZZ"; // y = 21
        rows[1] = ".ZZZZZZZZZ"; // y = 20
        let mut game = game_with(&rows.join("\n"), piece(PieceKind::O, Rotation::Spawn, 4, 0));

        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(
            events,
            [
                Event::Locked(PieceKind::O),
                Event::GameOver(GameOverReason::BlockOut)
            ]
        );
        assert_eq!(game.view().active, None);
        assert!(game.update(ms(5000), &[Action::HardDrop]).is_empty());
    }

    #[test]
    fn lock_out_ends_game() {
        let stack = vec![".ZZZZZZZZZ"; VISIBLE_HEIGHT as usize].join("\n");
        let mut game = game_with(&stack, piece(PieceKind::O, Rotation::Spawn, 4, 25));

        let events = game.update(Duration::ZERO, &[Action::HardDrop]);
        assert_eq!(
            events,
            [
                Event::Locked(PieceKind::O),
                Event::GameOver(GameOverReason::LockOut)
            ]
        );
        assert_eq!(game.view().game_over, Some(GameOverReason::LockOut));
    }

    #[test]
    fn ghost_shows_hard_drop_position() {
        let game = game_with("", piece(PieceKind::T, Rotation::Spawn, 3, 10));
        assert_eq!(game.view().ghost, Some(t_on_floor()));
    }
}
