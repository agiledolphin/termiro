//! 彩蛋：生日蛋糕动画。在开始界面点击 TERMINO 标志打开。
//!
//! 开场时盘子先摆好，三层蛋糕和蜡烛像俄罗斯方块一样依次落下堆好，蜡烛逐一点亮；
//! 之后烛焰跳动、彩纸飘落，循环播放。按空格吹灭蜡烛：冒出青烟，随后放烟花。

use std::ops::Range;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::theme::Theme;
use crate::party::Stage;
use crate::platform::ColorDepth;

/// 循环动画每帧 150ms，24 帧一轮（3.6 秒）。烛焰 4 帧、彩虹色 8 帧、彩纸和烟花 24 帧一个周期，
/// 都能整除 24，循环时画面不会跳。
const FRAME_TIME: Duration = Duration::from_millis(150);
const FRAMES: u32 = 24;

/// 三层蛋糕和盘子，从上到下。每个字符代表一类格子：`w` 奶油、`p` 草莓、`v` 香草、
/// `c` 巧克力、`=` 盘子，空格透明。宽度都取偶数，偶数宽的名字和蜡烛能正好居中。
const TIERS: [&str; 12] = [
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
/// 蛋糕两侧留给彩纸和烟花的空间。
const MARGIN: u16 = 4;
pub const WIDTH: u16 = CAKE_WIDTH + 2 * MARGIN;
pub const HEIGHT: u16 = 20;
/// TIERS 第一行在画面中的行：蛋糕贴着画面底部，上方留给蜡烛和烟花。
const TIERS_TOP: u16 = HEIGHT - TIERS.len() as u16;

/// 开场时依次落下的部分：(在 TIERS 中的行, 开始落下的毫秒数)。盘子一开始就摆好。
const PLATE: Range<usize> = 11..12;
const LAYERS: [(Range<usize>, u64); 3] = [(6..11, 100), (3..6, 550), (0..3, 1000)];
/// 蜡烛最后落下，然后逐一点亮。开场总长见 [`crate::party::INTRO`]。
const CANDLES_DROP_MS: u64 = 1450;
const DROP_MS: u64 = 400;
const LIGHT_MS: u64 = 2000;
const LIGHT_STEP_MS: u64 = 200;
const _: () = assert!(CANDLES_DROP_MS + DROP_MS <= LIGHT_MS, "蜡烛要先落好再点亮");
/// 吹灭蜡烛后，先冒烟，过一会儿开始放烟花。
const SMOKE_FRAMES: i32 = 8;
const FIREWORKS_DELAY: Duration = Duration::from_millis(900);

/// 名字写在最下层蛋糕胚三行的中间一行（TIERS 第 9 行），在这一层（第 1 到 34 列）里水平居中，
/// 两侧各留一格不写。
const NAME_ROW: usize = 9;
const BOTTOM_TIER_LEFT: u16 = 1;
const BOTTOM_TIER_WIDTH: u16 = 34;
const NAME_MAX_WIDTH: usize = BOTTOM_TIER_WIDTH as usize - 2;

/// 数字蜡烛的 3×5 像素字，每个像素占 1 列，像细长的数字蜡烛；两根之间空 2 列。
const DIGITS: [[&str; 5]; 10] = [
    ["###", "#.#", "#.#", "#.#", "###"],
    [".#.", "##.", ".#.", ".#.", "###"],
    ["###", "..#", "###", "#..", "###"],
    ["###", "..#", "###", "..#", "###"],
    ["#.#", "#.#", "###", "..#", "..#"],
    ["###", "#..", "###", "..#", "###"],
    ["###", "#..", "###", "#.#", "###"],
    ["###", "..#", ".#.", ".#.", ".#."],
    ["###", "#.#", "###", "#.#", "###"],
    ["###", "#.#", "###", "..#", "###"],
];
const DIGIT_WIDTH: u16 = 3;
const DIGIT_GAP: u16 = 2;

/// 数字蜡烛上一列宽的小火苗，四种形态依次循环，看起来在跳动。
const SMALL_FLAMES: [(char, Color); 4] = [
    ('^', Color::Yellow),
    ('*', Color::LightYellow),
    ('^', Color::LightRed),
    ('\'', Color::Yellow),
];
/// 普通蜡烛上两列宽的烛焰，四种形态依次循环，看起来在左右摇曳。
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
/// 普通蜡烛是这个颜色和奶油色相间的条纹。
const STRIPE: Color = Color::LightBlue;
const CONFETTI: [char; 4] = ['*', 'o', '+', '.'];
const CONFETTI_COUNT: u16 = 14;
/// 烟花：(中心列, 中心行, 在一轮中绽放的帧, 颜色)。画在蛋糕两侧的空处。
const BURSTS: [(i32, i32, u32, Color); 4] = [
    (6, 5, 0, Color::LightRed),
    (37, 4, 6, Color::Yellow),
    (8, 2, 12, Color::Cyan),
    (36, 8, 18, Color::LightMagenta),
];
const BURST_DIRECTIONS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

const GREETING: &str = "HAPPY BIRTHDAY!";
const WISH: &str = "MAKE A WISH!";

/// 循环动画在 `t` 时应显示的帧。
fn frame_at(t: Duration) -> u32 {
    (t.as_millis() / FRAME_TIME.as_millis() % u128::from(FRAMES)) as u32
}

/// 蛋糕和周围的特效，需要 [`WIDTH`]×[`HEIGHT`] 的区域。
pub struct Cake<'a> {
    pub stage: Stage,
    pub theme: &'a Theme,
    pub name: Option<&'a str>,
    /// 1 到 99 时画成数字蜡烛，否则是一根普通蜡烛。
    pub age: Option<u8>,
}

