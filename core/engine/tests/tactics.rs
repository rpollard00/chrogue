//! Positions that the search must solve within a node budget. Each case tells its budget.
//!
//! The results of this game are not those of chess: a side with only its king loses (rout),
//! and a side with no legal move loses with or without a check. Thus each side of a position
//! has one piece or more that is not the king.

mod common;

use chrogue_engine::fen::square;
use chrogue_engine::{
    Color, Kind, Level, MATE, MATE_BOUND, Move, MoveList, Outcome, Rules, SearchResult, SideRules, Special, State,
    choose_move, legal_moves, outcome,
};
use common::from_fen;

const WHITE: Color = Color::White;
const BLACK: Color = Color::Black;

fn flags(names: &[&str]) -> SideRules {
    SideRules::from_flags(names.iter().copied()).unwrap()
}

fn standard() -> Rules {
    Rules::standard()
}

/// Rules with the flags for White only.
fn white_has(names: &[&str]) -> Rules {
    Rules::new(flags(names), SideRules::standard())
}

/// Searches the position with a node budget. The search must not change the state.
fn solve(state: &mut State, nodes: u64) -> SearchResult {
    let before = state.clone();
    let result = choose_move(state, &Level::nodes("test", nodes), 1).expect("the position has a legal move");
    assert!(*state == before, "the search changed the state");
    assert!(result.nodes <= nodes, "the search used {} nodes of {nodes}", result.nodes);
    assert_eq!(result.pv.first(), Some(&result.mv));
    result
}

fn solve_fen(fen: &str, turn: Color, rules: Rules, nodes: u64) -> (State, SearchResult) {
    let mut state = from_fen(fen, turn, rules);
    let result = solve(&mut state, nodes);
    (state, result)
}

fn mv(from: &str, to: &str) -> (u8, u8) {
    (square(from), square(to))
}

fn squares_of(m: Move) -> (u8, u8) {
    (m.from, m.to)
}

/// The outcome after the move of the result.
fn outcome_after(state: &State, result: &SearchResult) -> Option<Outcome> {
    let mut next = state.clone();
    next.make(result.mv);
    outcome(&mut next)
}

/// True if the side that has the move can end the battle as the winner with one move.
fn can_win_at_once(state: &State) -> bool {
    let mut state = state.clone();
    let us = state.turn();
    let mut list = MoveList::new();
    legal_moves(&mut state, &mut list);
    list.iter().any(|&m| {
        let undo = state.make(m);
        let won = outcome(&mut state).is_some_and(|end| end.winner() == Some(us));
        state.unmake(m, undo);
        won
    })
}

/// A score of a forced win in `plies` half moves.
fn win_in(plies: i32) -> i32 {
    MATE - plies
}

// ---- Mates of ordinary chess ----

#[test]
fn mate_in_1_on_the_back_rank() {
    let (state, result) = solve_fen("6k1/5ppp/8/8/8/8/5PPP/R5K1", WHITE, standard(), 1_000);
    assert_eq!(squares_of(result.mv), mv("a1", "a8"));
    assert_eq!(result.score, win_in(1));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Checkmate { winner: WHITE }));
}

#[test]
fn mate_in_1_for_black() {
    let (state, result) = solve_fen("r5k1/5ppp/8/8/8/8/5PPP/6K1", BLACK, standard(), 1_000);
    assert_eq!(squares_of(result.mv), mv("a8", "a1"));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Checkmate { winner: BLACK }));
}

#[test]
fn mate_in_2_with_two_rooks() {
    // The rook goes to the 7th rank, and then the other rook mates on the 8th rank.
    // Black has three pieces, thus a rout is slower than the mate.
    let (_, result) = solve_fen("7k/p7/1p6/p7/8/8/3R4/2R3K1", WHITE, standard(), 20_000);
    assert_eq!(result.score, win_in(3));
    assert!([mv("c1", "c7"), mv("d2", "d7")].contains(&squares_of(result.mv)), "{:?}", result.mv);
    assert_eq!(result.pv.len(), 3);
}

