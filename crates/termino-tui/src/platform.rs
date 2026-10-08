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
