use std::ops::Add;

/// 盘面坐标。x 向右、y 向上，`(0, 0)` 是左下角。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl Add for Pos {
    type Output = Pos;

    fn add(self, other: Pos) -> Pos {
        Pos::new(self.x + other.x, self.y + other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl PieceKind {
    pub const ALL: [PieceKind; 7] = [
        Self::I,
        Self::O,
        Self::T,
        Self::S,
        Self::Z,
        Self::J,
        Self::L,
    ];

    pub const fn letter(self) -> char {
        match self {
            Self::I => 'I',
            Self::O => 'O',
            Self::T => 'T',
            Self::S => 'S',
            Self::Z => 'Z',
            Self::J => 'J',
            Self::L => 'L',
        }
    }

    pub const fn from_letter(letter: char) -> Option<Self> {
        Some(match letter {
            'I' => Self::I,
            'O' => Self::O,
            'T' => Self::T,
            'S' => Self::S,
            'Z' => Self::Z,
            'J' => Self::J,
            'L' => Self::L,
            _ => return None,
        })
    }

    /// 旋转包围盒的边长。SRS 中每种方块都在自己的包围盒内做纯旋转。
    const fn box_size(self) -> i32 {
        match self {
            Self::I => 4,
            Self::O => 2,
            _ => 3,
        }
    }

    /// 出生朝向下的格子，坐标相对包围盒左下角。
    const fn spawn_cells(self) -> [Pos; 4] {
        const fn p(x: i32, y: i32) -> Pos {
            Pos::new(x, y)
        }
        match self {
            Self::I => [p(0, 2), p(1, 2), p(2, 2), p(3, 2)],
            Self::O => [p(0, 0), p(1, 0), p(0, 1), p(1, 1)],
            Self::T => [p(0, 1), p(1, 1), p(2, 1), p(1, 2)],
            Self::S => [p(0, 1), p(1, 1), p(1, 2), p(2, 2)],
            Self::Z => [p(0, 2), p(1, 2), p(1, 1), p(2, 1)],
            Self::J => [p(0, 2), p(0, 1), p(1, 1), p(2, 1)],
            Self::L => [p(2, 2), p(0, 1), p(1, 1), p(2, 1)],
        }
    }

    /// 指定朝向下的格子，坐标相对包围盒左下角。
    pub fn cells(self, rotation: Rotation) -> [Pos; 4] {
        let n = self.box_size();
        let mut cells = self.spawn_cells();
        for _ in 0..rotation as u8 {
            // 在包围盒内顺时针转 90°（y 向上）
            for c in &mut cells {
                *c = Pos::new(c.y, n - 1 - c.x);
            }
        }
        cells
    }
}

/// SRS 的四个朝向，依次为 0、R、2、L。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Rotation {
    #[default]
    Spawn = 0,
    Right = 1,
    Reverse = 2,
    Left = 3,
}

impl Rotation {
    pub const fn cw(self) -> Self {
        match self {
            Self::Spawn => Self::Right,
            Self::Right => Self::Reverse,
            Self::Reverse => Self::Left,
            Self::Left => Self::Spawn,
        }
    }

    pub const fn ccw(self) -> Self {
        match self {
            Self::Spawn => Self::Left,
            Self::Right => Self::Spawn,
            Self::Reverse => Self::Right,
            Self::Left => Self::Reverse,
        }
    }
}

/// 盘面上的一个方块：种类、朝向，以及包围盒左下角的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub kind: PieceKind,
    pub rotation: Rotation,
    pub origin: Pos,
}

impl Piece {
    pub fn cells(&self) -> [Pos; 4] {
        self.kind.cells(self.rotation).map(|c| c + self.origin)
    }

    pub(crate) fn shifted(self, dx: i32, dy: i32) -> Self {
        Self {
            origin: self.origin + Pos::new(dx, dy),
            ..self
        }
    }

    /// 最低格子所在的行。
    pub(crate) fn bottom(&self) -> i32 {
        self.cells().iter().map(|c| c.y).min().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut cells: [Pos; 4]) -> [Pos; 4] {
        cells.sort_by_key(|c| (c.y, c.x));
        cells
    }

    fn ps(coords: [(i32, i32); 4]) -> [Pos; 4] {
        sorted(coords.map(|(x, y)| Pos::new(x, y)))
    }

    #[test]
    fn t_rotations_match_srs() {
        let t = PieceKind::T;
        assert_eq!(
            sorted(t.cells(Rotation::Right)),
            ps([(1, 0), (1, 1), (1, 2), (2, 1)])
        );
        assert_eq!(
            sorted(t.cells(Rotation::Reverse)),
            ps([(0, 1), (1, 1), (2, 1), (1, 0)])
        );
        assert_eq!(
            sorted(t.cells(Rotation::Left)),
            ps([(1, 0), (1, 1), (1, 2), (0, 1)])
        );
    }

    #[test]
    fn i_rotations_match_srs() {
        let i = PieceKind::I;
        assert_eq!(
            sorted(i.cells(Rotation::Right)),
            ps([(2, 0), (2, 1), (2, 2), (2, 3)])
        );
        assert_eq!(
            sorted(i.cells(Rotation::Reverse)),
            ps([(0, 1), (1, 1), (2, 1), (3, 1)])
        );
        assert_eq!(
            sorted(i.cells(Rotation::Left)),
            ps([(1, 0), (1, 1), (1, 2), (1, 3)])
        );
    }

    #[test]
    fn o_does_not_wobble() {
        let spawn = sorted(PieceKind::O.cells(Rotation::Spawn));
        for r in [Rotation::Right, Rotation::Reverse, Rotation::Left] {
            assert_eq!(sorted(PieceKind::O.cells(r)), spawn);
        }
    }

    #[test]
    fn cw_and_ccw_are_inverse() {
        for r in [
            Rotation::Spawn,
            Rotation::Right,
            Rotation::Reverse,
            Rotation::Left,
        ] {
            assert_eq!(r.cw().ccw(), r);
            assert_eq!(r.cw().cw().cw().cw(), r);
        }
    }

    #[test]
    fn letters_round_trip() {
        for kind in PieceKind::ALL {
            assert_eq!(PieceKind::from_letter(kind.letter()), Some(kind));
        }
        assert_eq!(PieceKind::from_letter('.'), None);
    }
}
