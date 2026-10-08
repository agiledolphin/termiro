//! 把 `App` 画到终端上。只读状态，不修改任何东西。
//!
//! 界面文字只用 ASCII：方向箭头、`·` 等字符在东亚语言环境下可能被当作双宽字符，导致错位。

mod board;
mod logo;
mod panel;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

use self::board::BoardWidget;
use termino_core::Action;

use crate::app::{App, Confirm};
use crate::input::Input;
use crate::keymap::{Command, Keymap};

const MIN_WIDTH: u16 = panel::WIDTH + 1 + board::WIDTH + 1 + panel::WIDTH;
const MIN_HEIGHT: u16 = board::HEIGHT;

const HELP: [(Command, &str); 10] = [
    (Command::Game(Action::MoveLeft), "move left"),
    (Command::Game(Action::MoveRight), "move right"),
    (Command::Game(Action::SoftDrop), "soft drop"),
    (Command::Game(Action::HardDrop), "hard drop"),
    (Command::Game(Action::RotateCw), "rotate"),
    (Command::Game(Action::RotateCcw), "rotate ccw"),
    (Command::Game(Action::Hold), "hold"),
    (Command::Pause, "resume"),
    (Command::Restart, "restart"),
    (Command::Quit, "quit"),
];

pub fn draw(frame: &mut Frame, app: &App, input: &Input) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    if app.on_title() {
        draw_title(frame, area, input.keymap(), app.confirming());
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
    let keys = input.keymap();
    let buf = frame.buffer_mut();
    panel::hold(&view, hold_area, buf);
    panel::next(&view, next_area, buf);
    frame.render_widget(BoardWidget::new(&view), board_area);
    frame.render_widget(panel::stats(&view, app.banner()), stats_area);
    let hint = format!("{}  pause/help", key_label(keys, Command::Pause));
    frame.render_widget(Line::from(hint.dim()), hint_area);

    if let Some(confirm) = app.confirming() {
        draw_confirm(frame, board_area, confirm);
    } else if view.game_over.is_some() {
        let lines = vec![
            Line::from("GAME OVER".bold()),
            Line::from(""),
            Line::from(format!("SCORE {}", view.score)),
            Line::from(""),
            Line::from(format!("{}  restart", key_label(keys, Command::Restart)).dim()),
            Line::from(format!("{}  quit", key_label(keys, Command::Quit)).dim()),
        ];
        draw_overlay(frame, board_area, lines.into_iter().map(Line::centered));
    } else if app.paused() {
        let mut lines = vec![Line::from("PAUSED".bold()).centered(), Line::from("")];
        lines.extend(HELP.iter().map(|&(command, what)| {
            Line::from(vec![
                format!(" {:<7}", key_label(keys, command)).bold(),
                what.dim(),
            ])
        }));
        // 告诉玩家长按连发由谁控制：终端不支持时只能用系统的按键重复
        let repeat = if input.has_auto_repeat() { "DAS" } else { "OS" };
        lines.push(Line::from(""));
        lines.push(Line::from(format!(" auto-repeat: {repeat}").dim()));
        draw_overlay(frame, board_area, lines);
    }
}

/// 确认框。确认键固定为 Y/Enter 和 N/Esc，不受按键配置影响。
fn draw_confirm(frame: &mut Frame, area: Rect, confirm: Confirm) {
    draw_overlay(frame, area, confirm_lines(confirm));
}

fn confirm_lines(confirm: Confirm) -> [Line<'static>; 4] {
    let question = match confirm {
        Confirm::Quit => "QUIT?",
        Confirm::Restart => "RESTART?",
    };
    [
        Line::from(question.bold()),
        Line::from(""),
        Line::from(vec!["Y Enter  ".bold(), "yes".dim()]),
        Line::from(vec!["N Esc    ".bold(), "no ".dim()]),
    ]
    .map(Line::centered)
}

/// 开始界面：标志加上开始、退出的按键提示；确认退出时提示换成确认问题。
/// 终端不够宽时换成小号标志。
fn draw_title(frame: &mut Frame, area: Rect, keys: &Keymap, confirm: Option<Confirm>) {
    let big = area.width >= logo::WIDTH;
    let logo_height = if big { logo::HEIGHT } else { 1 };
    let hints = [
        (Command::Game(Action::HardDrop), "start"),
        (Command::Quit, "quit"),
    ]
    .map(|(command, what)| {
        Line::from(vec![
            format!("{:<7}", key_label(keys, command)).bold(),
            what.dim(),
        ])
    });
    let text_height = match confirm {
        Some(_) => 4,
        None => hints.len() as u16,
    };

    let block = centered(area, area.width, logo_height + 2 + text_height);
    let [logo_area, _, text_area] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(block);
    if big {
        frame.render_widget(logo::Logo, centered(logo_area, logo::WIDTH, logo::HEIGHT));
    } else {
        frame.render_widget(logo::small().centered(), logo_area);
    }
    match confirm {
        Some(confirm) => {
            frame.render_widget(Paragraph::new(confirm_lines(confirm).to_vec()), text_area)
        }
        None => {
            // 提示文字左对齐成一列，整体居中
            let width = hints.iter().map(Line::width).max().unwrap_or(0) as u16;
            frame.render_widget(
                Paragraph::new(hints.to_vec()),
                centered(text_area, width, text_area.height),
            );
        }
    }
}

/// 按键提示，过长时截断以免挤乱布局。
fn key_label(keys: &Keymap, command: Command) -> String {
    keys.label(command).chars().take(6).collect()
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
    use termino_core::PieceKind;

    use super::*;
    use crate::app::tests::playing as started;
    use crate::config::Timing;

    /// 把画面转成文本。纯文本快照看不到背景色，所以有背景色的格子
    /// 换成对应方块的字母（灰色为 `#`），其余格子保留原字符。
    fn render(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let input = Input::new(Keymap::default(), &Timing::default(), true);
        terminal.draw(|frame| draw(frame, app, &input)).unwrap();
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
        let mut app = started(7);
        press(&mut app, &[Action::HardDrop]);
        press(&mut app, &[Action::Hold]);
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn paused() {
        let mut app = started(7);
        app.handle(Command::Pause);
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn game_over() {
        let mut app = started(7);
        while app.view().game_over.is_none() {
            press(&mut app, &[Action::HardDrop]);
        }
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn confirm_restart() {
        let mut app = started(7);
        app.handle(Command::Restart);
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn confirm_quit_on_title() {
        let mut app = App::new(7);
        app.handle(Command::Quit);
        assert_snapshot!(render(&app, 80, 24));
    }

    #[test]
    fn title() {
        assert_snapshot!(render(&App::new(7), 80, 24));
    }

    #[test]
    fn title_on_narrow_terminal() {
        assert_snapshot!(render(&App::new(7), MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn terminal_too_small() {
        assert_snapshot!(render(&started(7), 30, 10));
    }
}