/// 写在蛋糕上的名字：放得下时字之间加空格，像奶油裱字；再放不下就返回 `None`，
/// 由调用方改放在祝福语下方。
pub fn name_label(name: &str) -> Option<String> {
    let spaced: String = name.chars().map(String::from).collect::<Vec<_>>().join(" ");
    [format!(" {spaced} "), format!(" {name} ")]
        .into_iter()
        .find(|label| Span::raw(label.as_str()).width() <= NAME_MAX_WIDTH)
}

/// 祝福语：开场时不显示，点燃时是 `HAPPY BIRTHDAY!`，吹灭后是 `MAKE A WISH!`，颜色随帧流动。
pub fn greeting(stage: Stage, theme: &Theme) -> Option<Line<'static>> {
    let (text, frame) = match stage {
        Stage::Stacking(_) => return None,
        Stage::Lit(t) => (GREETING, frame_at(t)),
        Stage::Blown(t) => (WISH, frame_at(t)),
    };
    let spans: Vec<Span> = text
        .chars()
        .enumerate()
        .map(|(i, letter)| {
            let color = RAINBOW[(i + frame as usize) % RAINBOW.len()];
            Span::styled(letter.to_string(), theme.fg(color).bold())
        })
        .collect();
    Some(Line::from(spans))
}

/// 蜡烛的格子和烛焰位置。x 是蛋糕内的列，y 是画面行。
struct Candles {
    cells: Vec<(u16, u16, CandleCell)>,
    /// 每簇烛焰的位置；两列宽的烛焰是左半边的位置。
    flames: Vec<(u16, u16)>,
    /// 数字蜡烛用一列宽的小火苗。
    small_flames: bool,
    top: u16,
}

#[derive(Clone, Copy)]
enum CandleCell {
    /// 普通蜡烛的条纹，`true` 为彩色段。
    Stripe(bool),
    Digit,
}

impl Candles {
    fn new(age: Option<u8>) -> Self {
        let bottom = TIERS_TOP - 1;
        let mut cells = Vec::new();
        let mut flames = Vec::new();
        match age.filter(|age| (1..=99).contains(age)) {
            Some(age) => {
                let digits: Vec<usize> = age
                    .to_string()
                    .bytes()
                    .map(|b| usize::from(b - b'0'))
                    .collect();
                let count = digits.len() as u16;
                let width = count * DIGIT_WIDTH + (count - 1) * DIGIT_GAP;
                let top = bottom - 4;
                let mut left = (CAKE_WIDTH - width) / 2;
                for digit in digits {
                    for (dy, row) in DIGITS[digit].iter().enumerate() {
                        for (dx, pixel) in row.chars().enumerate() {
                            if pixel == '#' {
                                cells.push((left + dx as u16, top + dy as u16, CandleCell::Digit));
                            }
                        }
                    }
                    flames.push((left + DIGIT_WIDTH / 2, top - 1));
                    left += DIGIT_WIDTH + DIGIT_GAP;
                }
                Self {
                    cells,
                    flames,
                    small_flames: true,
                    top,
                }
            }
            None => {
                let center = CAKE_WIDTH / 2 - 1;
                for y in bottom - 1..=bottom {
                    let colored = (bottom - y) % 2 == 1;
                    cells.push((center, y, CandleCell::Stripe(colored)));
                    cells.push((center + 1, y, CandleCell::Stripe(colored)));
                }
                flames.push((center, bottom - 2));
                Self {
                    cells,
                    flames,
                    small_flames: false,
                    top: bottom - 1,
                }
            }
        }
    }
}

