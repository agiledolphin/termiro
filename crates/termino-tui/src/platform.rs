//! 终端初始化、能力探测与还原。平台差异集中在这里处理。

use std::io::{self, stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    DisableFocusChange, EnableFocusChange, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

/// 是否开启了 kitty 键盘协议，还原时需要关闭。
static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);

pub struct Capabilities {
    /// 能收到按键释放事件，可以自己实现长按连发。
    pub key_release: bool,
}

/// 进入备用屏幕和 raw mode，开启能用的增强功能。
/// 之后无论正常退出还是 panic，都会通过 [`restore`] 还原终端。
pub fn init() -> io::Result<(DefaultTerminal, Capabilities)> {
    // ratatui::init 自带 panic hook，负责关闭 raw mode 和离开备用屏幕
    let terminal = ratatui::init();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));

    let enhanced = supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
        KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
    }
    // 不支持焦点事件的终端会忽略这条指令
    execute!(stdout(), EnableFocusChange)?;

    Ok((
        terminal,
        Capabilities {
            // Windows 的控制台 API 本身就上报按键释放
            key_release: enhanced || cfg!(windows),
        },
    ))
}

/// 还原终端。可以重复调用。
pub fn restore() {
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(stdout(), DisableFocusChange);
    ratatui::restore();
}

/// 终端能显示的颜色数量。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
    Ansi16,
    None,
}

/// 根据环境变量推断颜色能力。
pub fn detect_color_depth() -> ColorDepth {
    color_depth_from(|name| std::env::var(name).ok())
}

fn color_depth_from(env: impl Fn(&str) -> Option<String>) -> ColorDepth {
    // https://no-color.org：设置了非空的 NO_COLOR 就不输出颜色
    if env("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return ColorDepth::None;
    }
    let colorterm = env("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return ColorDepth::TrueColor;
    }
    let term = env("TERM").unwrap_or_default();
    if term == "dumb" {
        return ColorDepth::None;
    }
    // Windows Terminal 支持真彩色，但不设置 COLORTERM
    if env("WT_SESSION").is_some() {
        return ColorDepth::TrueColor;
    }
    // macOS 自带的终端是 xterm-256color，不支持真彩色
    if term.contains("256color") {
        return ColorDepth::Ansi256;
    }
    ColorDepth::Ansi16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depth(vars: &[(&str, &str)]) -> ColorDepth {
        color_depth_from(|name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn detects_color_depth_from_environment() {
        assert_eq!(
            depth(&[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]),
            ColorDepth::TrueColor
        );
        assert_eq!(depth(&[("TERM", "xterm-256color")]), ColorDepth::Ansi256);
        assert_eq!(depth(&[("TERM", "xterm")]), ColorDepth::Ansi16);
        assert_eq!(depth(&[]), ColorDepth::Ansi16);
        assert_eq!(depth(&[("WT_SESSION", "abc")]), ColorDepth::TrueColor);
        assert_eq!(depth(&[("TERM", "dumb")]), ColorDepth::None);
    }

    #[test]
    fn no_color_wins_unless_empty() {
        assert_eq!(
            depth(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")]),
            ColorDepth::None
        );
        assert_eq!(
            depth(&[("NO_COLOR", ""), ("TERM", "xterm-256color")]),
            ColorDepth::Ansi256
        );
    }
}
