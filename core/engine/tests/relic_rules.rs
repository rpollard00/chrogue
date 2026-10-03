//! The six rule flags, the moves from one square, and the results of a battle.

mod common;

use chrogue_engine::fen::{START, square};
use chrogue_engine::{Color, Outcome, Rules, SideRules, Special, in_check, outcome};
use common::from_fen;
use common::{legal, squares, targets, targets_from};

fn white(rules: SideRules) -> Rules {
    Rules::new(rules, SideRules::standard())
}

#[test]
fn forced_march_lets_a_moved_pawn_move_two_squares() {
    let fen = "4k3/7p/8/8/8/4P3/8/4K3";
    assert_eq!(targets(&mut from_fen(fen, Color::White, Rules::standard()), "e3"), squares(&["e4"]));
    let mut state = from_fen(fen, Color::White, white(SideRules::standard().forced_march()));
    assert_eq!(targets(&mut state, "e3"), squares(&["e4", "e5"]));
}

#[test]
fn backpedal_lets_a_pawn_move_backward_to_an_empty_square() {
    let mut state = from_fen("4k3/7p/8/8/8/4P3/8/4K3", Color::White, white(SideRules::standard().backpedal()));
    assert_eq!(targets(&mut state, "e3"), squares(&["e2", "e4"]));
}

#[test]
fn early_promo_promotes_on_the_seventh_rank() {
    let mut state = from_fen("4k3/7p/4P3/8/8/8/8/4K3", Color::White, white(SideRules::standard().early_promo()));
    let moves: Vec<_> = legal(&mut state).into_iter().filter(|m| m.from == square("e6")).collect();
    assert_eq!(moves.len(), 4);
    assert!(moves.iter().all(|m| m.promo.is_some() && m.to == square("e7")));
}

#[test]
fn king_knight_lets_the_king_move_and_give_check_as_a_knight() {
    let rules = || white(SideRules::standard().king_knight());
    let mut state = from_fen("4k3/7p/8/8/8/8/8/4K2P", Color::White, rules());
    assert!(targets(&mut state, "e1").contains(&square("f3")));
    assert!(in_check(&from_fen("4k3/7p/3K4/8/8/8/8/7P", Color::Black, rules()), Color::Black));
    // The same position with ordinary rules has no check.
    assert!(!in_check(&from_fen("4k3/7p/3K4/8/8/8/8/7P", Color::Black, Rules::standard()), Color::Black));
}

#[test]
fn long_leap_adds_the_long_knight_jump() {
    let mut state = from_fen("4k3/7p/8/8/8/8/8/N3K3", Color::White, white(SideRules::standard().long_leap()));
    assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c2", "b4", "d2"]));
}

#[test]
fn sidestep_moves_a_bishop_one_square_without_a_capture() {
    let mut state = from_fen("4k3/8/8/8/8/p7/P7/B3K3", Color::White, white(SideRules::standard().sidestep()));
    assert!(targets(&mut state, "a1").contains(&square("b1")));
    assert!(!targets(&mut state, "a1").contains(&square("a2")));
}

#[test]
fn moves_from_gives_the_moves_of_a_piece_of_the_side_that_does_not_have_the_move() {
    let rules = Rules::new(SideRules::standard(), SideRules::standard().long_leap());
    let leap = from_fen("4k3/8/8/3n4/8/8/8/4K3", Color::White, rules);
    assert_eq!(targets_from(&leap, "d5").len(), 15);
    assert!(targets_from(&leap, "d5").contains(&square("a4")));
    assert!(targets_from(&leap, "a1").is_empty());

    // The rook on e7 is pinned to its king, thus it stays on the e-file.
    let pin = from_fen("4k3/4r3/8/8/8/8/8/4RK2", Color::White, Rules::standard());
    assert_eq!(targets_from(&pin, "e7"), squares(&["e6", "e5", "e4", "e3", "e2", "e1"]));

    let state = from_fen("4k3/8/8/3pP3/8/8/8/4K3", Color::White, Rules::standard());
    let state = state.with_en_passant(Some(square("d6"))).unwrap();
    assert_eq!(targets_from(&state, "d5"), squares(&["d4"]));
    assert_eq!(state.turn(), Color::White);
    assert_eq!(state.ep(), Some(square("d6")));
    assert_eq!(targets_from(&state, "e5"), squares(&["d6", "e6"]));
}

#[test]
fn outcome_finds_checkmate_stalemate_and_a_lone_king() {
    let result = |fen: &str, turn: Color| outcome(&mut from_fen(fen, turn, Rules::standard()));
    assert_eq!(result("R5k1/5ppp/8/8/8/8/8/4K3", Color::Black), Some(Outcome::Checkmate { winner: Color::White }));
    assert_eq!(result("7k/5Q2/8/8/8/8/8/4K2p", Color::Black), Some(Outcome::Stalemate { winner: Color::White }));
    assert_eq!(result("7k/8/8/8/8/8/8/4K2P", Color::Black), Some(Outcome::Rout { winner: Color::White }));
    assert_eq!(result(START, Color::White), None);
}

#[test]
fn outcome_finds_two_lone_kings_and_the_clock() {
    let result = |fen: &str, turn: Color| outcome(&mut from_fen(fen, turn, Rules::standard()));
    assert_eq!(result("7k/8/8/8/8/8/8/4K3", Color::White), Some(Outcome::Bare));
    assert_eq!(result("7k/7p/8/8/8/8/8/4K3", Color::White), Some(Outcome::Rout { winner: Color::Black }));

    let mut state = from_fen("7k/7p/8/8/8/8/P7/4K3", Color::White, Rules::standard());
    state.clock = 99;
    assert_eq!(outcome(&mut state), None);
    state.clock = 100;
    assert_eq!(outcome(&mut state), Some(Outcome::Clock));
}

#[test]
fn only_a_capture_resets_the_clock() {
    // The pawn on e3 can step back, advance, or capture the knight on d4.
    let mut state = from_fen("4k3/7p/8/8/3n4/4P3/8/4K3", Color::White, white(SideRules::standard().backpedal()));
    state.clock = 7;
    let moves = legal(&mut state);
    let to = |name: &str| *moves.iter().find(|m| m.from == square("e3") && m.to == square(name)).unwrap();
    let king = *moves.iter().find(|m| m.from == square("e1")).unwrap();
    for (m, clock) in [(to("e2"), 8), (to("e4"), 8), (king, 8), (to("d4"), 0)] {
        let undo = state.make(m);
        assert_eq!(state.clock, clock);
        state.unmake(m, undo);
        assert_eq!(state.clock, 7);
    }
}

#[test]
fn a_double_step_into_the_promotion_zone_promotes_and_makes_no_en_passant_square() {
    let rules = white(SideRules::standard().forced_march().early_promo());
    let mut state = from_fen("4k3/7p/8/4P3/8/8/8/4K3", Color::White, rules);
    let moves: Vec<_> = legal(&mut state).into_iter().filter(|m| m.to == square("e7")).collect();
    assert_eq!(moves.len(), 4);
    assert!(moves.iter().all(|m| m.promo.is_some() && m.special == Special::None));
    state.make(moves[0]);
    assert_eq!(state.ep(), None);
}

#[test]
fn a_side_with_no_king_is_never_in_check() {
    let state = from_fen("8/8/8/8/8/8/r7/R6K", Color::Black, Rules::standard());
    assert!(!in_check(&state, Color::Black));
    assert!(!in_check(&state, Color::White));
}