/// 开场时某部分落下的位移（行数，负数表示还在上方）。还没开始落下时为 `None`。
/// 每次落一整行，看起来像方块在下落。
fn drop_shift(ms: u64, start: u64, final_top: u16, height: u16) -> Option<i32> {
    if ms < start {
        return None;
    }
    let distance = i32::from(final_top + height);
    let fallen = distance * (ms - start).min(DROP_MS) as i32 / DROP_MS as i32;
    Some(fallen - distance)
}

impl Widget for Cake<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 区域不够高时裁掉上方的天空，保证蛋糕底部完整
        let skip = HEIGHT.saturating_sub(area.height);
        let mut canvas = Canvas { buf, area, skip };
        let palette = Palette::new(self.theme.depth());
        // 开场经过的毫秒数；开场结束后视为所有部分都已落好
        let (ms, frame) = match self.stage {
            Stage::Stacking(t) => (t.as_millis() as u64, 0),
            Stage::Lit(t) | Stage::Blown(t) => (u64::MAX, frame_at(t)),
        };

        match self.stage {
            Stage::Lit(_) => self.confetti(&mut canvas, frame),
            Stage::Blown(t) if t >= FIREWORKS_DELAY => {
                self.fireworks(&mut canvas, frame_at(t - FIREWORKS_DELAY))
            }
            _ => {}
        }

        let label = self.name.and_then(name_label);
        self.tiers(&mut canvas, PLATE, 0, frame, palette, label.is_some());
        for (rows, start) in LAYERS {
            let top = TIERS_TOP + rows.start as u16;
            let Some(shift) = drop_shift(ms, start, top, rows.len() as u16) else {
                continue;
            };
            let has_name = rows.contains(&NAME_ROW);
            self.tiers(&mut canvas, rows, shift, frame, palette, label.is_some());
            if let (true, Some(label)) = (has_name, &label) {
                self.name(&mut canvas, label, shift, palette);
            }
        }

        let candles = Candles::new(self.age);
        let height = TIERS_TOP - candles.top;
        if let Some(shift) = drop_shift(ms, CANDLES_DROP_MS, candles.top, height) {
            self.candles(&mut canvas, &candles, shift, frame, palette);
        }
        self.flames(&mut canvas, &candles, ms, frame);
    }
}

