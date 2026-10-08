//! 把 `App` 画到终端上。只读状态，不修改任何东西。
//!
//! 界面文字只用 ASCII：方向箭头、`·` 等字符在东亚语言环境下可能被当作双宽字符，导致错位。

mod board;
mod panel;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

use self::board::BoardWidget;
use crate::app::App;

const MIN_WIDTH: u16 = panel::WIDTH + 1 + board::WIDTH + 1 + panel::WIDTH;
const MIN_HEIGHT: u16 = board::HEIGHT;

const HELP: [(&str, &str); 10] = [
    ("Left", "move left"),
    ("Right", "move right"),
    ("Down", "soft drop"),
    ("Space", "hard drop"),
    ("Up X", "rotate"),
    ("Z", "rotate ccw"),
    ("C", "hold"),
    ("P", "resume"),
    ("R", "restart"),
    ("Q", "quit"),
];

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    let [left, _, board_area, _, right] = Layout::horizontal([
        Constraint::Length(panel::WIDTH),
        Constraint::Length(1),
        Constraint::Length(board::WIDTH),
        Constraint::Length(1),
        Constraint::Length(panel::WIDTH),
    ])
    .areas(centered(area, MIN_WIDTH, MIN_HEIGHT));
    let [hold_area, _, stats_area, hint_area] = Layout::vertical([
        Constraint::Length(panel::HOLD_HEIGHT),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(left);
    let [next_area, _] = Layout::vertical([
        Constraint::Length(panel::next_height(app.view().next.len())),
        Constraint::Min(0),
    ])
    .areas(right);

    let view = app.view();
    let buf = frame.buffer_mut();
    panel::hold(&view, hold_area, buf);
    panel::next(&view, next_area, buf);
    frame.render_widget(BoardWidget::new(&view), board_area);
    frame.render_widget(panel::stats(&view, app.banner()), stats_area);
    frame.render_widget(Line::from("P  pause/help".dim()), hint_area);

    if view.game_over.is_some() {
        let lines = vec![
            Line::from("GAME OVER".bold()),
            Line::from(""),
            Line::from(format!("SCORE {}", view.score)),
            Line::from(""),
            Line::from("R  restart".dim()),
            Line::from("Q  quit".dim()),
        ];
        draw_overlay(frame, board_area, lines.into_iter().map(Line::centered));
    } else if app.paused() {
        let mut lines = vec![Line::from("PAUSED".bold()).centered(), Line::from("")];
        lines.extend(
            HELP.iter()
                .map(|(key, what)| Line::from(vec![format!(" {key:<7}").bold(), what.dim()])),
        );
        draw_overlay(frame, board_area, lines);
    }
}

/// 盖在盘面中央的提示框，宽度与盘面内部一致。
fn draw_overlay<'a>(
    frame: &mut Frame,
    board_area: Rect,
    lines: impl IntoIterator<Item = Line<'a>>,
) {
    let lines: Vec<_> = lines.into_iter().collect();
    let area = centered(board_area, board::WIDTH - 2, lines.len() as u16 + 2);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(Block::bordered()), area);
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Terminal too small".bold()).centered(),
        Line::from(format!(
            "need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        ))
        .centered(),
    ];
    frame.render_widget(Paragraph::new(text), centered(area, area.width, 2));
}

/// `area` 中央一块 `width`×`height` 的区域，超出时截断到 `area` 大小。
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use termino_core::{Action, PieceKind};

    use super::*;
    use crate::input::Command;

    /// 把画面转成文本。纯文本快照看不到背景色，所以有背景色的格子
    /// 换成对应方块的字母（灰色为 `#`），其余格子保留原字符。
    fn render(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        to_text(terminal.backend().buffer())
    }

    fn to_text(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                let letter = PieceKind::ALL
                    .into_iter()
                    .find(|&k| board::color(k) == cell.bg)
                    .map(PieceKind::letter)
                    .or((cell.bg == Color::DarkGray).then_some('#'));
                match letter {
                    Some(letter) => out.push(letter),
                    None => out.push_str(cell.symbol()),
                }
            }
            out.push('\n');
        }
        out
    }

    fn press(app: &mut App, actions: &[Action]) {
        for &action in actions {
            app.handle(Command::Game(action));
        }
        app.tick(Duration::ZERO);
    }

    #[test]
    fn playing() {
        let mut app = App::new(7);
        press(&mut app, &[Action::HardDrop]);
        press(&mut app, &[Action::Hold]);
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn paused() {
        let mut app = App::new(7);
        app.handle(Command::Pause);
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn game_over() {
        let mut app = App::new(7);
        while app.view().game_over.is_none() {
            press(&mut app, &[Action::HardDrop]);
        }
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn terminal_too_small() {
        assert_snapshot!(render(&App::new(7), 30, 10));
    }
}
