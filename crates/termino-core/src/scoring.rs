//! Guideline 计分（消行、T-Spin、Back-to-Back、连击、下落），以及等级与重力。

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TSpin {
    Mini,
    Full,
}

/// 一次锁定带来的消行或 T-Spin（T-Spin 可以不消行）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clear {
    pub lines: u8,
    pub t_spin: Option<TSpin>,
    /// 与上一次消行同为高难度消行（四消或带消行的 T-Spin），基础分 ×1.5。
    pub back_to_back: bool,
    /// 连续第几次消行，第一次为 0。
    pub combo: u32,
    /// 本次得分，已乘等级。
    pub points: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Scoring {
    pub(crate) score: u64,
    /// 连续消行的次数减一；上一次锁定没有消行时为 `None`。
    combo: Option<u32>,
    /// 上一次消行是否为高难度消行。
    last_was_difficult: bool,
}

impl Scoring {
    /// 记录一次锁定。消了行或做出 T-Spin 时返回结果。
    pub(crate) fn lock(&mut self, lines: u8, t_spin: Option<TSpin>, level: u32) -> Option<Clear> {
        if lines == 0 {
            self.combo = None;
            // 不消行的 T-Spin 照样得分，但不影响 Back-to-Back
            t_spin?;
        }

        let mut back_to_back = false;
        let mut combo = 0;
        if lines > 0 {
            let difficult = lines == 4 || t_spin.is_some();
            back_to_back = difficult && self.last_was_difficult;
            self.last_was_difficult = difficult;
            combo = self.combo.map_or(0, |c| c + 1);
            self.combo = Some(combo);
        }

        let mut points = action_points(lines, t_spin) * level;
        if back_to_back {
            points = points * 3 / 2;
        }
        points += 50 * combo * level;
        self.score += u64::from(points);

        Some(Clear {
            lines,
            t_spin,
            back_to_back,
            combo,
            points,
        })
    }

    pub(crate) fn soft_drop(&mut self, rows: u32) {
        self.score += u64::from(rows);
    }

    pub(crate) fn hard_drop(&mut self, rows: u32) {
        self.score += 2 * u64::from(rows);
    }
}

/// 不含等级倍率的基础分。
fn action_points(lines: u8, t_spin: Option<TSpin>) -> u32 {
    match (t_spin, lines) {
        (None, 0) => 0,
        (None, 1) => 100,
        (None, 2) => 300,
        (None, 3) => 500,
        (None, _) => 800,
        (Some(TSpin::Mini), 0) => 100,
        (Some(TSpin::Mini), 1) => 200,
        (Some(TSpin::Mini), _) => 400,
        (Some(TSpin::Full), 0) => 400,
        (Some(TSpin::Full), 1) => 800,
        (Some(TSpin::Full), 2) => 1200,
        (Some(TSpin::Full), _) => 1600,
    }
}

/// 每消 10 行升一级。
pub(crate) fn level(start_level: u32, lines: u32) -> u32 {
    start_level.max(1) + lines / 10
}

/// Guideline 重力曲线 `(0.8 - (level - 1) * 0.007) ^ (level - 1)` 秒/行，预先算好的微秒值。
/// 不在运行时用浮点计算，是因为不同平台的浮点结果可能有细微差异，会破坏回放的确定性。
const GRAVITY_MICROS: [u64; 20] = [
    1_000_000, 793_000, 617_796, 472_729, 355_197, 262_004, 189_677, 134_735, 93_882, 64_152,
    42_976, 28_218, 18_153, 11_439, 7_059, 4_264, 2_520, 1_457, 824, 455,
];

/// 某等级下自然下落一行的时间。20 级以上保持 20 级的速度。
pub(crate) fn gravity(level: u32) -> Duration {
    let index = level.clamp(1, GRAVITY_MICROS.len() as u32) as usize - 1;
    Duration::from_micros(GRAVITY_MICROS[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(scoring: &mut Scoring, lines: u8, t_spin: Option<TSpin>) -> u32 {
        scoring.lock(lines, t_spin, 1).map_or(0, |c| c.points)
    }

    #[test]
    fn plain_lock_scores_nothing() {
        assert_eq!(Scoring::default().lock(0, None, 1), None);
    }

    #[test]
    fn line_clears_scale_with_level() {
        let mut s = Scoring::default();
        let tetris = s.lock(4, None, 3).unwrap();
        assert_eq!(tetris.points, 2400);
        assert_eq!(s.score, 2400);
    }

    #[test]
    fn back_to_back_needs_consecutive_difficult_clears() {
        let mut s = Scoring::default();
        assert_eq!(points(&mut s, 4, None), 800);
        // 两次四消之间只有不消行的锁定，B2B 不中断
        assert_eq!(points(&mut s, 0, None), 0);
        let second = s.lock(4, None, 1).unwrap();
        assert!(second.back_to_back);
        assert_eq!(second.points, 1200);
        // 普通消行打断 B2B
        points(&mut s, 0, None);
        assert_eq!(points(&mut s, 1, None), 100);
        assert!(!s.lock(4, None, 1).unwrap().back_to_back);
    }

    #[test]
    fn t_spin_without_lines_keeps_back_to_back() {
        let mut s = Scoring::default();
        points(&mut s, 2, Some(TSpin::Full));
        points(&mut s, 0, None);
        assert_eq!(points(&mut s, 0, Some(TSpin::Full)), 400);
        let tetris = s.lock(4, None, 1).unwrap();
        assert!(tetris.back_to_back);
    }

    #[test]
    fn combo_adds_fifty_per_step() {
        let mut s = Scoring::default();
        let combos: Vec<_> = (0..3).map(|_| s.lock(1, None, 1).unwrap()).collect();
        assert_eq!(
            combos.iter().map(|c| c.combo).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(
            combos.iter().map(|c| c.points).collect::<Vec<_>>(),
            [100, 150, 200]
        );
        // 不消行的锁定中断连击
        s.lock(0, None, 1);
        assert_eq!(s.lock(1, None, 1).unwrap().combo, 0);
    }

    #[test]
    fn drops_score_per_row() {
        let mut s = Scoring::default();
        s.soft_drop(3);
        s.hard_drop(10);
        assert_eq!(s.score, 23);
    }

    #[test]
    fn level_and_gravity() {
        assert_eq!(level(1, 0), 1);
        assert_eq!(level(1, 19), 2);
        assert_eq!(level(5, 10), 6);
        assert_eq!(gravity(1), Duration::from_secs(1));
        assert_eq!(gravity(2), Duration::from_millis(793));
        assert_eq!(gravity(99), gravity(20));
        assert!((1..20).all(|l| gravity(l) > gravity(l + 1)));
    }
}