#[test]
fn mate_in_2_for_black() {
    let (_, result) = solve_fen("2r3k1/3r4/8/8/P7/1P6/P7/7K", BLACK, standard(), 20_000);
    assert_eq!(result.score, win_in(3));
    assert!([mv("d7", "d2"), mv("c8", "c2")].contains(&squares_of(result.mv)), "{:?}", result.mv);
}

#[test]
fn mate_in_3_with_two_rooks() {
    let (_, result) = solve_fen("8/p6k/1p6/p7/8/8/3R4/2R3K1", WHITE, standard(), 100_000);
    assert_eq!(result.score, win_in(5));
    assert_eq!(result.pv.len(), 5);
}

#[test]
fn a_forced_win_is_played_to_its_end() {
    // The search plays the two sides of the mate in 3. The battle ends after 5 half moves.
    let mut state = from_fen("8/p6k/1p6/p7/8/8/3R4/2R3K1", WHITE, standard());
    let mut plies = 0;
    while outcome(&mut state).is_none() {
        let result = solve(&mut state, 100_000);
        state.make(result.mv);
        plies += 1;
        assert!(plies <= 5, "the mate in 3 took more than 5 half moves");
    }
    assert_eq!(outcome(&mut state), Some(Outcome::Checkmate { winner: WHITE }));
}

// ---- Material ----

#[test]
fn a_knight_fork_wins_the_queen() {
    let (_, result) = solve_fen("4k3/7p/8/1q1N4/8/8/7P/6K1", WHITE, standard(), 20_000);
    assert_eq!(squares_of(result.mv), mv("d5", "c7"));
    assert!(result.score > 400, "score {}", result.score);
}

#[test]
fn a_rook_skewer_wins_the_queen() {
    let (_, result) = solve_fen("8/7p/8/q3k3/8/8/6K1/7R", WHITE, standard(), 20_000);
    assert_eq!(squares_of(result.mv), mv("h1", "h5"));
    assert!(result.score > 400, "score {}", result.score);
}

#[test]
fn a_free_queen_is_captured() {
    let (_, result) = solve_fen("6k1/5ppp/8/3q4/8/8/3R1PPP/6K1", WHITE, standard(), 5_000);
    assert_eq!(squares_of(result.mv), mv("d2", "d5"));
    assert!(result.score > 400, "score {}", result.score);
}

#[test]
fn a_defended_pawn_is_not_captured_by_the_queen() {
    // The pawn on d5 has the pawn on e6 behind it. Qxd5 loses the queen for a pawn.
    let (_, result) = solve_fen("6k1/5ppp/4p3/3p4/8/8/3Q1PPP/6K1", WHITE, standard(), 5_000);
    assert_ne!(squares_of(result.mv), mv("d2", "d5"));
}

// ---- Defence ----

#[test]
fn a_mate_in_1_is_prevented() {
    // Black wants Re1 mate. Rxa6 wins a pawn and loses the battle.
    let (state, result) = solve_fen("4r1k1/R4ppp/p7/8/8/8/5PPP/6K1", WHITE, standard(), 20_000);
    let mut next = state.clone();
    next.make(result.mv);
    assert!(!can_win_at_once(&next), "{:?} permits a mate in 1", result.mv);
    assert!(result.score > -MATE_BOUND);
}

// ---- Rout: a side with only its king loses ----

#[test]
fn the_capture_of_the_last_piece_wins_by_rout() {
    // The king defends the knight. In chess, Rxb4 loses the rook for the knight. Here it wins at once.
    let (state, result) = solve_fen("8/8/8/2k5/Rn6/8/8/6K1", WHITE, standard(), 1_000);
    assert_eq!(squares_of(result.mv), mv("a4", "b4"));
    assert_eq!(result.score, win_in(1));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Rout { winner: WHITE }));
}

#[test]
fn the_capture_of_the_last_piece_wins_by_rout_for_black() {
    let (state, result) = solve_fen("6k1/8/8/rN6/2K5/8/8/8", BLACK, standard(), 1_000);
    assert_eq!(squares_of(result.mv), mv("a5", "b5"));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Rout { winner: BLACK }));
}

