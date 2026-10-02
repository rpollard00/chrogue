//! The Zobrist key that `State` keeps up to date must be the key of all its pieces.

mod common;

use chrogue_engine::fen::{KIWIPETE, START};
use chrogue_engine::rng::Rng;
use chrogue_engine::rules::FLAG_NAMES;
use chrogue_engine::zobrist::key_from_scratch;
use chrogue_engine::{Color, Move, Rules, SideRules, Special, State, outcome};
use common::from_fen;
use common::legal;

fn all_flags() -> Rules {
    let side = || SideRules::from_flags(FLAG_NAMES).unwrap();
    Rules::new(side(), side())
}

/// Plays random games. Returns the number of moves that the walk made.
fn walk(start: &State, rng: &mut Rng, games: u32) -> (u32, [u32; 4]) {
    let mut made = 0;
    // The numbers of en passant captures, castles, promotions, and backward steps.
    let mut special = [0; 4];
    for _ in 0..games {
        let mut state = start.clone();
        for _ in 0..200 {
            if outcome(&mut state).is_some() {
                break;
            }
            let moves = legal(&mut state);
            // Each legal move must give the key of its position, and its unmake must give the key back.
            let before = state.key();
            for &m in &moves {
                let undo = state.make(m);
                assert_eq!(state.key(), key_from_scratch(&state), "the key is wrong after make of {m:?}");
                state.unmake(m, undo);
                assert_eq!(state.key(), key_from_scratch(&state), "the key is wrong after unmake of {m:?}");
                assert_eq!(state.key(), before, "unmake of {m:?} did not restore the key");
            }
            let m = moves[rng.below(moves.len() as u64) as usize];
            special[0] += (m.special == Special::EnPassant) as u32;
            special[1] += (m.special == Special::Castle) as u32;
            special[2] += m.promo.is_some() as u32;
            special[3] += (m.special == Special::Backward) as u32;
            state.make(m);
            made += 1;
        }
    }
    (made, special)
}

#[test]
fn the_key_is_right_after_each_make_and_unmake_with_all_the_rule_flags() {
    let mut rng = Rng::new(2026);
    let mut made = 0;
    let mut special = [0; 4];
    for (fen, turn) in
        [(START, Color::White), (KIWIPETE, Color::White), ("r3k2r/1pp4p/8/8/8/8/PPP4P/R3K2R", Color::Black)]
    {
        let (count, kinds) = walk(&from_fen(fen, turn, all_flags()), &mut rng, 40);
        made += count;
        for (total, kind) in special.iter_mut().zip(kinds) {
            *total += kind;
        }
    }
    assert!(made > 3000, "the walk made only {made} moves");
    assert!(special.iter().all(|&count| count > 0), "the walk did not make each special move: {special:?}");
}

#[test]
fn the_key_is_right_with_the_rules_of_ordinary_chess() {
    let (made, _) = walk(&from_fen(KIWIPETE, Color::White, Rules::standard()), &mut Rng::new(7), 20);
    assert!(made > 500);
}

#[test]
fn the_key_has_the_side_to_move_the_en_passant_square_and_the_moved_flag() {
    let state = from_fen(START, Color::White, Rules::standard());
    let black = state.clone().with_turn(Color::Black);
    assert_ne!(state.key(), black.key());

    // After e2-e4 with no en passant square, and with the en passant square e3.
    let state = from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR", Color::Black, Rules::standard());
    let with_ep = state.clone().with_en_passant(Some(20)).unwrap();
    assert_ne!(state.key(), with_ep.key());
    assert_eq!(with_ep.key(), key_from_scratch(&with_ep));

    // The rook goes out and comes back. The pieces are on the same squares, but the rook
    // cannot castle now, thus the key is not the same.
    let mut state = from_fen("4k3/7p/8/8/8/8/7P/R3K3", Color::White, Rules::standard());
    let first = state.key();
    for (from, to) in [(0, 8), (60, 61), (8, 0), (61, 60)] {
        state.make(Move::new(from, to));
    }
    assert_eq!(state.turn(), Color::White);
    assert_ne!(state.key(), first);
}

#[test]
fn two_move_orders_to_the_same_position_give_the_same_key() {
    let start = from_fen(START, Color::White, Rules::standard());
    let play = |moves: [(u8, u8); 4]| {
        let mut state = start.clone();
        for (from, to) in moves {
            state.make(Move::new(from, to));
        }
        state
    };
    // Nf3 Nf6 Nc3 Nc6, and Nc3 Nc6 Nf3 Nf6.
    let a = play([(6, 21), (62, 45), (1, 18), (57, 42)]);
    let b = play([(1, 18), (57, 42), (6, 21), (62, 45)]);
    assert_eq!(a.key(), b.key());
    assert_ne!(a.key(), start.key());
}
