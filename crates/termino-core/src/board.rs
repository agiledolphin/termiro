use std::fmt;

use crate::piece::{PieceKind, Pos};

pub const WIDTH: i32 = 10;
/// 总高度：可见的 20 行加上方 20 行缓冲区（Tetris Guideline 规定）。
pub const HEIGHT: i32 = 40;
pub const VISIBLE_HEIGHT: i32 = 20;

type Row = [Option<PieceKind>; WIDTH as usize];
const EMPTY_ROW: Row = [None; WIDTH as usize];

/// 已锁定方块组成的盘面。
#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    /// `rows[0]` 是最底行。
    rows: [Row; HEIGHT as usize],
}

impl Default for Board {
    fn default() -> Self {
        Self {
            rows: [EMPTY_ROW; HEIGHT as usize],
        }
    }
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从文本构造盘面，供测试和残局模式使用，格式与 `Display` 输出相同。
    ///
    /// 每行文本对应盘面一行，最后一行是 y = 0；`.` 表示空，`IOTSZJL` 表示对应方块。
    /// 空行和首尾空白会被忽略。
    ///
    /// # Panics
    ///
    /// 行宽不是 [`WIDTH`]、行数超过 [`HEIGHT`] 或出现其它字符时 panic。
    pub fn from_ascii(text: &str) -> Self {
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        assert!(lines.len() <= HEIGHT as usize, "行数超过盘面高度 {HEIGHT}");

        let mut board = Self::new();
        for (y, line) in lines.iter().rev().enumerate() {
            assert_eq!(
                line.chars().count(),
                WIDTH as usize,
                "第 {y} 行宽度应为 {WIDTH}: {line:?}"
            );
            for (x, ch) in line.chars().enumerate() {
                board.rows[y][x] = match ch {
                    '.' => None,
                    _ => Some(
                        PieceKind::from_letter(ch).unwrap_or_else(|| panic!("未知字符 {ch:?}")),
                    ),
                };
            }
        }
        board
    }

    fn in_bounds(pos: Pos) -> bool {
        (0..WIDTH).contains(&pos.x) && (0..HEIGHT).contains(&pos.y)
    }

    /// 某格的方块；空格或越界时为 `None`。
    pub fn get(&self, pos: Pos) -> Option<PieceKind> {
        if Self::in_bounds(pos) {
            self.rows[pos.y as usize][pos.x as usize]
        } else {
            None
        }
    }

    /// 该格在盘面内且为空。
    pub fn is_free(&self, pos: Pos) -> bool {
        Self::in_bounds(pos) && self.rows[pos.y as usize][pos.x as usize].is_none()
    }

    /// 这些格子是否都能放下。
    pub fn fits(&self, cells: impl IntoIterator<Item = Pos>) -> bool {
        cells.into_iter().all(|c| self.is_free(c))
    }

    /// 已占用的格子总数。
    pub fn filled_count(&self) -> usize {
        self.rows.iter().flatten().filter(|c| c.is_some()).count()
    }

    pub(crate) fn fill(&mut self, cells: impl IntoIterator<Item = Pos>, kind: PieceKind) {
        for c in cells {
            debug_assert!(self.is_free(c), "{c:?} 已被占用或越界");
            self.rows[c.y as usize][c.x as usize] = Some(kind);
        }
    }

    /// 消除所有满行，上方的行依次下移。返回消除的行数。
    pub(crate) fn clear_full_rows(&mut self) -> u8 {
        let mut kept = 0;
        for y in 0..HEIGHT as usize {
            if self.rows[y].iter().all(Option::is_some) {
                continue;
            }
            self.rows[kept] = self.rows[y];
            kept += 1;
        }
        self.rows[kept..].fill(EMPTY_ROW);
        (HEIGHT as usize - kept) as u8
    }
}

impl fmt::Display for Board {
    /// 输出可见区域；缓冲区里有方块时一并输出到最高的那一行。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let highest = self
            .rows
            .iter()
            .rposition(|row| row.iter().any(Option::is_some))
            .map_or(0, |y| y as i32 + 1);
        let top = highest.max(VISIBLE_HEIGHT);
        for y in (0..top).rev() {
            for cell in &self.rows[y as usize] {
                write!(f, "{}", cell.map_or('.', PieceKind::letter))?;
            }
            if y > 0 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n{self}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_round_trip() {
        let board = Board::from_ascii(
            "
            T.........
            IIII..JJJ.
            ",
        );
        assert_eq!(board.get(Pos::new(0, 1)), Some(PieceKind::T));
        assert_eq!(board.get(Pos::new(6, 0)), Some(PieceKind::J));
        assert_eq!(board.get(Pos::new(4, 0)), None);
        assert_eq!(Board::from_ascii(&board.to_string()), board);
    }

    #[test]
    fn walls_floor_and_ceiling_are_not_free() {
        let board = Board::new();
        assert!(board.is_free(Pos::new(0, 0)));
        assert!(board.is_free(Pos::new(WIDTH - 1, HEIGHT - 1)));
        assert!(!board.is_free(Pos::new(-1, 0)));
        assert!(!board.is_free(Pos::new(WIDTH, 0)));
        assert!(!board.is_free(Pos::new(0, -1)));
        assert!(!board.is_free(Pos::new(0, HEIGHT)));
    }

    #[test]
    fn clears_non_adjacent_full_rows() {
        let mut board = Board::from_ascii(
            "
            T.........
            IIIIIIIIII
            J.J.......
            OOOOOOOOOO
            ",
        );
        assert_eq!(board.clear_full_rows(), 2);
        assert_eq!(
            board,
            Board::from_ascii(
                "
                T.........
                J.J.......
                "
            )
        );
    }
}
