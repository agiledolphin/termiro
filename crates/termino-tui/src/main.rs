//! Termino 终端入口：解析命令行、读取配置、初始化终端，运行固定步长的主循环。

mod app;
mod audio;
mod config;
mod input;
mod keymap;
mod party;
mod platform;
mod render;
mod storage;

use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, MouseButton, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::App;
use crate::config::{Birthday, ColorSetting, Config};
use crate::input::Input;
use crate::keymap::{Command, Keymap};
use crate::platform::ColorDepth;
use crate::render::Theme;

/// 逻辑帧率固定为 60Hz，与终端刷新速度无关。
const TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);
/// 落后太多（比如进程被挂起）时不再追帧，直接从当前时刻继续。
const MAX_LAG: Duration = Duration::from_millis(250);

const USAGE: &str = "\
Usage: termino [OPTION]

Options:
  --config-path     print where the config file is read from
  --default-config  print the default config file
  -h, --help        print this help";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--config-path") => {
            match Config::path() {
                Some(path) => println!("{}", path.display()),
                None => println!("(no config directory on this system)"),
            }
            return ExitCode::SUCCESS;
        }
        Some("--default-config") => {
            print!("{}", config::DEFAULT_TOML);
            return ExitCode::SUCCESS;
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("unknown option {other:?}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    // 配置有误时在进入全屏之前报错，用户才能看到
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => return fail(&format!("invalid config: {e}")),
    };
    let keymap = match Keymap::new(&config.keys) {
        Ok(keymap) => keymap,
        Err(e) => return fail(&format!("invalid config: {e}")),
    };

    let depth = match config.display.color {
        ColorSetting::Auto => platform::detect_color_depth(),
        ColorSetting::Truecolor => ColorDepth::TrueColor,
        ColorSetting::Ansi256 => ColorDepth::Ansi256,
        ColorSetting::Ansi16 => ColorDepth::Ansi16,
        ColorSetting::None => ColorDepth::None,
    };
    let theme = Theme::new(depth, config.display.ascii);

    let result = platform::init().and_then(|(terminal, caps)| {
        let input = Input::new(keymap, &config.timing, caps.key_release);
        run(terminal, input, &theme, &config.birthday)
    });
    platform::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e.to_string()),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("termino: {message}");
    ExitCode::FAILURE
}

fn run(
    mut terminal: DefaultTerminal,
    mut input: Input,
    theme: &Theme,
    birthday: &Birthday,
) -> io::Result<()> {
    let mut app = App::new(seed_from_clock())
        .with_best(storage::load())
        .with_name(&birthday.name)
        .with_age(birthday.age);
    let mut music = Music::default();
    let mut next_tick = Instant::now() + TICK;

    while !app.should_quit() {
        terminal.draw(|frame| render::draw(frame, &app, &input, theme))?;

        let timeout = next_tick.saturating_duration_since(Instant::now());
        if event::poll(timeout)? {
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        let command = if app.party().is_some() {
                            input.party_key(key)
                        } else if app.confirming().is_some() {
                            input.confirm_key(key)
                        } else {
                            input.key(key)
                        };
                        if let Some(command) = command {
                            app.handle(command);
                        }
                    }
                    Event::Mouse(mouse)
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left) =>
                    {
                        let size = terminal.size()?;
                        let area = Rect::new(0, 0, size.width, size.height);
                        if app.party().is_some() {
                            app.handle(Command::Dismiss);
                        } else if render::logo_hit(area, mouse.column, mouse.row) {
                            app.handle(Command::EasterEgg);
                        }
                    }
                    // 失去焦点后可能收不到按键释放，松开所有键并暂停
                    Event::FocusLost => {
                        input.release_all();
                        app.pause();
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        let now = Instant::now();
        if now.duration_since(next_tick) > MAX_LAG {
            next_tick = now;
        }
        while next_tick <= now {
            for action in input.tick(TICK) {
                app.handle(Command::Game(action));
            }
            app.tick(TICK);
            next_tick += TICK;
        }
        save_record(&mut app);
        music.follow(birthday.sound && app.party().is_some_and(|party| party.music_wanted()));
    }
    // 退出时放弃的那一局也可能破了纪录
    save_record(&mut app);
    Ok(())
}

/// 彩蛋配乐：跟着彩蛋打开和关闭。
#[derive(Default)]
struct Music {
    playing: Option<audio::Music>,
    /// 本次彩蛋已经尝试过开始播放；没有音频设备时不会每帧重试。
    tried: bool,
}

impl Music {
    fn follow(&mut self, wanted: bool) {
        if !wanted {
            self.playing = None;
            self.tried = false;
        } else if !self.tried {
            self.tried = true;
            self.playing = audio::play_birthday();
        }
    }
}

fn save_record(app: &mut App) {
    if let Some(record) = app.take_unsaved() {
        if storage::save(&record).is_err() {
            app.set_save_failed();
        }
    }
}

fn seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}
