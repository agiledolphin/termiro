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

/// 蛋糕形状，每个字符代表一类格子：`<` `>` 烛焰左右两半、`|` 蜡烛、`w` 奶油、
/// `p` 草莓、`v` 香草、`c` 巧克力三层蛋糕胚、`=` 盘子，空格透明。
/// 宽度都取偶数，蜡烛占两列，这样偶数宽的名字（比如三个汉字加字间空格）能正好居中。
const CAKE: [&str; 15] = [
    "                 <>                 ",
    "                 ||                 ",
    "                 ||                 ",
    "           wwwwwwwwwwwwww           ",
    "           wpwwpwwwppwwpw           ",
    "           pppppppppppppp           ",
    "      wwwwwwwwwwwwwwwwwwwwwwww      ",
    "      wvwwwvwwvvwwwvwwvwwwwvwv      ",
    "      vvvvvvvvvvvvvvvvvvvvvvvv      ",
    " wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww ",
    " wccwwcwwwccwwwcwwccwwwcwwcwwwcccww ",
    " cccccccccccccccccccccccccccccccccc ",
    " cccccccccccccccccccccccccccccccccc ",
    " cccccccccccccccccccccccccccccccccc ",
    "====================================",
];
const CAKE_WIDTH: u16 = 36;
/// 两侧和上方留给飘落彩纸的空间。
const MARGIN: u16 = 4;
const SKY: u16 = 1;
pub const WIDTH: u16 = CAKE_WIDTH + 2 * MARGIN;
/// 正好 16 行：彩纸每帧落一行，一轮落完一圈。
pub const HEIGHT: u16 = CAKE.len() as u16 + SKY;

const GREETING: &str = "HAPPY BIRTHDAY!";
/// 名字写在最下层蛋糕胚三行的中间一行，在这一层（第 1 到 34 列）里水平居中，两侧各留一格不写。
const NAME_ROW: u16 = 12;
const BOTTOM_TIER_LEFT: u16 = 1;
const BOTTOM_TIER_WIDTH: u16 = 34;
const NAME_MAX_WIDTH: usize = BOTTOM_TIER_WIDTH as usize - 2;

/// 烛焰的四种形态，依次循环，看起来在左右摇曳。
const FLAMES: [(&str, Color); 4] = [
    ("/\\", Color::Yellow),
    ("/)", Color::LightYellow),
    ("()", Color::LightRed),
    ("(\\", Color::Yellow),
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
/// 蜡烛是这个颜色和奶油色相间的条纹。
const CANDLE: Color = Color::LightBlue;
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
    pub name: Option<&'a str>,
}

