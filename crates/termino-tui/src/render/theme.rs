//! 配色与字符。颜色按终端能力分四档；另外可以只用 ASCII 字符画边框。

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::widgets::Block;
use termino_core::PieceKind;

use crate::platform::ColorDepth;

/// 一个盘面格子：两列宽的字符串和它的样式。
pub type Cell = (&'static str, Style);

/// 方框字符在东亚语言环境下可能被当作双宽字符，这时改用 ASCII 边框。
const ASCII_BORDER: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    depth: ColorDepth,
    ascii: bool,
}

impl Theme {
    pub const fn new(depth: ColorDepth, ascii: bool) -> Self {
        Self { depth, ascii }
    }

    /// 方块颜色；无颜色模式下为 `None`。
    pub fn color(&self, kind: PieceKind) -> Option<Color> {
        use PieceKind::*;
        let color = match self.depth {
            ColorDepth::TrueColor => match kind {
                I => Color::Rgb(0, 190, 220),
                O => Color::Rgb(240, 200, 0),
                T => Color::Rgb(170, 70, 210),
                S => Color::Rgb(90, 200, 70),
                Z => Color::Rgb(225, 60, 60),
                J => Color::Rgb(60, 100, 230),
                L => Color::Rgb(245, 140, 20),
            },
            ColorDepth::Ansi256 => Color::Indexed(match kind {
                I => 44,
                O => 220,
                T => 134,
                S => 76,
                Z => 160,
                J => 33,
                L => 208,
            }),
            // 16 色里没有橙色，L 用亮红，与 Z 的红色区分
            ColorDepth::Ansi16 => match kind {
                I => Color::Cyan,
                O => Color::Yellow,
                T => Color::Magenta,
                S => Color::Green,
                Z => Color::Red,
                J => Color::Blue,
                L => Color::LightRed,
            },
            ColorDepth::None => return None,
        };
        Some(color)
    }

    /// 实心格子：有颜色时是两个带背景色的空格，不依赖字体；无颜色时是 `[]`。
    pub fn block(&self, kind: PieceKind) -> Cell {
        match self.color(kind) {
            Some(color) => ("  ", Style::new().bg(color)),
            None => ("[]", Style::new()),
        }
    }

    /// 灰掉的格子，用于本回合已经用过的暂存方块。
    pub fn faded(&self) -> Cell {
        match self.depth {
            ColorDepth::TrueColor => ("  ", Style::new().bg(Color::Rgb(90, 90, 90))),
            ColorDepth::Ansi256 => ("  ", Style::new().bg(Color::Indexed(240))),
            ColorDepth::Ansi16 => ("  ", Style::new().bg(Color::DarkGray)),
            ColorDepth::None => ("::", Style::new()),
        }
    }

    /// 落点预览。
    pub fn ghost(&self, kind: PieceKind) -> Cell {
        match self.color(kind) {
            Some(color) => ("[]", Style::new().fg(color)),
            None => ("::", Style::new()),
        }
    }

    pub fn empty(&self) -> Cell {
        match self.depth {
            ColorDepth::None => (" .", Style::new().add_modifier(Modifier::DIM)),
            _ => (" .", Style::new().fg(Color::DarkGray)),
        }
    }

    /// 文字的强调色；无颜色模式下不着色。
    pub fn fg(&self, color: Color) -> Style {
        match self.depth {
            ColorDepth::None => Style::new(),
            _ => Style::new().fg(color),
        }
    }

    /// 带边框的方块容器。
    pub fn bordered(&self) -> Block<'static> {
        let set = if self.ascii {
            ASCII_BORDER
        } else {
            border::PLAIN
        };
        Block::bordered().border_set(set)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const DEPTHS: [ColorDepth; 3] = [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
    ];

    #[test]
    fn piece_colors_are_distinct_at_every_depth() {
        for depth in DEPTHS {
            let theme = Theme::new(depth, false);
            let colors: HashSet<Color> = PieceKind::ALL.map(|k| theme.color(k).unwrap()).into();
            assert_eq!(colors.len(), 7, "{depth:?}");
        }
    }

    #[test]
    fn no_color_uses_glyphs() {
        let theme = Theme::new(ColorDepth::None, false);
        assert_eq!(theme.block(PieceKind::T), ("[]", Style::new()));
        assert_eq!(theme.ghost(PieceKind::T).0, "::");
        assert_eq!(theme.fg(Color::Red), Style::new());
    }
}
