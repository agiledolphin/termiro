//! 盘面两侧的面板：暂存、预览、分数和消行提示。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};
use termino_core::{Clear, GameView, PieceKind, Rotation, TSpin};

use super::board::{CELL_WIDTH, block_style};

pub const WIDTH: u16 = 14;
/// 暂存框高度：方块占 2 行，加上边框。
pub const HOLD_HEIGHT: u16 = 4;
/// 预览框高度：每个方块 2 行，之间空 1 行，加上边框。
pub fn next_height(count: usize) -> u16 {
    (count as u16 * 3).saturating_sub(1) + 2
}

pub fn hold(view: &GameView, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(" HOLD ");
    let inner = block.inner(area);
    block.render(area, buf);
    if let Some(kind) = view.hold {
        // 本回合已经用过暂存时画成灰色
        let style = if view.can_hold {
            block_style(kind)
        } else {
            Style::new().bg(Color::DarkGray)
        };
        draw_piece(buf, inner, kind, style);
    }
}

pub fn next(view: &GameView, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(" NEXT ");
    let inner = block.inner(area);
    block.render(area, buf);
    for (i, &kind) in view.next.iter().enumerate() {
        let slot = Rect {
            y: inner.y + i as u16 * 3,
            height: 2,
            ..inner
        };
        if slot.bottom() > inner.bottom() {
            break;
        }
        draw_piece(buf, slot, kind, block_style(kind));
    }
}

/// 分数、等级、行数，以及最近一次消行的提示。
pub fn stats(view: &GameView, banner: Option<Clear>) -> Paragraph<'static> {
    let mut lines = vec![
        Line::from("SCORE".bold()),
        Line::from(view.score.to_string()),
        Line::from(""),
        Line::from(vec!["LEVEL ".bold(), view.level.to_string().into()]),
        Line::from(vec!["LINES ".bold(), view.lines.to_string().into()]),
        Line::from(""),
    ];
    if let Some(clear) = banner {
        lines.extend(describe(clear));
    }
    Paragraph::new(lines)
}

/// 把一次消行写成几行短文字，例如 `T-SPIN` / `DOUBLE` / `BACK-TO-BACK` / `+1800`。
fn describe(clear: Clear) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match clear.t_spin {
        Some(TSpin::Full) => lines.push(Line::from("T-SPIN".bold().magenta())),
        Some(TSpin::Mini) => lines.push(Line::from("T-SPIN MINI".bold().magenta())),
        None => {}
    }
    let name = match clear.lines {
        0 => None,
        1 => Some("SINGLE"),
        2 => Some("DOUBLE"),
        3 => Some("TRIPLE"),
        _ => Some("TETRIS"),
    };
    if let Some(name) = name {
        let span = if clear.lines >= 4 {
            name.bold().cyan()
        } else {
            name.bold()
        };
        lines.push(Line::from(span));
    }
    if clear.back_to_back {
        lines.push(Line::from("BACK-TO-BACK".bold().yellow()));
    }
    if clear.combo > 0 {
        lines.push(Line::from(format!("COMBO {}", clear.combo).bold()));
    }
    lines.push(Line::from(format!("+{}", clear.points).dim()));
    lines
}

/// 在 `area` 里居中画一个出生朝向的方块。
fn draw_piece(buf: &mut Buffer, area: Rect, kind: PieceKind, style: Style) {
    let cells = kind.cells(Rotation::Spawn);
    let min_x = cells.iter().map(|c| c.x).min().unwrap();
    let max_x = cells.iter().map(|c| c.x).max().unwrap();
    let max_y = cells.iter().map(|c| c.y).max().unwrap();
    let width = (max_x - min_x + 1) as u16 * CELL_WIDTH;
    let left = area.x + area.width.saturating_sub(width) / 2;
    for c in cells {
        let x = left + (c.x - min_x) as u16 * CELL_WIDTH;
        let y = area.y + (max_y - c.y) as u16;
        if y < area.bottom() {
            buf.set_stringn(x, y, "  ", CELL_WIDTH as usize, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(clear: Clear) -> Vec<String> {
        describe(clear).iter().map(ToString::to_string).collect()
    }

    #[test]
    fn describes_t_spin_with_bonuses() {
        let clear = Clear {
            lines: 2,
            t_spin: Some(TSpin::Full),
            back_to_back: true,
            combo: 3,
            points: 1950,
        };
        assert_eq!(
            text(clear),
            ["T-SPIN", "DOUBLE", "BACK-TO-BACK", "COMBO 3", "+1950"]
        );
    }

    #[test]
    fn describes_t_spin_mini_without_lines() {
        let clear = Clear {
            lines: 0,
            t_spin: Some(TSpin::Mini),
            back_to_back: false,
            combo: 0,
            points: 100,
        };
        assert_eq!(text(clear), ["T-SPIN MINI", "+100"]);
    }
}
