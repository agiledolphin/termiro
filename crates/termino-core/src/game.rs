use std::time::Duration;

use crate::board::{Board, VISIBLE_HEIGHT};
use crate::piece::{Piece, PieceKind, Pos, Rotation};
use crate::randomizer::SevenBag;
use crate::rotation;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// 方块锁定到盘面。
    Locked(PieceKind),
    LinesCleared(u8),
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
    /// 重力：自然下落一行所需的时间。M3 引入等级后改由等级决定。
    pub gravity: Duration,
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
            gravity: Duration::from_secs(1),
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
    gravity_timer: Duration,
    /// 落地后累计的时间，离开地面时清零。
    lock_timer: Duration,
    lock_resets: u32,
    /// 当前方块到过的最低行。到达新低点时 `lock_resets` 清零。
    lowest_y: i32,
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
            gravity_timer: Duration::ZERO,
            lock_timer: Duration::ZERO,
            lock_resets: 0,
            lowest_y: 0,
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
            lines: self.lines,
            game_over: self.over,
        }
    }

    fn apply(&mut self, action: Action, events: &mut Vec<Event>) {
        let Some(piece) = self.active else { return };
        let moved = match action {
            Action::MoveLeft => Some(piece.shifted(-1, 0)).filter(|p| self.fits(p)),
            Action::MoveRight => Some(piece.shifted(1, 0)).filter(|p| self.fits(p)),
            Action::RotateCw => rotation::rotate(&self.board, piece, piece.rotation.cw()),
            Action::RotateCcw => rotation::rotate(&self.board, piece, piece.rotation.ccw()),
            Action::SoftDrop => {
                let down = piece.shifted(0, -1);
                if self.fits(&down) {
                    self.fell(down);
                    self.gravity_timer = Duration::ZERO;
                }
                return;
            }
            Action::HardDrop => {
                self.fell(self.drop_position(piece));
                self.lock(events);
                return;
            }
        };
        if let Some(after) = moved {
            self.player_moved(piece, after);
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
        while self.gravity_timer >= self.rules.gravity {
            self.gravity_timer -= self.rules.gravity;
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

    fn lock(&mut self, events: &mut Vec<Event>) {
        let Some(piece) = self.active.take() else {
            return;
        };
        self.board.fill(piece.cells(), piece.kind);
        events.push(Event::Locked(piece.kind));
        if piece.bottom() >= VISIBLE_HEIGHT {
            return self.end(GameOverReason::LockOut, events);
        }
        let cleared = self.board.clear_full_rows();
        if cleared > 0 {
            self.lines += u32::from(cleared);
            events.push(Event::LinesCleared(cleared));
        }
        self.spawn(events);
    }

    fn spawn(&mut self, events: &mut Vec<Event>) {
        self.queue.push(self.bag.next_piece());
        let piece = spawn_piece(self.queue.remove(0));
        if !self.fits(&piece) {
            return self.end(GameOverReason::BlockOut, events);
        }
        self.gravity_timer = Duration::ZERO;
        self.lock_timer = Duration::ZERO;
        self.lock_resets = 0;
        self.lowest_y = piece.bottom();
        // Guideline：出生后如果下方有空间，立即下落一行
        let down = piece.shifted(0, -1);
        self.fell(if self.fits(&down) { down } else { piece });
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
        assert_eq!(
            events,
            [Event::Locked(PieceKind::I), Event::LinesCleared(1)]
        );
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
        assert_eq!(
            events,
            [Event::Locked(PieceKind::I), Event::LinesCleared(4)]
        );
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
            [Event::Locked(PieceKind::T), Event::LinesCleared(3)]
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
