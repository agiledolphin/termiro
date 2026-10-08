//! 彩蛋：生日蛋糕动画。在开始界面点击 TERMINO 标志打开，按任意键或点击关闭。

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::theme::Theme;
use crate::platform::ColorDepth;

/// 每帧 150ms，16 帧一轮，约 2.4 秒循环一次。
/// 所有动画的周期都是 16 帧的约数，循环时画面不会跳。
const FRAME_TIME: Duration = Duration::from_millis(150);
const FRAMES: u32 = 16;

/// 蛋糕形状，每个字符代表一类格子：`*` 烛焰、`|` 蜡烛、`w` 奶油、
/// `p` 草莓蛋糕胚、`c` 巧克力蛋糕胚、`=` 盘子，空格透明。
const CAKE: [&str; 12] = [
    "       *    *    *    *    *       ",
    "       |    |    |    |    |       ",
    "       |    |    |    |    |       ",
    "    wwwwwwwwwwwwwwwwwwwwwwwwwww    ",
    "    wpwwwpwwppwwwpwwpwwwwpwwpww    ",
    "    ppppppppppppppppppppppppppp    ",
    "    ppppppppppppppppppppppppppp    ",
    " wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww ",
    " wccwwcwwwccwwwcwwccwwwcwwcwwwcccw ",
    " ccccccccccccccccccccccccccccccccc ",
    " ccccccccccccccccccccccccccccccccc ",
    "===================================",
];
const CAKE_WIDTH: u16 = 35;
/// 第一根蜡烛所在的列，蜡烛之间相隔 5 列。
const FIRST_CANDLE: usize = 7;
/// 两侧和上方留给飘落彩纸的空间。
const MARGIN: u16 = 4;
const SKY: u16 = 4;
pub const WIDTH: u16 = CAKE_WIDTH + 2 * MARGIN;
/// 正好 16 行：彩纸每帧落一行，一轮落完一圈。
pub const HEIGHT: u16 = CAKE.len() as u16 + SKY;

const GREETING: &str = "HAPPY BIRTHDAY!";

/// 烛焰的四种形态，依次循环，看起来在跳动。
const FLAMES: [(char, Color); 4] = [
    ('^', Color::Yellow),
    ('*', Color::LightYellow),
    ('^', Color::LightRed),
    ('\'', Color::Yellow),
];
const RAINBOW: [Color; 8] = [
    Color::Red,
    Color::LightRed,
    Color::Yellow,
    Color::Green,
    Color::Cyan,
    Color::Blue,
    Color::Magenta,
    Color::LightMagenta,
];
const CANDLES: [Color; 5] = [
    Color::Cyan,
    Color::LightMagenta,
    Color::Yellow,
    Color::Green,
    Color::LightBlue,
];
const CONFETTI: [char; 4] = ['*', 'o', '+', '.'];
const CONFETTI_COUNT: u16 = 14;

/// 动画播放了 `elapsed` 之后应显示的帧。
pub fn frame_at(elapsed: Duration) -> u32 {
    (elapsed.as_millis() / FRAME_TIME.as_millis() % u128::from(FRAMES)) as u32
}

/// 蛋糕和飘落的彩纸，需要 [`WIDTH`]×[`HEIGHT`] 的区域。
pub struct Cake<'a> {
    pub frame: u32,
    pub theme: &'a Theme,
}