impl Cake<'_> {
    fn tiers(
        &self,
        canvas: &mut Canvas,
        rows: Range<usize>,
        shift: i32,
        frame: u32,
        palette: Option<Palette>,
        has_name: bool,
    ) {
        let frame = frame as usize;
        for dy in rows {
            for (dx, part) in TIERS[dy].chars().enumerate() {
                // 蛋糕胚上零星的糖粒；写名字的那一行不撒，免得挤在名字旁边
                let sprinkle = (dx * 7 + dy * 5) % 9 == 0 && !(has_name && dy == NAME_ROW);
                let (symbol, style) = match (part, palette) {
                    (' ', _) => continue,
                    ('=', _) => ('=', self.theme.fg(Color::Gray)),
                    // 无颜色时用字符画
                    ('w', None) => ('~', Style::new()),
                    ('p' | 'v' | 'c', None) if sprinkle => {
                        (if (frame + dx) % 4 < 2 { 'o' } else { '.' }, Style::new())
                    }
                    ('p', None) => (':', Style::new()),
                    ('v', None) => ('%', Style::new()),
                    ('c', None) => ('#', Style::new()),
                    // 有颜色时用背景色填充
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
                let y = i32::from(TIERS_TOP) + dy as i32 + shift;
                canvas.put(i32::from(MARGIN) + dx as i32, y, symbol, style);
            }
        }
    }

    fn name(&self, canvas: &mut Canvas, label: &str, shift: i32, palette: Option<Palette>) {
        let width = Span::raw(label).width() as u16;
        let x = MARGIN + BOTTOM_TIER_LEFT + BOTTOM_TIER_WIDTH.saturating_sub(width) / 2;
        let y = i32::from(TIERS_TOP) + NAME_ROW as i32 + shift;
        let style = match palette {
            Some(palette) => Style::new().bg(palette.chocolate).fg(palette.frosting),
            None => Style::new(),
        };
        canvas.text(x, y, label, style.bold());
    }

    fn candles(
        &self,
        canvas: &mut Canvas,
        candles: &Candles,
        shift: i32,
        frame: u32,
        palette: Option<Palette>,
    ) {
        for &(x, y, cell) in &candles.cells {
            // 数字蜡烛上闪烁的亮片
            let glitter = (u32::from(x) * 3 + u32::from(y) * 5 + frame) % 11 == 0;
            let (symbol, style) = match (cell, palette) {
                (CandleCell::Stripe(_), None) => ('|', Style::new()),
                (CandleCell::Digit, None) => ('@', Style::new()),
                (CandleCell::Stripe(colored), Some(palette)) => {
                    let color = if colored { STRIPE } else { palette.frosting };
                    (' ', Style::new().bg(color))
                }
                (CandleCell::Digit, Some(palette)) if glitter => {
                    ('.', Style::new().bg(palette.gold).fg(Color::White).bold())
                }
                (CandleCell::Digit, Some(palette)) => (' ', Style::new().bg(palette.gold)),
            };
            canvas.put(i32::from(MARGIN + x), i32::from(y) + shift, symbol, style);
        }
    }

    /// 开场时蜡烛逐一点亮，点燃后跳动，吹灭后冒烟。
    fn flames(&self, canvas: &mut Canvas, candles: &Candles, ms: u64, frame: u32) {
        for (i, &(x, y)) in candles.flames.iter().enumerate() {
            let (x, y) = (i32::from(MARGIN + x), i32::from(y));
            match self.stage {
                Stage::Stacking(_) if ms < LIGHT_MS + i as u64 * LIGHT_STEP_MS => {}
                Stage::Stacking(_) | Stage::Lit(_) => {
                    let step = frame as usize + i;
                    if candles.small_flames {
                        let (symbol, color) = SMALL_FLAMES[step % SMALL_FLAMES.len()];
                        canvas.put(x, y, symbol, self.theme.fg(color).bold());
                    } else {
                        let (shape, color) = FLAMES[step % FLAMES.len()];
                        canvas.text(x as u16, y, shape, self.theme.fg(color).bold());
                    }
                }
                Stage::Blown(t) => {
                    // 一缕青烟左右摆动着往上飘，几帧后散去
                    let age = (t.as_millis() / FRAME_TIME.as_millis()) as i32;
                    if age >= SMOKE_FRAMES {
                        continue;
                    }
                    for j in 0..=age.min(3) {
                        let symbol = if (j + age) % 2 == 0 { '(' } else { ')' };
                        let style = self.theme.fg(Color::Gray).dim();
                        canvas.put(x + j % 2, y - j - age / 3, symbol, style);
                    }
                }
            }
        }
    }

    fn confetti(&self, canvas: &mut Canvas, frame: u32) {
        for i in 0..CONFETTI_COUNT {
            let x = i32::from((i * 29 + 3) % WIDTH);
            // 一轮 24 帧落下 24 行，比画面高，循环时从顶上重新出现
            let y = ((u32::from(i) * 7 + frame) % FRAMES) as i32;
            let color = RAINBOW[(i as usize + frame as usize / 2) % RAINBOW.len()];
            let symbol = CONFETTI[i as usize % CONFETTI.len()];
            canvas.put(x, y, symbol, self.theme.fg(color));
        }
    }

    /// 烟花从中心绽开，扩散成一圈后淡去。
    fn fireworks(&self, canvas: &mut Canvas, frame: u32) {
        for &(cx, cy, start, color) in &BURSTS {
            let age = (frame + FRAMES - start) % FRAMES;
            let (radius, symbol) = match age {
                0 => (0, '*'),
                1 => (1, '+'),
                2 => (2, '*'),
                3 | 4 => (3, '.'),
                _ => continue,
            };
            let mut style = self.theme.fg(color).bold();
            if age >= 4 {
                style = self.theme.fg(color).dim();
            }
            if radius == 0 {
                canvas.put(cx, cy, symbol, style);
                continue;
            }
            for (dx, dy) in BURST_DIRECTIONS {
                // 横向距离加倍，终端字符瘦长，这样烟花看起来是圆的
                canvas.put(cx + dx * 2 * radius, cy + dy * radius, symbol, style);
            }
        }
    }
}

