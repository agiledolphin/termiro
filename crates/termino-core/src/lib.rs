//! Termino 的纯游戏逻辑。
//!
//! 本 crate 不做任何 IO、不读系统时钟：时间由调用方通过 [`Game::update`] 传入，
//! 随机数由种子决定。因此「种子 + 输入序列」可以完整复现一局游戏。
//!
//! 坐标系：x 向右、y 向上，`(0, 0)` 是盘面左下角。SRS 踢墙表同样采用 y 向上。

mod board;
mod game;
mod piece;
mod randomizer;
mod rotation;

pub use board::{Board, HEIGHT, VISIBLE_HEIGHT, WIDTH};
pub use game::{Action, Event, Game, GameOverReason, GameView, Rules};
pub use piece::{Piece, PieceKind, Pos, Rotation};
pub use randomizer::SevenBag;