/// 写在蛋糕上的名字：放得下时字母之间加空格，像奶油裱字；再放不下就返回 `None`，
/// 由调用方改放在祝福语下方。
pub fn name_label(name: &str) -> Option<String> {
    let spaced: String = name.chars().map(String::from).collect::<Vec<_>>().join(" ");
    [format!(" {spaced} "), format!(" {name} ")]
        .into_iter()
        .find(|label| Span::raw(label.as_str()).width() <= NAME_MAX_WIDTH)
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

        let label = self.name.and_then(name_label);
        for (dy, row) in CAKE.iter().enumerate() {
            for (dx, part) in row.chars().enumerate() {
                // 蛋糕胚上零星的糖粒；写名字的那一行不撒，免得挤在名字旁边
                let name_row = label.is_some() && dy == NAME_ROW as usize;
                let sprinkle = (dx * 7 + dy * 5) % 9 == 0 && !name_row;
                let (symbol, style) = match (part, colors) {
                    (' ', _) => continue,
                    ('<' | '>', _) => {
                        let (shape, color) = FLAMES[frame % FLAMES.len()];
                        let half = usize::from(part == '>');
                        let symbol = shape.chars().nth(half).unwrap_or(' ');
                        (symbol, self.theme.fg(color).bold())
                    }
                    ('=', _) => ('=', self.theme.fg(Color::Gray)),
                    // 无颜色时用字符画
                    ('|', None) => ('|', Style::new()),
                    ('w', None) => ('~', Style::new()),
                    ('p' | 'v' | 'c', None) if sprinkle => {
                        (if (frame + dx) % 4 < 2 { 'o' } else { '.' }, Style::new())
                    }
                    ('p', None) => (':', Style::new()),
                    ('v', None) => ('%', Style::new()),
                    ('c', None) => ('#', Style::new()),
                    // 有颜色时用背景色填充；蜡烛是彩色和白色相间的条纹
                    ('|', Some(palette)) => {
                        let color = if dy % 2 == 1 {
                            CANDLE
                        } else {
                            palette.frosting
                        };
                        (' ', Style::new().bg(color))
                    }
                    ('w', Some(palette)) => (' ', Style::new().bg(palette.frosting)),
                    ('p' | 'v' | 'c', Some(palette)) => {
                        let base = match part {
                            'p' => palette.strawberry,
                            'v' => palette.vanilla,
                            _ => palette.chocolate,
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

        if let Some(label) = label {
            let width = Span::raw(label.as_str()).width() as u16;
            let x = MARGIN + BOTTOM_TIER_LEFT + BOTTOM_TIER_WIDTH.saturating_sub(width) / 2;
            let style = match colors {
                Some(palette) => Style::new().bg(palette.chocolate).fg(palette.frosting),
                None => Style::new(),
            };
            if x + width <= area.width && SKY + NAME_ROW < area.height {
                let (x, y) = (area.x + x, area.y + SKY + NAME_ROW);
                buf.set_stringn(x, y, &label, width as usize, style.bold());
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
    vanilla: Color,
    chocolate: Color,
}

impl Palette {
    fn new(depth: ColorDepth) -> Option<Self> {
        let palette = match depth {
            ColorDepth::TrueColor => Self {
                frosting: Color::Rgb(255, 248, 240),
                strawberry: Color::Rgb(255, 140, 180),
                vanilla: Color::Rgb(245, 215, 140),
                chocolate: Color::Rgb(120, 70, 40),
            },
            ColorDepth::Ansi256 => Self {
                frosting: Color::Indexed(230),
                strawberry: Color::Indexed(211),
                vanilla: Color::Indexed(222),
                chocolate: Color::Indexed(94),
            },
            // 16 色里没有棕色，巧克力层用红色代替
            ColorDepth::Ansi16 => Self {
                frosting: Color::White,
                strawberry: Color::LightMagenta,
                vanilla: Color::Yellow,
                chocolate: Color::Red,
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
            name: Some("Ada"),
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
    fn name_label_spacing_and_fallback() {
        assert_eq!(name_label("Ada").as_deref(), Some(" A d a "));
        assert_eq!(name_label("小明").as_deref(), Some(" 小 明 "));
        // 字母加空格放不下时去掉空格
        let long = "Bartholomew Smithers";
        assert_eq!(name_label(long), Some(format!(" {long} ")));
        // 实在放不下
        assert_eq!(name_label(&"长".repeat(16)), None);
    }

    #[test]
    fn even_width_name_is_exactly_centered() {
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        let mut buf = Buffer::empty(area);
        let theme = Theme::new(ColorDepth::None, false);
        Cake {
            frame: 0,
            theme: &theme,
            name: Some("东西西"),
        }
        .render(area, &mut buf);
        // 无颜色模式下巧克力层是 `#`，数名字两侧各有多少格
        let y = SKY + NAME_ROW;
        let tier = (MARGIN + BOTTOM_TIER_LEFT)..(MARGIN + BOTTOM_TIER_LEFT + BOTTOM_TIER_WIDTH);
        let row: Vec<&str> = tier.map(|x| buf[(x, y)].symbol()).collect();
        let left = row.iter().take_while(|s| **s == "#").count();
        let right = row.iter().rev().take_while(|s| **s == "#").count();
        assert_eq!(left, right, "{}", row.concat());
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