/// 蛋糕的填充色；无颜色模式下没有。
#[derive(Clone, Copy)]
struct Palette {
    frosting: Color,
    strawberry: Color,
    vanilla: Color,
    chocolate: Color,
    /// 数字蜡烛的金色。
    gold: Color,
}

impl Palette {
    fn new(depth: ColorDepth) -> Option<Self> {
        let palette = match depth {
            ColorDepth::TrueColor => Self {
                frosting: Color::Rgb(255, 248, 240),
                strawberry: Color::Rgb(255, 140, 180),
                vanilla: Color::Rgb(245, 215, 140),
                chocolate: Color::Rgb(120, 70, 40),
                gold: Color::Rgb(255, 196, 40),
            },
            ColorDepth::Ansi256 => Self {
                frosting: Color::Indexed(230),
                strawberry: Color::Indexed(211),
                vanilla: Color::Indexed(222),
                chocolate: Color::Indexed(94),
                gold: Color::Indexed(214),
            },
            // 16 色里没有棕色，巧克力层用红色代替
            ColorDepth::Ansi16 => Self {
                frosting: Color::White,
                strawberry: Color::LightMagenta,
                vanilla: Color::Yellow,
                chocolate: Color::Red,
                gold: Color::LightYellow,
            },
            ColorDepth::None => return None,
        };
        Some(palette)
    }
}

/// 按区域内坐标写格子，超出区域的部分直接忽略；坐标可以是负数（还在画面上方）。
struct Canvas<'a> {
    buf: &'a mut Buffer,
    area: Rect,
    /// 画面顶部被裁掉的行数。
    skip: u16,
}