#[test]
fn the_last_piece_runs_from_a_rout() {
    // The rook attacks the knight, the last piece of White. Kxe3 wins a pawn and loses the battle.
    // A defence of the knight does not help.
    let (state, result) = solve_fen("1r4k1/8/8/8/8/4p3/1N2K3/8", WHITE, standard(), 20_000);
    assert_eq!(result.mv.from, square("b2"), "the knight must move: {:?}", result.mv);
    let mut next = state.clone();
    next.make(result.mv);
    assert!(!can_win_at_once(&next));
    assert!(result.score > -MATE_BOUND);
}

#[test]
fn the_last_piece_is_not_traded() {
    // The bishop is the last piece of White. In chess, Bxc5 Kxc5 is an equal trade. Here it
    // leaves White with only its king: a loss by rout.
    let (state, result) = solve_fen("8/8/3k4/2n4p/8/4B3/8/4K3", WHITE, standard(), 20_000);
    assert_ne!(squares_of(result.mv), mv("e3", "c5"));
    let mut next = state.clone();
    next.make(result.mv);
    assert!(!can_win_at_once(&next));
    assert!(result.score > -MATE_BOUND);
}

// ---- A side with no legal move loses ----

#[test]
fn a_stalemate_wins() {
    // In chess, Kc7 is a draw by stalemate. Here Black has no legal move and loses.
    // White has no capture and no check, thus no other move wins at once.
    let (state, result) = solve_fen("k7/p2K4/P7/8/8/8/8/8", WHITE, standard(), 1_000);
    assert_eq!(result.score, win_in(1));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Stalemate { winner: WHITE }));
}

#[test]
fn a_stalemate_wins_for_black() {
    let (state, result) = solve_fen("8/8/8/8/8/p7/P2k4/K7", BLACK, standard(), 1_000);
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Stalemate { winner: BLACK }));
}

#[test]
fn the_side_with_one_free_piece_does_not_give_it_away() {
    // White can move only its rook: the pawn is blocked and the king has no free square.
    // Rxe8 wins a knight, but Kxe8 leaves White with no legal move: a loss.
    let (state, result) = solve_fen("4nk2/8/1b6/8/8/7p/7P/4R2K", WHITE, standard(), 20_000);
    assert_ne!(squares_of(result.mv), mv("e1", "e8"));
    let mut next = state.clone();
    next.make(result.mv);
    assert!(!can_win_at_once(&next));
}

// ---- Positions that only the modified rules solve ----

#[test]
fn king_knight_the_king_mates_with_a_knight_move() {
    // The king jumps from e5 to f7 and attacks h8 as a knight. No black piece attacks f7.
    let fen = "6rk/6pp/8/4K3/8/8/P7/8";
    let (state, result) = solve_fen(fen, WHITE, white_has(&["kingKnight"]), 5_000);
    assert_eq!(squares_of(result.mv), mv("e5", "f7"));
    assert_eq!(outcome_after(&state, &result), Some(Outcome::Checkmate { winner: WHITE }));
    // With the rules of chess, White has no win.
    let (_, result) = solve_fen(fen, WHITE, standard(), 5_000);
    assert!(result.score < MATE_BOUND);
}

#[test]
fn king_knight_the_king_escapes_a_mate_with_a_knight_move() {
    let fen = "4r3/8/8/8/8/8/5PPP/4r1K1";
    // With the rules of chess, this is a mate on the back rank.
    let mut mated = from_fen(fen, WHITE, standard());
    assert_eq!(choose_move(&mut mated, &Level::nodes("test", 5_000), 1), None);
    assert_eq!(outcome(&mut mated), Some(Outcome::Checkmate { winner: BLACK }));

    let (_, result) = solve_fen(fen, WHITE, white_has(&["kingKnight"]), 5_000);
    assert_eq!(result.mv.from, square("g1"));
    assert!([square("f3"), square("h3")].contains(&result.mv.to), "{:?}", result.mv);
    assert!(result.score > -MATE_BOUND);
}