impl Widget for Cake<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frame = self.frame as usize;
        let colors = Palette::new(self.theme.depth());

        // 先撒彩纸，再把蛋糕画在上面
        for i in 0..CONFETTI_COUNT {
            let x = (i * 29 + 3) % WIDTH;
            let y = (i * 7 + self.frame as u16) % HEIGHT;
            let color = RAINBOW[(i as usize + frame / 2) % RAINBOW.len()];
            let symbol = CONFETTI[i as usize % CONFETTI.len()];
            put(buf, area, x, y, symbol, self.theme.fg(color));
        }

        for (dy, row) in CAKE.iter().enumerate() {
            for (dx, part) in row.chars().enumerate() {
                let candle = dx.saturating_sub(FIRST_CANDLE) / 5;
                // 蛋糕胚上零星的糖粒
                let sprinkle = (dx * 7 + dy * 5) % 9 == 0;
                let (symbol, style) = match (part, colors) {
                    (' ', _) => continue,
                    ('*', _) => {
                        let (symbol, color) = FLAMES[(frame + candle) % FLAMES.len()];
                        (symbol, self.theme.fg(color).bold())
                    }
                    ('=', _) => ('=', self.theme.fg(Color::Gray)),
                    // 无颜色时用字符画
                    ('|', None) => ('|', Style::new()),
                    ('w', None) => ('~', Style::new()),
                    ('p' | 'c', None) if sprinkle => {
                        (if (frame + dx) % 4 < 2 { 'o' } else { '.' }, Style::new())
                    }
                    ('p', None) => (':', Style::new()),
                    ('c', None) => ('#', Style::new()),
                    // 有颜色时用背景色填充；蜡烛是彩色和白色相间的条纹
                    ('|', Some(palette)) => {
                        let color = if dy % 2 == 1 {
                            CANDLES[candle % CANDLES.len()]
                        } else {
                            palette.frosting
                        };
                        (' ', Style::new().bg(color))
                    }
                    ('w', Some(palette)) => (' ', Style::new().bg(palette.frosting)),
                    ('p' | 'c', Some(palette)) => {
                        let base = if part == 'p' {
                            palette.strawberry
                        } else {
                            palette.chocolate
                        };
                        if sprinkle {
                            let color = RAINBOW[(frame + dx) % RAINBOW.len()];
                            ('.', Style::new().bg(base).fg(color).bold())
                        } else {
                            (' ', Style::new().bg(base))
                        }
                    }
                    _ => continue,
                };
                let (x, y) = (MARGIN + dx as u16, SKY + dy as u16);
                put(buf, area, x, y, symbol, style);
            }
        }
    }
}

/// 彩虹色的 `HAPPY BIRTHDAY!`，颜色随帧流动。
pub fn greeting(frame: u32, theme: &Theme) -> Line<'static> {
    let spans: Vec<Span> = GREETING
        .chars()
        .enumerate()
        .map(|(i, letter)| {
            let color = RAINBOW[(i + frame as usize) % RAINBOW.len()];
            Span::styled(letter.to_string(), theme.fg(color).bold())
        })
        .collect();
    Line::from(spans)
}

/// 蛋糕的填充色；无颜色模式下没有。
#[derive(Clone, Copy)]
struct Palette {
    frosting: Color,
    strawberry: Color,
    chocolate: Color,
}

impl Palette {
    fn new(depth: ColorDepth) -> Option<Self> {
        let palette = match depth {
            ColorDepth::TrueColor => Self {
                frosting: Color::Rgb(255, 248, 240),
                strawberry: Color::Rgb(255, 140, 180),
                chocolate: Color::Rgb(120, 70, 40),
            },
            ColorDepth::Ansi256 => Self {
                frosting: Color::Indexed(230),
                strawberry: Color::Indexed(211),
                chocolate: Color::Indexed(94),
            },
            // 16 色里没有棕色，下层改成香草色
            ColorDepth::Ansi16 => Self {
                frosting: Color::White,
                strawberry: Color::LightMagenta,
                chocolate: Color::Yellow,
            },
            ColorDepth::None => return None,
        };
        Some(palette)
    }
}

fn put(buf: &mut Buffer, area: Rect, x: u16, y: u16, symbol: char, style: Style) {
    if x >= area.width || y >= area.height {
        return;
    }
    if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
        cell.set_char(symbol).set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(frame: u32, depth: ColorDepth) -> Buffer {
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        let mut buf = Buffer::empty(area);
        let theme = Theme::new(depth, false);
        Cake {
            frame,
            theme: &theme,
        }
        .render(area, &mut buf);
        buf
    }

    #[test]
    fn template_is_rectangular() {
        assert!(CAKE.iter().all(|row| row.len() == CAKE_WIDTH as usize));
        assert_eq!(HEIGHT, FRAMES as u16);
    }

    #[test]
    fn frames_loop() {
        assert_eq!(frame_at(Duration::ZERO), 0);
        assert_eq!(frame_at(FRAME_TIME * 3 + FRAME_TIME / 2), 3);
        assert_eq!(frame_at(FRAME_TIME * FRAMES), 0);
    }

    #[test]
    fn animation_moves_and_loops_seamlessly() {
        for depth in [ColorDepth::TrueColor, ColorDepth::None] {
            assert_ne!(render(0, depth), render(1, depth));
            for frame in 0..FRAMES {
                assert_eq!(render(frame, depth), render(frame + FRAMES, depth));
            }
        }
    }
}
