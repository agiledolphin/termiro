//! 把 `App` 画到终端上。只读状态，不修改任何东西。
//!
//! 界面文字只用 ASCII：方向箭头、`·` 等字符在东亚语言环境下可能被当作双宽字符，导致错位。

mod board;
mod logo;
mod panel;
mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use termino_core::Action;

use self::board::BoardWidget;
pub use self::theme::Theme;

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

pub fn draw(frame: &mut Frame, app: &App, input: &Input, theme: &Theme) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    if app.on_title() {
        draw_title(frame, area, app, input.keymap(), theme);
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
    panel::hold(&view, theme, hold_area, buf);
    panel::next(&view, theme, next_area, buf);
    frame.render_widget(BoardWidget::new(&view, theme), board_area);
    let stats = panel::stats(&view, app.best_score(), app.banner(), theme);
    frame.render_widget(stats, stats_area);
    let hint = format!("{}  pause/help", key_label(keys, Command::Pause));
    frame.render_widget(Line::from(hint.dim()), hint_area);

    if let Some(confirm) = app.confirming() {
        draw_overlay(frame, board_area, confirm_lines(confirm), theme);
    } else if view.game_over.is_some() {
        let mut lines = vec![
            Line::from("GAME OVER".bold()),
            Line::from(""),
            Line::from(format!("SCORE {}", view.score)),
        ];
        if app.new_record() {
            lines.push(Line::from(
                Span::styled("NEW RECORD!", theme.fg(Color::Yellow)).bold(),
            ));
        } else {
            lines.push(Line::from(format!("BEST {}", app.best_score()).dim()));
        }
        if app.save_failed() {
            lines.push(Line::from("(record not saved)".dim()));
        }
        lines.extend([
            Line::from(""),
            Line::from(format!("{}  restart", key_label(keys, Command::Restart)).dim()),
            Line::from(format!("{}  quit", key_label(keys, Command::Quit)).dim()),
        ]);
        draw_overlay(
            frame,
            board_area,
            lines.into_iter().map(Line::centered),
            theme,
        );
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
        draw_overlay(frame, board_area, lines, theme);
    }
}

/// 确认框的内容。确认键固定为 Y/Enter 和 N/Esc，不受按键配置影响。
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
fn draw_title(frame: &mut Frame, area: Rect, app: &App, keys: &Keymap, theme: &Theme) {
    let confirm = app.confirming();
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
    let best = app.best_score();

    let block = centered(area, area.width, logo_height + 4 + text_height);
    let [logo_area, _, best_area, _, text_area] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(block);
    if big {
        let logo = logo::Logo { theme };
        frame.render_widget(logo, centered(logo_area, logo::WIDTH, logo::HEIGHT));
    } else {
        frame.render_widget(logo::small(theme).centered(), logo_area);
    }
    if best > 0 {
        let line = Line::from(format!("BEST {best}").dim()).centered();
        frame.render_widget(line, best_area);
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
    theme: &Theme,
) {
    let lines: Vec<_> = lines.into_iter().collect();
    let area = centered(board_area, board::WIDTH - 2, lines.len() as u16 + 2);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(theme.bordered()), area);
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
    use termino_core::PieceKind;

    use super::*;
    use crate::app::tests::playing as started;
    use crate::config::Timing;
    use crate::platform::ColorDepth;
    use crate::storage::Record;

    const COLOR: Theme = Theme::new(ColorDepth::Ansi16, false);
    const PLAIN: Theme = Theme::new(ColorDepth::None, true);

    /// 把画面转成文本。纯文本快照看不到背景色，所以有背景色的格子
    /// 换成对应方块的字母（灰色为 `#`），其余格子保留原字符。
    fn render(app: &App, width: u16, height: u16) -> String {
        render_with(app, width, height, &COLOR)
    }

    fn render_with(app: &App, width: u16, height: u16, theme: &Theme) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let input = Input::new(Keymap::default(), &Timing::default(), true);
        terminal
            .draw(|frame| draw(frame, app, &input, theme))
            .unwrap();
        to_text(terminal.backend().buffer(), theme)
    }

    fn to_text(buf: &Buffer, theme: &Theme) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                let colored = |(_, style): theme::Cell| style.bg.is_some_and(|bg| bg == cell.bg);
                let letter = PieceKind::ALL
                    .into_iter()
                    .find(|&k| colored(theme.block(k)))
                    .map(PieceKind::letter)
                    .or(colored(theme.faded()).then_some('#'));
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
        let mut app = started(7).with_best(Some(Record {
            score: 100_000,
            lines: 120,
            level: 13,
        }));
        while app.view().game_over.is_none() {
            press(&mut app, &[Action::HardDrop]);
        }
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn new_record() {
        let mut app = started(7).with_best(Some(Record {
            score: 10,
            lines: 0,
            level: 1,
        }));
        while app.view().game_over.is_none() {
            press(&mut app, &[Action::HardDrop]);
        }
        assert_snapshot!(render(&app, MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn plain_theme() {
        let mut app = started(7);
        press(&mut app, &[Action::HardDrop]);
        press(&mut app, &[Action::Hold]);
        assert_snapshot!(render_with(&app, MIN_WIDTH, MIN_HEIGHT, &PLAIN));
    }

    #[test]
    fn plain_title() {
        let app = App::new(7).with_best(Some(Record {
            score: 4321,
            lines: 12,
            level: 2,
        }));
        assert_snapshot!(render_with(&app, 80, 24, &PLAIN));
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
