use crate::piece::PieceKind;

/// SplitMix64。自己实现而不用 `rand` crate，是为了保证同一种子在任何依赖版本下
/// 都生成相同序列——回放和联机同步都依赖这一点。
#[derive(Clone, Debug)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `[0, n)` 内的整数。乘法取高位法，n 很小时偏差可以忽略。
    fn below(&mut self, n: usize) -> usize {
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }
}

/// 7-bag 随机器：每 7 个方块是 7 种方块的一个随机排列。
#[derive(Clone, Debug)]
pub struct SevenBag {
    rng: SplitMix64,
    bag: [PieceKind; 7],
    taken: usize,
}

impl SevenBag {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64(seed),
            bag: PieceKind::ALL,
            taken: 7,
        }
    }

    pub fn next_piece(&mut self) -> PieceKind {
        if self.taken == 7 {
            self.bag = PieceKind::ALL;
            // Fisher–Yates 洗牌
            for i in (1..7).rev() {
                let j = self.rng.below(i + 1);
                self.bag.swap(i, j);
            }
            self.taken = 0;
        }
        self.taken += 1;
        self.bag[self.taken - 1]
    }
}

impl Iterator for SevenBag {
    type Item = PieceKind;

    fn next(&mut self) -> Option<PieceKind> {
        Some(self.next_piece())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bag_is_a_permutation() {
        for seed in 0..50 {
            let pieces: Vec<_> = SevenBag::new(seed).take(7 * 100).collect();
            for bag in pieces.chunks(7) {
                let mut sorted = bag.to_vec();
                sorted.sort_by_key(|k| k.letter());
                let mut all = PieceKind::ALL.to_vec();
                all.sort_by_key(|k| k.letter());
                assert_eq!(sorted, all, "seed {seed}");
            }
        }
    }

    #[test]
    fn same_seed_same_sequence() {
        let a: Vec<_> = SevenBag::new(7).take(70).collect();
        let b: Vec<_> = SevenBag::new(7).take(70).collect();
        let c: Vec<_> = SevenBag::new(8).take(70).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// 锁定具体序列。若这里失败，说明随机算法变了，旧回放将无法复现；
    /// 确实需要修改时，请同时考虑回放文件的版本兼容。
    #[test]
    fn sequence_is_stable_across_versions() {
        let s: String = SevenBag::new(42).take(14).map(PieceKind::letter).collect();
        assert_eq!(s, "TSLZOIJJSITLZO");
    }
}