impl Canvas<'_> {
    fn put(&mut self, x: i32, y: i32, symbol: char, style: Style) {
        let y = y - i32::from(self.skip);
        let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
            return;
        };
        if x >= self.area.width || y >= self.area.height {
            return;
        }
        if let Some(cell) = self.buf.cell_mut((self.area.x + x, self.area.y + y)) {
            cell.set_char(symbol).set_style(style);
        }
    }

    fn text(&mut self, x: u16, y: i32, text: &str, style: Style) {
        let Ok(y) = u16::try_from(y - i32::from(self.skip)) else {
            return;
        };
        if y >= self.area.height || x >= self.area.width {
            return;
        }
        let max = usize::from(self.area.width - x);
        self.buf
            .set_stringn(self.area.x + x, self.area.y + y, text, max, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::party::INTRO;

    fn render(stage: Stage, depth: ColorDepth, name: Option<&str>, age: Option<u8>) -> Buffer {
        let area = Rect::new(0, 0, WIDTH, HEIGHT);
        let mut buf = Buffer::empty(area);
        let theme = Theme::new(depth, false);
        Cake {
            stage,
            theme: &theme,
            name,
            age,
        }
        .render(area, &mut buf);
        buf
    }

    fn count(buf: &Buffer, symbol: &str) -> usize {
        buf.content()
            .iter()
            .filter(|c| c.symbol() == symbol)
            .count()
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn templates_fit() {
        assert!(TIERS.iter().all(|row| row.len() == CAKE_WIDTH as usize));
        assert!(INTRO >= ms(LIGHT_MS + LIGHT_STEP_MS));
    }

    #[test]
    fn name_label_spacing_and_fallback() {
        assert_eq!(name_label("Ada").as_deref(), Some(" A d a "));
        assert_eq!(name_label("小明").as_deref(), Some(" 小 明 "));
        // 字加空格放不下时去掉空格
        let long = "Bartholomew Smithers";
        assert_eq!(name_label(long), Some(format!(" {long} ")));
        // 实在放不下
        assert_eq!(name_label(&"长".repeat(16)), None);
    }

    #[test]
    fn even_width_name_is_exactly_centered() {
        let buf = render(Stage::Lit(ms(0)), ColorDepth::None, Some("东西西"), None);
        // 无颜色模式下巧克力层是 `#`，数名字两侧各有多少格
        let y = TIERS_TOP + NAME_ROW as u16;
        let left = MARGIN + BOTTOM_TIER_LEFT;
        let row: Vec<&str> = (left..left + BOTTOM_TIER_WIDTH)
            .map(|x| buf[(x, y)].symbol())
            .collect();
        let before = row.iter().take_while(|s| **s == "#").count();
        let after = row.iter().rev().take_while(|s| **s == "#").count();
        assert_eq!(before, after, "{}", row.concat());
    }

    /// 点着的烛焰数：数字蜡烛的火苗位置上是火苗字符。
    fn lit_flames(buf: &Buffer, age: Option<u8>) -> usize {
        Candles::new(age)
            .flames
            .iter()
            .filter(|&&(x, y)| ["^", "*", "'", "/", "("].contains(&buf[(MARGIN + x, y)].symbol()))
            .count()
    }

    #[test]
    fn age_becomes_digit_candles() {
        let pixels = |digit: usize| {
            DIGITS[digit]
                .iter()
                .map(|row| row.matches('#').count())
                .sum::<usize>()
        };
        let buf = render(Stage::Lit(ms(0)), ColorDepth::None, None, Some(51));
        assert_eq!(count(&buf, "@"), pixels(5) + pixels(1));
        // 两根数字蜡烛各有一簇火苗，左右对称地排在中间
        assert_eq!(lit_flames(&buf, Some(51)), 2);
        let flames = Candles::new(Some(51)).flames;
        assert_eq!(flames[0].0 + flames[1].0 + 1, CAKE_WIDTH);
        // 没有年龄时是一根普通蜡烛
        let plain = render(Stage::Lit(ms(0)), ColorDepth::None, None, None);
        assert_eq!(count(&plain, "@"), 0);
        assert_eq!(count(&plain, "|"), 4);
        assert_eq!(lit_flames(&plain, None), 1);
    }

    #[test]
    fn intro_stacks_the_cake_then_lights_candles() {
        let stage = |t| render(Stage::Stacking(ms(t)), ColorDepth::None, None, Some(51));
        // 一开始只有盘子
        let start = stage(0);
        assert_eq!(count(&start, "="), CAKE_WIDTH as usize);
        assert_eq!(
            count(&start, "#") + count(&start, "%") + count(&start, "@"),
            0
        );
        // 蛋糕都落好、蜡烛还没点
        let stacked = stage(LIGHT_MS - 1);
        assert!(count(&stacked, "#") > 0 && count(&stacked, "@") > 0);
        assert_eq!(lit_flames(&stacked, Some(51)), 0);
        // 逐一点亮
        assert_eq!(lit_flames(&stage(LIGHT_MS), Some(51)), 1);
        assert_eq!(lit_flames(&stage(LIGHT_MS + LIGHT_STEP_MS), Some(51)), 2);
    }

    #[test]
    fn short_area_drops_sky_not_plate() {
        let area = Rect::new(0, 0, WIDTH, HEIGHT - 2);
        let mut buf = Buffer::empty(area);
        let theme = Theme::new(ColorDepth::None, false);
        let stage = Stage::Lit(ms(0));
        Cake {
            stage,
            theme: &theme,
            name: None,
            age: Some(51),
        }
        .render(area, &mut buf);
        assert_eq!(buf[(MARGIN, HEIGHT - 3)].symbol(), "=");
    }

    #[test]
    fn loops_are_seamless() {
        let cycle = FRAME_TIME * FRAMES;
        for depth in [ColorDepth::TrueColor, ColorDepth::None] {
            for k in 0..FRAMES {
                let t = ms(2000) + FRAME_TIME * k;
                for stage in [Stage::Lit, Stage::Blown] {
                    let a = render(stage(t), depth, Some("Ada"), Some(51));
                    let b = render(stage(t + cycle), depth, Some("Ada"), Some(51));
                    assert_eq!(a, b);
                }
            }
            let lit = |t| render(Stage::Lit(t), depth, None, Some(51));
            assert_ne!(lit(ms(0)), lit(FRAME_TIME));
        }
    }

    #[test]
    fn blowing_out_makes_smoke_then_fireworks() {
        let blown = |t| render(Stage::Blown(t), ColorDepth::None, None, Some(51));
        let smoke = blown(FRAME_TIME * 2);
        let flames = Candles::new(Some(51)).flames;
        assert!(
            flames
                .iter()
                .all(|&(x, y)| !["^", "*", "'"].contains(&smoke[(MARGIN + x, y)].symbol())),
            "吹灭后不再有火苗"
        );
        assert!(count(&smoke, "(") + count(&smoke, ")") > 0);
        // 烟散了以后放烟花：第一朵在第 0 帧绽放，中心是 `*`
        let fireworks = blown(FIREWORKS_DELAY);
        let (cx, cy, _, _) = BURSTS[0];
        assert_eq!(fireworks[(cx as u16, cy as u16)].symbol(), "*");
    }
}
