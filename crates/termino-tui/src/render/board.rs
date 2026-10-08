use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::Widget;
use termino_core::{GameView, Pos, VISIBLE_HEIGHT};

use super::theme::{Cell, Theme};

/// 每个格子占 2 列，因为终端字符是瘦长的，这样方块才接近正方形。
pub const CELL_WIDTH: u16 = 2;
/// 含边框的尺寸。
pub const WIDTH: u16 = termino_core::WIDTH as u16 * CELL_WIDTH + 2;
pub const HEIGHT: u16 = VISIBLE_HEIGHT as u16 + 2;

/// 盘面：已锁定的方块、落点预览和当前方块，只画可见的 20 行。
pub struct BoardWidget<'a> {
    view: &'a GameView<'a>,
    theme: &'a Theme,
}

impl<'a> BoardWidget<'a> {
    pub fn new(view: &'a GameView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for BoardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = self
            .theme
            .bordered()
            .title(" TERMINO ")
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        block.render(area, buf);

        for y in 0..VISIBLE_HEIGHT {
            for x in 0..termino_core::WIDTH {
                let pos = Pos::new(x, y);
                let cell = match self.view.board.get(pos) {
                    Some(kind) => self.theme.block(kind),
                    None => self.theme.empty(),
                };
                paint(buf, inner, pos, cell);
            }
        }
        // 先画落点预览，与当前方块重叠时被覆盖
        if let Some(ghost) = self.view.ghost {
            for pos in ghost.cells() {
                paint(buf, inner, pos, self.theme.ghost(ghost.kind));
            }
        }
        if let Some(piece) = self.view.active {
            for pos in piece.cells() {
                paint(buf, inner, pos, self.theme.block(piece.kind));
            }
        }
    }
}

/// 在盘面坐标 `pos` 处画一个格子。不在可见区域内的格子直接忽略。
fn paint(buf: &mut Buffer, inner: Rect, pos: Pos, (symbol, style): Cell) {
    if !(0..VISIBLE_HEIGHT).contains(&pos.y) {
        return;
    }
    // 盘面 y 向上，屏幕 y 向下
    let x = inner.x + pos.x as u16 * CELL_WIDTH;
    let y = inner.y + (VISIBLE_HEIGHT - 1 - pos.y) as u16;
    buf.set_stringn(x, y, symbol, CELL_WIDTH as usize, style);
}