#[test]
fn long_leap_the_knight_forks_the_king_and_the_queen() {
    // From d5 the knight attacks e8 and a6 with long leaps.
    let fen = "4k3/7p/q7/8/8/4N3/7P/6K1";
    let (_, result) = solve_fen(fen, WHITE, white_has(&["longLeap"]), 20_000);
    assert_eq!(squares_of(result.mv), mv("e3", "d5"));
    assert!(result.score > 400, "score {}", result.score);
    // With the rules of chess, White has a knight against a queen.
    let (_, result) = solve_fen(fen, WHITE, standard(), 20_000);
    assert!(result.score < -300, "score {}", result.score);
}

#[test]
fn early_promo_the_pawn_wins_the_promotion_race() {
    // White promotes on e7 and has the time to stop the pawn on h3.
    let fen = "8/8/4P3/8/8/7p/6k1/K7";
    let (_, result) = solve_fen(fen, WHITE, white_has(&["earlyPromo"]), 50_000);
    assert_eq!(squares_of(result.mv), mv("e6", "e7"));
    assert_eq!(result.mv.promo, Some(Kind::Queen));
    assert!(result.score >= MATE_BOUND, "score {}", result.score);
    // With the rules of chess, the two pawns promote and White has no win.
    let (_, result) = solve_fen(fen, WHITE, standard(), 50_000);
    assert!(result.score < 300, "score {}", result.score);
}

#[test]
fn sidestep_the_bishop_escapes_from_a_trap() {
    // The rook attacks the bishop on a7, and the pawn on b6 closes its diagonal.
    let fen = "r5k1/B1p3pp/1p6/8/8/8/6PP/6K1";
    let (_, with_rule) = solve_fen(fen, WHITE, white_has(&["sidestep"]), 20_000);
    assert_eq!(squares_of(with_rule.mv), mv("a7", "b7"));
    // With the rules of chess, the bishop is lost.
    let (_, without) = solve_fen(fen, WHITE, standard(), 20_000);
    assert!(with_rule.score > without.score + 200, "{} and {}", with_rule.score, without.score);
}

#[test]
fn backpedal_the_pawn_steps_back_to_block_a_check() {
    // The bishop gives check on the long diagonal. The queen can block on c6 or e4, and
    // then Black captures it with a mate. The pawn can step back from g3 to g2.
    let fen = "6k1/1b6/8/8/8/6P1/2Q1n2P/7K";
    let (_, result) = solve_fen(fen, WHITE, white_has(&["backpedal"]), 20_000);
    assert_eq!(squares_of(result.mv), mv("g3", "g2"));
    assert_eq!(result.mv.special, Special::Backward);
    assert!(result.score > 300, "score {}", result.score);
    // With the rules of chess, Black mates in 2.
    let (_, result) = solve_fen(fen, WHITE, standard(), 20_000);
    assert_eq!(result.score, -win_in(2));
}

#[test]
fn forced_march_the_pawn_runs_from_the_king() {
    // The king on e5 is in the square of the pawn on a4. With a double step on each move,
    // the pawn promotes after two moves.
    let fen = "8/7p/8/4k3/P7/8/8/6K1";
    let (_, result) = solve_fen(fen, WHITE, white_has(&["forcedMarch"]), 50_000);
    assert_eq!(squares_of(result.mv), mv("a4", "a6"));
    assert_eq!(result.mv.special, Special::DoubleStep);
    assert!(result.score > 500, "score {}", result.score);
    // With the rules of chess, the king captures the pawn and White loses by rout.
    let (_, result) = solve_fen(fen, WHITE, standard(), 50_000);
    assert!(result.score < 0, "score {}", result.score);
}

#[test]
fn the_rules_of_black_are_for_black_only() {
    // Black has the long leap: the knight on e6 forks from d4 with the mirror of the white fork.
    let rules = Rules::new(SideRules::standard(), flags(&["longLeap"]));
    let (_, result) = solve_fen("6k1/7p/4n3/8/8/Q7/7P/4K3", BLACK, rules, 20_000);
    assert_eq!(squares_of(result.mv), mv("e6", "d4"));
    assert!(result.score > 400, "score {}", result.score);
}

// ---- A king that can be captured ----

