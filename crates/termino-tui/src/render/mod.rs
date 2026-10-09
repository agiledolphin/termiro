//! 把 `App` 画到终端上。只读状态，不修改任何东西。
//!
//! 界面文字只用 ASCII：方向箭头、`·` 等字符在东亚语言环境下可能被当作双宽字符，导致错位。

mod board;
mod cake;
mod fireworks;
mod logo;
mod panel;
mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use termino_core::Action;

use self::board::BoardWidget;
pub use self::theme::Theme;

use crate::app::{App, Confirm};
use crate::input::Input;
use crate::keymap::{Command, Keymap};
use crate::party::Stage;

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
    if too_small(area) {
        draw_too_small(frame, area);
        return;
    }

    if let Some(party) = app.party() {
        draw_cake(frame, area, app, party.stage(), theme);
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

/// 开始界面各部分的位置。绘制和鼠标点击判断共用这一份计算。
struct TitleLayout {
    /// 标志实际占据的区域：大号是像素字，小号是一行彩色文字。
    logo: Rect,
    big: bool,
    best: Rect,
    hints: Rect,
}

const TITLE_HINTS: [(Command, &str); 2] = [
    (Command::Game(Action::HardDrop), "start"),
    (Command::Quit, "quit"),
];

fn title_layout(area: Rect) -> TitleLayout {
    let big = area.width >= logo::WIDTH;
    let logo_height = if big { logo::HEIGHT } else { 1 };
    let block = centered(area, area.width, logo_height + 4 + TITLE_HINTS.len() as u16);
    let [logo_row, _, best, _, hints] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(block);
    let logo = if big {
        centered(logo_row, logo::WIDTH, logo::HEIGHT)
    } else {
        centered(logo_row, logo::SMALL_WIDTH, 1)
    };
    TitleLayout {
        logo,
        big,
        best,
        hints,
    }
}

/// 鼠标点击的位置是否落在开始界面的标志上。
pub fn logo_hit(area: Rect, column: u16, row: u16) -> bool {
    !too_small(area) && title_layout(area).logo.contains(Position::new(column, row))
}

/// 开始界面：标志加上开始、退出的按键提示。终端不够宽时换成小号标志。
fn draw_title(frame: &mut Frame, area: Rect, app: &App, keys: &Keymap, theme: &Theme) {
    let layout = title_layout(area);
    if layout.big {
        frame.render_widget(logo::Logo { theme }, layout.logo);
    } else {
        frame.render_widget(logo::small(theme), layout.logo);
    }
    let best = app.best_score();
    if best > 0 {
        let line = Line::from(format!("BEST {best}").dim()).centered();
        frame.render_widget(line, layout.best);
    }
    // 提示文字左对齐成一列，整体居中
    let hints = TITLE_HINTS.map(|(command, what)| {
        Line::from(vec![
            format!("{:<7}", key_label(keys, command)).bold(),
            what.dim(),
        ])
    });
    let width = hints.iter().map(Line::width).max().unwrap_or(0) as u16;
    frame.render_widget(
        Paragraph::new(hints.to_vec()),
        centered(layout.hints, width, layout.hints.height),
    );
}

/// 彩蛋：生日蛋糕、彩虹色的祝福，以及按键提示。名字写在蛋糕上，太长时改放在祝福语下方。
/// 终端高度够时在各部分之间留空行。
fn draw_cake(frame: &mut Frame, area: Rect, app: &App, stage: Stage, theme: &Theme) {
    let name = app.name();
    let name_below = name.filter(|name| cake::name_label(name).is_none());
    let text_rows = 2 + u16::from(name_below.is_some());
    let gap = u16::from(area.height >= cake::HEIGHT + text_rows + 2);
    let block = centered(area, area.width, cake::HEIGHT + text_rows + 2 * gap);
    let [art, _, greeting, below, _, hint_row] = Layout::vertical([
        Constraint::Length(cake::HEIGHT),
        Constraint::Length(gap),
        Constraint::Length(1),
        Constraint::Length(u16::from(name_below.is_some())),
        Constraint::Length(gap),
        Constraint::Length(1),
    ])
    .areas(block);
    let cake = cake::Cake {
        stage,
        theme,
        name,
        age: app.age(),
    };
    // 烟花铺满整个画面，画在蛋糕下面一层
    frame.render_widget(fireworks::Fireworks { stage, theme }, area);
    frame.render_widget(cake, centered(art, cake::WIDTH, cake::HEIGHT));
    if let Some(line) = cake::greeting(stage, theme) {
        frame.render_widget(line.centered(), greeting);
    }
    if let (Some(name), Stage::Lit(_) | Stage::Blown(_)) = (name_below, stage) {
        frame.render_widget(Line::from(format!("~ {name} ~").bold()).centered(), below);
    }
    let hint = match stage {
        Stage::Stacking(_) => "Space  skip      Esc  close",
        Stage::Lit(_) => "Space  blow out the candles      Esc  close",
        Stage::Blown(_) => "Space  light again      Esc  close",
    };
    frame.render_widget(Line::from(hint.dim()).centered(), hint_row);
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

fn too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
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
    use crate::party::INTRO;
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
    fn logo_click_area() {
        let area = Rect::new(0, 0, 80, 24);
        let logo = title_layout(area).logo;
        assert_eq!((logo.width, logo.height), (logo::WIDTH, logo::HEIGHT));
        assert!(logo_hit(area, logo.x, logo.y));
        assert!(logo_hit(area, logo.right() - 1, logo.bottom() - 1));
        assert!(!logo_hit(area, logo.x, logo.bottom()));
        assert!(!logo_hit(area, 0, 0));

        // 窄终端上是一行小号标志
        let narrow = Rect::new(0, 0, MIN_WIDTH, MIN_HEIGHT);
        let small = title_layout(narrow).logo;
        assert_eq!((small.width, small.height), (logo::SMALL_WIDTH, 1));
        assert!(logo_hit(narrow, small.x + 6, small.y));

        // 终端太小时不显示标志
        assert!(!logo_hit(Rect::new(0, 0, 30, 10), 15, 5));
    }

    /// 打开彩蛋并播放到指定阶段。
    fn party(name: &str, age: u32, at: Duration, blow: bool) -> App {
        let mut app = App::new(7).with_name(name).with_age(age);
        app.handle(Command::EasterEgg);
        if blow {
            app.tick(INTRO);
            app.handle(Command::Blow);
        }
        app.tick(at);
        app
    }

    #[test]
    fn birthday_cake_stacking() {
        let app = party("Ada", 51, Duration::from_millis(1200), false);
        assert_snapshot!(render_with(&app, 80, 24, &PLAIN));
    }

    #[test]
    fn birthday_cake() {
        let app = party("Ada", 51, INTRO, false);
        assert_snapshot!(render_with(&app, 80, 24, &PLAIN));
    }

    #[test]
    fn birthday_cake_compact_candles() {
        let app = party("Ada", 51, INTRO, false);
        assert_snapshot!(render(&app, 80, 24));
    }

    #[test]
    fn birthday_cake_blown() {
        let app = party("Ada", 51, Duration::from_millis(1950), true);
        assert_snapshot!(render_with(&app, 80, 24, &PLAIN));
    }

    #[test]
    fn birthday_cake_with_long_name_on_small_terminal() {
        let app = party(&"Bartholomew".repeat(3), 0, INTRO, false);
        assert_snapshot!(render_with(&app, MIN_WIDTH, MIN_HEIGHT, &PLAIN));
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
