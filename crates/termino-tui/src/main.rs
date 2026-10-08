//! Termino 终端入口：初始化终端，运行固定步长的主循环。

mod app;
mod input;
mod render;

use std::io;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

use crate::app::App;

/// 逻辑帧率固定为 60Hz，与终端刷新速度无关。
const TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);
/// 落后太多（比如进程被挂起）时不再追帧，直接从当前时刻继续。
const MAX_LAG: Duration = Duration::from_millis(250);

fn main() -> io::Result<()> {
    // ratatui::init 会开启 raw mode 和备用屏幕，并安装 panic hook 保证崩溃时也能还原终端
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let mut app = App::new(seed_from_clock());
    let mut next_tick = Instant::now() + TICK;

    while !app.should_quit() {
        terminal.draw(|frame| render::draw(frame, &app))?;

        let timeout = next_tick.saturating_duration_since(Instant::now());
        if event::poll(timeout)? {
            loop {
                if let Event::Key(key) = event::read()? {
                    if let Some(command) = input::map_key(key) {
                        app.handle(command);
                    }
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
            app.tick(TICK);
            next_tick += TICK;
        }
    }
    Ok(())
}

fn seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}
