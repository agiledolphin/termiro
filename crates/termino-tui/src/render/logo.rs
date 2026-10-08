//! 开始界面的 TERMINO 标志：用画方块的方式画字母，每个字母一种方块颜色。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use termino_core::PieceKind;

use super::board::CELL_WIDTH;
use super::theme::Theme;

/// 5 行高的像素字，`#` 是一个格子。
const LETTERS: [(PieceKind, [&str; 5]); 7] = [
    (PieceKind::T, ["###", ".#.", ".#.", ".#.", ".#."]),
    (PieceKind::Z, ["###", "#..", "##.", "#..", "###"]),
    (PieceKind::L, ["##.", "#.#", "##.", "#.#", "#.#"]),
    (PieceKind::O, ["#...#", "##.##", "#.#.#", "#...#", "#...#"]),
    (PieceKind::I, ["###", ".#.", ".#.", ".#.", "###"]),
    (PieceKind::J, ["#..#", "##.#", "#.##", "#..#", "#..#"]),
    (PieceKind::S, ["###", "#.#", "#.#", "#.#", "###"]),
];
const NAME: &str = "TERMINO";

/// 字母之间空一格。
const GAP: u16 = 1;
pub const HEIGHT: u16 = 5;
pub const WIDTH: u16 = {
    let mut cells = 0;
    let mut i = 0;
    while i < LETTERS.len() {
        cells += LETTERS[i].1[0].len() as u16 + GAP;
        i += 1;
    }
    (cells - GAP) * CELL_WIDTH
};

/// 大号标志，需要 [`WIDTH`]×[`HEIGHT`] 的区域。
pub struct Logo<'a> {
    pub theme: &'a Theme,
}

impl Widget for Logo<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        for (kind, rows) in LETTERS {
            let (symbol, style) = self.theme.block(kind);
            for (dy, row) in rows.iter().enumerate() {
                for (dx, pixel) in row.chars().enumerate() {
                    let (px, py) = (x + dx as u16 * CELL_WIDTH, area.y + dy as u16);
                    if pixel == '#' && px < area.right() && py < area.bottom() {
                        buf.set_stringn(px, py, symbol, CELL_WIDTH as usize, style);
                    }
                }
            }
            x += (rows[0].len() as u16 + GAP) * CELL_WIDTH;
        }
    }
}

/// 窄终端用的小号标志：`T E R M I N O`，每个字母一种颜色。
pub fn small(theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (letter, (kind, _))) in NAME.chars().zip(LETTERS).enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = theme
            .color(kind)
            .map_or(Style::new(), |c| Style::new().fg(c))
            .bold();
        spans.push(Span::styled(letter.to_string(), style));
    }
    Line::from(spans)
}
