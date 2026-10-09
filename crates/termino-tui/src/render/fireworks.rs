//! 吹灭蜡烛后的烟花：一朵朵从下方升空、在高处炸开，铺满整个终端画面。
//! 画在蛋糕下面一层，被蛋糕挡住的部分就藏在后面。

use std::f32::consts::TAU;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::Widget;

use super::cake::{FRAMES, frame_at};
use super::theme::Theme;
use crate::party::Stage;

/// 吹灭蜡烛后先冒一会儿烟，再开始放烟花。
pub const DELAY: Duration = Duration::from_millis(900);

/// 每朵烟花：(横向位置, 炸开的高度, 在一轮中升空的帧, 主色)。位置是占画面宽高的比例，
/// 画面越大烟花铺得越开；都放在两侧和上角，避开中间的蛋糕和蜡烛。
const BURSTS: [(f32, f32, u32, Color); 5] = [
    (0.13, 0.30, 0, Color::LightRed),
    (0.87, 0.20, 5, Color::Yellow),
    (0.22, 0.08, 10, Color::Cyan),
    (0.82, 0.48, 14, Color::LightMagenta),
    (0.10, 0.62, 19, Color::LightGreen),
];
/// 升空用的帧数和升起的行数。
const RISE_FRAMES: u32 = 3;
const RISE_ROWS: i32 = 6;
/// 炸开后持续的帧数；半径在前几帧扩到最大，之后边下垂边淡去。
const BURST_FRAMES: u32 = 9;
const GROW_FRAMES: u32 = 4;
/// 最大半径（行）。横向距离加倍，因为终端字符瘦长，这样烟花看起来是圆的。
const MAX_RADIUS: f32 = 6.0;
const RAYS: usize = 16;

const _: () = assert!(
    RISE_FRAMES + BURST_FRAMES <= FRAMES,
    "一朵烟花要在一轮内放完"
);

pub struct Fireworks<'a> {
    pub stage: Stage,
    pub theme: &'a Theme,
}

impl Widget for Fireworks<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Stage::Blown(t) = self.stage else {
            return;
        };
        let Some(since) = t.checked_sub(DELAY) else {
            return;
        };
        let frame = frame_at(since);
        let theme = self.theme;
        let mut put = |x: i32, y: i32, symbol: char, style: Style| {
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                return;
            };
            if x < area.width && y < area.height {
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char(symbol).set_style(style);
                }
            }
        };

        for &(fx, fy, start, color) in &BURSTS {
            let cx = (fx * f32::from(area.width)) as i32;
            let cy = (fy * f32::from(area.height)) as i32;
            let age = (frame + FRAMES - start) % FRAMES;

            // 升空：一道火光拖着尾巴往上升
            if age < RISE_FRAMES {
                let y = cy + RISE_ROWS * (RISE_FRAMES - age) as i32 / RISE_FRAMES as i32;
                put(cx, y, '|', theme.fg(color).bold());
                put(cx, y + 1, '.', theme.fg(color).dim());
                continue;
            }
            let burst = age - RISE_FRAMES;
            if burst >= BURST_FRAMES {
                continue;
            }

            // 炸开的一瞬间：中心一团亮光
            if burst == 0 {
                put(cx, cy, '*', theme.fg(Color::White).bold());
                for (dx, dy) in [(-2, 0), (2, 0), (0, -1), (0, 1)] {
                    put(cx + dx, cy + dy, '+', theme.fg(color).bold());
                }
                continue;
            }

            let radius = MAX_RADIUS * burst.min(GROW_FRAMES) as f32 / GROW_FRAMES as f32;
            // 后半段受重力往下垂
            let droop = (burst as i32 - GROW_FRAMES as i32) / 2;
            let droop = droop.max(0);
            let (symbol, bright) = match burst {
                1 | 2 => ('*', true),
                3 | 4 => ('+', true),
                5 | 6 => ('+', false),
                _ => ('.', false),
            };
            for ray in 0..RAYS {
                let angle = TAU * ray as f32 / RAYS as f32;
                let (sin, cos) = angle.sin_cos();
                let at = |r: f32| {
                    let x = cx + (cos * r * 2.0).round() as i32;
                    let y = cy + (sin * r).round() as i32 + droop;
                    (x, y)
                };
                // 主色和白色交替，看起来在闪
                let ray_color = if ray % 2 == 0 { color } else { Color::White };
                let style = if bright {
                    theme.fg(ray_color).bold()
                } else {
                    theme.fg(ray_color).dim()
                };
                let (x, y) = at(radius);
                put(x, y, symbol, style);
                // 扩散过程中内圈拖着火星
                if (2..=5).contains(&burst) {
                    let (x, y) = at(radius * 0.55);
                    put(x, y, '.', theme.fg(color).dim());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ColorDepth;

    const FRAME: Duration = Duration::from_millis(150);

    fn render(t: Duration, width: u16, height: u16) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let theme = Theme::new(ColorDepth::None, false);
        Fireworks {
            stage: Stage::Blown(t),
            theme: &theme,
        }
        .render(area, &mut buf);
        buf
    }

    fn drawn(buf: &Buffer) -> usize {
        buf.content().iter().filter(|c| c.symbol() != " ").count()
    }

    #[test]
    fn waits_for_the_smoke_to_clear() {
        assert_eq!(drawn(&render(DELAY - FRAME, 80, 24)), 0);
        assert!(drawn(&render(DELAY, 80, 24)) > 0);
    }

    #[test]
    fn bursts_reach_full_size() {
        // 第一朵烟花在第 0 帧升空，升空 3 帧后炸开，再过 4 帧半径最大
        let t = DELAY + FRAME * (RISE_FRAMES + GROW_FRAMES);
        let buf = render(t, 80, 24);
        let (fx, fy, _, _) = BURSTS[0];
        let (cx, cy) = ((fx * 80.0) as u16, (fy * 24.0) as u16);
        // 正右方那条射线到了 12 列外，正下方那条到了 6 行外
        assert_eq!(buf[(cx + 12, cy)].symbol(), "+");
        assert_eq!(buf[(cx, cy + 6)].symbol(), "+");
    }

    #[test]
    fn loops_seamlessly() {
        let cycle = FRAME * FRAMES;
        for k in 0..FRAMES {
            let t = DELAY + FRAME * k;
            assert_eq!(render(t, 80, 24), render(t + cycle, 80, 24));
        }
    }

    #[test]
    fn small_areas_are_fine() {
        for k in 0..FRAMES {
            render(DELAY + FRAME * k, 6, 3);
        }
    }
}
