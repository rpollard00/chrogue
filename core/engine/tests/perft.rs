//! Perft counts for ordinary chess, and a walk that compares the state after each unmake with the state before the move.

mod common;

use chrogue_engine::fen::{KIWIPETE, START};
use chrogue_engine::{Color, MoveList, Rules, SideRules, State, perft, pseudo_moves};
use common::from_fen;

fn start() -> State {
    from_fen(START, Color::White, Rules::standard())
}

fn kiwipete() -> State {
    from_fen(KIWIPETE, Color::White, Rules::standard())
}

#[test]
fn perft_from_the_start_position() {
    assert_eq!(perft(&mut start(), 1), 20);
    assert_eq!(perft(&mut start(), 2), 400);
    assert_eq!(perft(&mut start(), 3), 8902);
    assert_eq!(perft(&mut start(), 4), 197281);
    assert_eq!(perft(&mut start(), 5), 4865609);
}

/// This test is slow without `--release`. Run it with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn perft_from_the_start_position_at_depth_6() {
    assert_eq!(perft(&mut start(), 6), 119060324);
}

#[test]
fn perft_with_castling_en_passant_and_promotion() {
    assert_eq!(perft(&mut kiwipete(), 1), 48);
    assert_eq!(perft(&mut kiwipete(), 2), 2039);
    assert_eq!(perft(&mut kiwipete(), 3), 97862);
    assert_eq!(perft(&mut kiwipete(), 4), 4085603);
}

/// Walks the move tree and compares the full state after each unmake.
fn walk(state: &mut State, depth: u32) {
    if depth == 0 {
        return;
    }
    let before = state.clone();
    let mut list = MoveList::new();
    pseudo_moves(state, state.turn(), false, &mut list);
    for &m in &list {
        let undo = state.make(m);
        assert!(state.is_consistent(), "the bitboards do not agree with the mailbox after {m:?}");
        walk(state, depth - 1);
        state.unmake(m, undo);
        assert!(*state == before, "unmake of {m:?} did not restore the state");
    }
}

#[test]
fn unmake_restores_the_state() {
    let mut state = kiwipete();
    let before = state.clone();
    assert_eq!(perft(&mut state, 3), 97862);
    assert!(state == before);
    assert!(state.is_consistent());
    walk(&mut state, 3);
    assert!(state == before);
}

#[test]
fn unmake_restores_the_state_with_all_the_relic_rules() {
    let side = || SideRules::from_flags(chrogue_engine::rules::FLAG_NAMES).unwrap();
    // The pawns are near promotion, thus the walk has promotions, en passant captures, and backward steps.
    let mut state = from_fen("r3k2r/1P4P1/8/2pP4/4Pp2/8/1p4p1/R3K2R", Color::White, Rules::new(side(), side()));
    let before = state.clone();
    walk(&mut state, 3);
    assert!(state == before);
    let mut state = state.with_turn(Color::Black);
    let before = state.clone();
    walk(&mut state, 3);
    assert!(state == before);
}