#[test]
fn the_capture_of_the_king_is_a_win() {
    // Black is in check and White has the move. The move generation permits the capture of
    // the king. The search gives it the score of a mate and does not look further.
    let mut state = from_fen("4k3/7p/8/8/8/8/7P/4RK2", WHITE, standard());
    for level in [Level::floor(1), Level::floor(8)] {
        let result = choose_move(&mut state, &level, 1).unwrap();
        assert_eq!(squares_of(result.mv), mv("e1", "e8"));
        assert_eq!(result.score, win_in(1));
    }
}

#[test]
fn a_castle_and_a_king_leap_to_the_same_square_are_one_move() {
    use chrogue_engine::{Atom, Mode};
    // The king has a leap of two squares to the side, thus e1-g1 is a castle and also a leap.
    // The engine gives only the castle.
    let leap = SideRules::standard().with_atom(Kind::King, Atom::leap(&[(2, 0), (-2, 0)], Mode::MoveOrCapture));
    let mut state = from_fen("4k3/7p/8/8/8/8/7P/4K2R", WHITE, Rules::new(leap, SideRules::standard()));
    let mut list = MoveList::new();
    legal_moves(&mut state, &mut list);
    let to_g1: Vec<Move> = list.iter().copied().filter(|m| squares_of(*m) == mv("e1", "g1")).collect();
    assert_eq!(to_g1, vec![Move { from: square("e1"), to: square("g1"), promo: None, special: Special::Castle }]);
    // The search gives one of the generated moves, with its `special` field.
    let result = solve(&mut state, 20_000);
    assert!(list.iter().any(|&m| m == result.mv));
    let mut next = state.clone();
    for &m in &result.pv {
        let mut legal = MoveList::new();
        legal_moves(&mut next, &mut legal);
        assert!(legal.iter().any(|&l| l == m), "the line has the move {m:?} that is not legal");
        next.make(m);
    }
}

// ---- Determinism and the levels ----

#[test]
fn the_same_position_level_and_seed_give_the_same_result() {
    let fen = "r2q1rk1/pp2bppp/2n1bn2/2pp4/3P4/2N1PN2/PP2BPPP/R1BQ1RK1";
    let rules = || Rules::new(flags(&["longLeap", "sidestep"]), flags(&["kingKnight", "forcedMarch"]));
    for level in Level::LADDER.iter().chain([&Level::reference()]) {
        let first = choose_move(&mut from_fen(fen, WHITE, rules()), level, 42).unwrap();
        for _ in 0..3 {
            let again = choose_move(&mut from_fen(fen, WHITE, rules()), level, 42).unwrap();
            assert_eq!(again, first, "level {}", level.name);
        }
        assert!(first.nodes <= level.limits.max_nodes);
    }
}

#[test]
fn the_seed_changes_the_move_of_a_level_with_noise_and_not_of_a_level_without() {
    let fen = "r2q1rk1/pp2bppp/2n1bn2/2pp4/3P4/2N1PN2/PP2BPPP/R1BQ1RK1";
    let moves = |level: &Level| {
        let mut moves: Vec<Move> =
            (0..12).map(|seed| choose_move(&mut from_fen(fen, WHITE, standard()), level, seed).unwrap().mv).collect();
        moves.sort_by_key(|m| (m.from, m.to));
        moves.dedup();
        moves.len()
    };
    assert!(moves(&Level::floor(1)) > 3, "the weakest level must not play one move only");
    assert_eq!(moves(&Level::floor(8)), 1);
}

#[test]
fn a_level_with_noise_still_takes_a_win_at_once() {
    for seed in 0..20 {
        let mut state = from_fen("6k1/5ppp/8/8/8/8/5PPP/R5K1", WHITE, standard());
        let result = choose_move(&mut state, &Level::floor(1), seed).unwrap();
        assert_eq!(squares_of(result.mv), mv("a1", "a8"), "seed {seed}");
    }
}

#[test]
fn the_search_gives_no_move_when_the_side_has_no_legal_move() {
    let mut state = from_fen("k7/p1K5/P7/8/8/8/8/8", BLACK, standard());
    for level in [Level::floor(1), Level::floor(8), Level::reference()] {
        assert_eq!(choose_move(&mut state, &level, 1), None);
    }
}
