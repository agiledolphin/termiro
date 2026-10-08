//! SRS（Super Rotation System）踢墙。

use crate::board::Board;
use crate::piece::{Piece, PieceKind, Pos, Rotation};

const fn p(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

// 各行顺序与 `transition_index` 一致：0→R, R→0, R→2, 2→R, 2→L, L→2, L→0, 0→L。
// 数值取自 Tetris Guideline，y 向上为正。

const JLSTZ_KICKS: [[Pos; 5]; 8] = [
    [p(0, 0), p(-1, 0), p(-1, 1), p(0, -2), p(-1, -2)],
    [p(0, 0), p(1, 0), p(1, -1), p(0, 2), p(1, 2)],
    [p(0, 0), p(1, 0), p(1, -1), p(0, 2), p(1, 2)],
    [p(0, 0), p(-1, 0), p(-1, 1), p(0, -2), p(-1, -2)],
    [p(0, 0), p(1, 0), p(1, 1), p(0, -2), p(1, -2)],
    [p(0, 0), p(-1, 0), p(-1, -1), p(0, 2), p(-1, 2)],
    [p(0, 0), p(-1, 0), p(-1, -1), p(0, 2), p(-1, 2)],
    [p(0, 0), p(1, 0), p(1, 1), p(0, -2), p(1, -2)],
];

const I_KICKS: [[Pos; 5]; 8] = [
    [p(0, 0), p(-2, 0), p(1, 0), p(-2, -1), p(1, 2)],
    [p(0, 0), p(2, 0), p(-1, 0), p(2, 1), p(-1, -2)],
    [p(0, 0), p(-1, 0), p(2, 0), p(-1, 2), p(2, -1)],
    [p(0, 0), p(1, 0), p(-2, 0), p(1, -2), p(-2, 1)],
    [p(0, 0), p(2, 0), p(-1, 0), p(2, 1), p(-1, -2)],
    [p(0, 0), p(-2, 0), p(1, 0), p(-2, -1), p(1, 2)],
    [p(0, 0), p(1, 0), p(-2, 0), p(1, -2), p(-2, 1)],
    [p(0, 0), p(-1, 0), p(2, 0), p(-1, 2), p(2, -1)],
];

const NO_KICK: [Pos; 1] = [p(0, 0)];

fn transition_index(from: Rotation, to: Rotation) -> usize {
    use Rotation::*;
    match (from, to) {
        (Spawn, Right) => 0,
        (Right, Spawn) => 1,
        (Right, Reverse) => 2,
        (Reverse, Right) => 3,
        (Reverse, Left) => 4,
        (Left, Reverse) => 5,
        (Left, Spawn) => 6,
        (Spawn, Left) => 7,
        _ => unreachable!("SRS 只定义相邻朝向之间的旋转：{from:?} → {to:?}"),
    }
}

/// 一次旋转要依次尝试的偏移量。
fn kicks(kind: PieceKind, from: Rotation, to: Rotation) -> &'static [Pos] {
    match kind {
        PieceKind::O => &NO_KICK,
        PieceKind::I => &I_KICKS[transition_index(from, to)],
        _ => &JLSTZ_KICKS[transition_index(from, to)],
    }
}

/// 第 5 个偏移的序号。T 用它转进槽里时，T-Spin 判定从 Mini 升级为完整 T-Spin。
pub(crate) const LAST_KICK: usize = 4;

/// 按 SRS 把方块转到 `to` 朝向：依次尝试各偏移量，返回第一个放得下的位置及其偏移序号。
pub(crate) fn rotate(board: &Board, piece: Piece, to: Rotation) -> Option<(Piece, usize)> {
    kicks(piece.kind, piece.rotation, to)
        .iter()
        .map(|&kick| Piece {
            rotation: to,
            origin: piece.origin + kick,
            ..piece
        })
        .enumerate()
        .find(|(_, candidate)| board.fits(candidate.cells()))
        .map(|(index, candidate)| (candidate, index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{HEIGHT, WIDTH};

    const ROTATIONS: [Rotation; 4] = [
        Rotation::Spawn,
        Rotation::Right,
        Rotation::Reverse,
        Rotation::Left,
    ];

    fn piece(kind: PieceKind, rotation: Rotation, x: i32, y: i32) -> Piece {
        Piece {
            kind,
            rotation,
            origin: Pos::new(x, y),
        }
    }

    #[test]
    fn reverse_rotation_uses_negated_kicks() {
        for kind in [PieceKind::I, PieceKind::T] {
            for from in ROTATIONS {
                let to = from.cw();
                let forward = kicks(kind, from, to);
                let back = kicks(kind, to, from);
                for (f, b) in forward.iter().zip(back) {
                    assert_eq!((f.x, f.y), (-b.x, -b.y), "{kind:?} {from:?}→{to:?}");
                }
            }
        }
    }

    #[test]
    fn rotates_in_place_in_open_space() {
        let board = Board::new();
        for kind in PieceKind::ALL {
            for from in ROTATIONS {
                let start = piece(kind, from, 4, 10);
                let (rotated, kick) = rotate(&board, start, from.cw()).unwrap();
                assert_eq!(kick, 0);
                assert_eq!(rotated.origin, start.origin);
                assert_eq!(rotated.rotation, from.cw());
            }
        }
    }

    #[test]
    fn t_kicks_off_left_wall() {
        // T 朝右贴左墙，转回出生朝向时第 1 个偏移出界，第 2 个偏移 (+1, 0) 成功
        let start = piece(PieceKind::T, Rotation::Right, -1, 5);
        let (rotated, kick) = rotate(&Board::new(), start, Rotation::Spawn).unwrap();
        assert_eq!(kick, 1);
        assert_eq!(rotated.origin, Pos::new(0, 5));
    }

    #[test]
    fn i_kicks_off_right_wall() {
        // 竖直的 I 贴右墙，R→2 时第 2 个偏移 (-1, 0) 成功
        let start = piece(PieceKind::I, Rotation::Right, 7, 5);
        assert_eq!(start.cells().map(|c| c.x), [9; 4]);
        let (rotated, kick) = rotate(&Board::new(), start, Rotation::Reverse).unwrap();
        assert_eq!(kick, 1);
        assert_eq!(rotated.origin, Pos::new(6, 5));
    }

    #[test]
    fn fails_when_every_kick_is_blocked() {
        let start = piece(PieceKind::T, Rotation::Spawn, 3, 10);
        let holes = start.cells();
        let mut board = Board::new();
        let everything_else = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| Pos::new(x, y)))
            .filter(|p| !holes.contains(p));
        board.fill(everything_else, PieceKind::Z);
        assert_eq!(rotate(&board, start, Rotation::Right), None);
        assert_eq!(rotate(&board, start, Rotation::Left), None);
    }
}
