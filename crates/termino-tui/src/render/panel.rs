//! 盘面两侧的面板：暂存、预览、分数和消行提示。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use termino_core::{Clear, GameView, PieceKind, Rotation, TSpin};

use super::board::CELL_WIDTH;
use super::theme::{Cell, Theme};

pub const WIDTH: u16 = 14;
/// 暂存框高度：方块占 2 行，加上边框。
pub const HOLD_HEIGHT: u16 = 4;
/// 预览框高度：每个方块 2 行，之间空 1 行，加上边框。
pub fn next_height(count: usize) -> u16 {
    (count as u16 * 3).saturating_sub(1) + 2
}

pub fn hold(view: &GameView, theme: &Theme, area: Rect, buf: &mut Buffer) {
    let block = theme.bordered().title(" HOLD ");
    let inner = block.inner(area);
    block.render(area, buf);
    if let Some(kind) = view.hold {
        // 本回合已经用过暂存时画成灰色
        let cell = if view.can_hold {
            theme.block(kind)
        } else {
            theme.faded()
        };
        draw_piece(buf, inner, kind, cell);
    }
}

pub fn next(view: &GameView, theme: &Theme, area: Rect, buf: &mut Buffer) {
    let block = theme.bordered().title(" NEXT ");
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
        draw_piece(buf, slot, kind, theme.block(kind));
    }
}

/// 分数、最高分、等级、行数，以及最近一次消行的提示。
pub fn stats(
    view: &GameView,
    best: u64,
    banner: Option<Clear>,
    theme: &Theme,
) -> Paragraph<'static> {
    let mut lines = vec![
        Line::from("SCORE".bold()),
        Line::from(view.score.to_string()),
        Line::from("BEST".bold()),
        Line::from(best.to_string()),
        Line::from(""),
        Line::from(vec!["LEVEL ".bold(), view.level.to_string().into()]),
        Line::from(vec!["LINES ".bold(), view.lines.to_string().into()]),
        Line::from(""),
    ];
    if let Some(clear) = banner {
        lines.extend(describe(clear, theme));
    }
    Paragraph::new(lines)
}

/// 把一次消行写成几行短文字，例如 `T-SPIN` / `DOUBLE` / `BACK-TO-BACK` / `+1800`。
fn describe(clear: Clear, theme: &Theme) -> Vec<Line<'static>> {
    let accent =
        |text: &'static str, color: Color| Line::from(Span::styled(text, theme.fg(color)).bold());
    let mut lines = Vec::new();
    match clear.t_spin {
        Some(TSpin::Full) => lines.push(accent("T-SPIN", Color::Magenta)),
        Some(TSpin::Mini) => lines.push(accent("T-SPIN MINI", Color::Magenta)),
        None => {}
    }
    match clear.lines {
        0 => {}
        1 => lines.push(Line::from("SINGLE".bold())),
        2 => lines.push(Line::from("DOUBLE".bold())),
        3 => lines.push(Line::from("TRIPLE".bold())),
        _ => lines.push(accent("TETRIS", Color::Cyan)),
    }
    if clear.back_to_back {
        lines.push(accent("BACK-TO-BACK", Color::Yellow));
    }
    if clear.combo > 0 {
        lines.push(Line::from(format!("COMBO {}", clear.combo).bold()));
    }
    lines.push(Line::from(format!("+{}", clear.points).dim()));
    lines
}

/// 在 `area` 里居中画一个出生朝向的方块。
fn draw_piece(buf: &mut Buffer, area: Rect, kind: PieceKind, (symbol, style): Cell) {
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
            buf.set_stringn(x, y, symbol, CELL_WIDTH as usize, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ColorDepth;

    fn text(clear: Clear) -> Vec<String> {
        describe(clear, &Theme::new(ColorDepth::Ansi16, false))
            .iter()
            .map(ToString::to_string)
            .collect()
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
