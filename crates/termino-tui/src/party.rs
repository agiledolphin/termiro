//! 彩蛋的播放进度：开场堆蛋糕、点亮蜡烛、吹灭蜡烛。

use std::time::Duration;

/// 开场动画的时长：蛋糕一层层落下堆好、蜡烛逐一点亮。
pub const INTRO: Duration = Duration::from_millis(2400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// 开场，参数是开场已经播放的时间。
    Stacking(Duration),
    /// 蜡烛点着，参数是点燃后经过的时间。
    Lit(Duration),
    /// 蜡烛已吹灭，参数是吹灭后经过的时间。
    Blown(Duration),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Party {
    /// 打开彩蛋后经过的时间。
    elapsed: Duration,
    /// 吹灭蜡烛的时间点。
    blown_at: Option<Duration>,
}

impl Party {
    pub fn tick(&mut self, dt: Duration) {
        self.elapsed += dt;
    }

    pub fn stage(&self) -> Stage {
        match self.blown_at {
            Some(at) => Stage::Blown(self.elapsed - at),
            None if self.elapsed < INTRO => Stage::Stacking(self.elapsed),
            None => Stage::Lit(self.elapsed - INTRO),
        }
    }

    /// 按空格：开场时跳过开场；蜡烛点着时吹灭；已吹灭时重新点燃。
    pub fn blow(&mut self) {
        match self.stage() {
            Stage::Stacking(_) => self.elapsed = INTRO,
            Stage::Lit(_) => self.blown_at = Some(self.elapsed),
            Stage::Blown(_) => {
                self.blown_at = None;
                self.elapsed = INTRO;
            }
        }
    }

    /// 生日歌只在蜡烛吹灭之前播放。
    pub fn music_wanted(&self) -> bool {
        !matches!(self.stage(), Stage::Blown(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn stages_follow_time_and_space_key() {
        let mut party = Party::default();
        assert_eq!(party.stage(), Stage::Stacking(Duration::ZERO));
        party.tick(INTRO + SECOND);
        assert_eq!(party.stage(), Stage::Lit(SECOND));
        assert!(party.music_wanted());

        party.blow();
        party.tick(SECOND);
        assert_eq!(party.stage(), Stage::Blown(SECOND));
        assert!(!party.music_wanted());

        // 再按一次重新点燃
        party.blow();
        assert_eq!(party.stage(), Stage::Lit(Duration::ZERO));
        assert!(party.music_wanted());
    }

    #[test]
    fn space_skips_intro() {
        let mut party = Party::default();
        party.tick(SECOND);
        party.blow();
        assert_eq!(party.stage(), Stage::Lit(Duration::ZERO));
    }
}
