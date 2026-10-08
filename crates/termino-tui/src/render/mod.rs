//! 把 `App` 画到终端上。只读状态，不修改任何东西。
//!
//! 界面文字只用 ASCII：方向箭头、`·` 等字符在东亚语言环境下可能被当作双宽字符，导致错位。

mod board;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use termino_core::GameView;

use self::board::BoardWidget;
use crate::app::App;

const SIDEBAR_WIDTH: u16 = 22;
const MIN_WIDTH: u16 = board::WIDTH + 1 + SIDEBAR_WIDTH;
const MIN_HEIGHT: u16 = board::HEIGHT;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    let [board_area, _, sidebar_area] = Layout::horizontal([
        Constraint::Length(board::WIDTH),
        Constraint::Length(1),
        Constraint::Length(SIDEBAR_WIDTH),
    ])
    .areas(centered(area, MIN_WIDTH, MIN_HEIGHT));

    let view = app.view();
    frame.render_widget(BoardWidget::new(&view), board_area);
    frame.render_widget(sidebar(&view), sidebar_area);

    if view.game_over.is_some() {
        draw_overlay(frame, board_area, "GAME OVER", &["R  restart", "Q  quit"]);
    } else if app.paused() {
        draw_overlay(frame, board_area, "PAUSED", &["P  resume", "R  restart"]);
    }
}

fn sidebar(view: &GameView) -> Paragraph<'static> {
    let help = [
        ("Left/Right", "move"),
        ("Down", "soft drop"),
        ("Space", "hard drop"),
        ("Up / X", "rotate cw"),
        ("Z", "rotate ccw"),
        ("P", "pause"),
        ("Q", "quit"),
    ];
    let mut lines = vec![
        Line::from(""),
        Line::from(vec!["LINES ".bold(), view.lines.to_string().into()]),
        Line::from(""),
    ];
    lines.extend(
        help.iter()
            .map(|(key, what)| Line::from(vec![format!("{key:<11}").bold(), what.dim()])),
    );
    Paragraph::new(lines)
}

/// 盖在盘面中央的提示框，宽度与盘面内部一致。
fn draw_overlay(frame: &mut Frame, board_area: Rect, title: &str, hints: &[&str]) {
    let mut text = vec![Line::from(title.bold()), Line::from("")];
    text.extend(hints.iter().map(|hint| Line::from(hint.dim())));
    let area = centered(board_area, board::WIDTH - 2, text.len() as u16 + 2);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::bordered()),
        area,
    );
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Terminal too small".bold()),
        Line::from(format!(
            "need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        )),
    ];
    let area = centered(area, area.width, 2);
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
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
    use termino_core::Action;

    use super::*;
    use crate::input::Command;

    fn render(app: &App, width: u16, height: u16) -> TestBackend {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().clone()
    }

    #[test]
    fn playing() {
        let mut app = App::new(7);
        app.handle(Command::Game(Action::HardDrop));
        app.tick(Duration::ZERO);
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
            app.handle(Command::Game(Action::HardDrop));
            app.tick(Duration::ZERO);
        }
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn terminal_too_small() {
        assert_snapshot!(render(&App::new(7), 30, 10));
    }
}
