//! 用随机输入序列检验 core 的不变量。

use std::time::Duration;

use proptest::prelude::*;
use termino_core::{Action, Board, Event, Game, HEIGHT, Pos, Rules, WIDTH};

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        Just(Action::MoveLeft),
        Just(Action::MoveRight),
        Just(Action::SoftDrop),
        Just(Action::HardDrop),
        Just(Action::RotateCw),
        Just(Action::RotateCcw),
        Just(Action::Hold),
    ]
}

/// 每一帧：若干动作 + 0~1200ms 的 dt。
fn frames() -> impl Strategy<Value = Vec<(Vec<Action>, u64)>> {
    prop::collection::vec((prop::collection::vec(action(), 0..4), 0u64..1200), 0..300)
}

fn has_full_row(board: &Board) -> bool {
    (0..HEIGHT).any(|y| (0..WIDTH).all(|x| board.get(Pos::new(x, y)).is_some()))
}

proptest! {
    #[test]
    fn invariants_hold_for_any_input(seed: u64, frames in frames()) {
        let mut game = Game::new(seed, Rules::default());
        let (mut locked, mut cleared) = (0i64, 0i64);
        let mut score = 0;

        for (actions, dt) in frames {
            let was_over = game.view().game_over.is_some();
            let events = game.update(Duration::from_millis(dt), &actions);
            if was_over {
                prop_assert!(events.is_empty());
            }
            for event in &events {
                match event {
                    Event::Locked(_) => locked += 1,
                    Event::Clear(clear) => cleared += i64::from(clear.lines),
                    Event::GameOver(_) => {}
                }
            }

            let view = game.view();
            prop_assert_eq!(view.board.filled_count() as i64, 4 * locked - 10 * cleared);
            prop_assert_eq!(i64::from(view.lines), cleared);
            prop_assert!(!has_full_row(view.board));
            prop_assert!(view.score >= score);
            score = view.score;
            prop_assert_eq!(view.level, 1 + view.lines / 10);
            prop_assert_eq!(view.active.is_none(), view.game_over.is_some());
            if let (Some(active), Some(ghost)) = (view.active, view.ghost) {
                prop_assert!(view.board.fits(active.cells()));
                prop_assert!(view.board.fits(ghost.cells()));
                prop_assert_eq!(ghost.origin.x, active.origin.x);
                prop_assert!(ghost.origin.y <= active.origin.y);
            }
        }
    }

    #[test]
    fn same_seed_and_input_replays_identically(seed: u64, frames in frames()) {
        let mut a = Game::new(seed, Rules::default());
        let mut b = Game::new(seed, Rules::default());
        for (actions, dt) in frames {
            let dt = Duration::from_millis(dt);
            prop_assert_eq!(a.update(dt, &actions), b.update(dt, &actions));
        }
        let (va, vb) = (a.view(), b.view());
        prop_assert_eq!(va.board, vb.board);
        prop_assert_eq!(va.active, vb.active);
        prop_assert_eq!(va.next, vb.next);
        prop_assert_eq!(va.hold, vb.hold);
        prop_assert_eq!(va.score, vb.score);
    }
}
