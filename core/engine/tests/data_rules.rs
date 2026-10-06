//! Rules that the six rule flags do not give. Each test defines its rules through the
//! public rules data only: these movement rules need no engine code. The tests also cover the
//! errors for rules and states that are not valid, and the limits of the move list. The last
//! part has the movement atoms of nine relics of the game.

mod common;

use std::collections::BTreeSet;

use chrogue_engine::fen::{KIWIPETE, square};
use chrogue_engine::movegen::{evasion_moves, evasion_squares, may_give_check};
use chrogue_engine::rules::{ALFIL, CAMEL, DABBABA, DIAG, FORWARD, KING, KNIGHT, ORTHO};
use chrogue_engine::{
    Atom, Castle, Color, Hook, Kind, Mode, Move, MoveList, Outcome, Piece, Placement, Promotion, Promotions, Rules,
    RulesError, Shield, SideRules, Special, Square, State, StateError, Tables, in_check, is_attacked, is_legal,
    legal_moves, moves_from, outcome, perft, pseudo_moves,
};
use common::{from_fen, legal, squares, targets, targets_from};

/// The moves of `moves_from`.
fn moves_from_list(state: &State, from: &str) -> Vec<Move> {
    let mut list = MoveList::new();
    moves_from(state, square(from), &mut list);
    list.as_slice().to_vec()
}

fn white(rules: SideRules) -> Rules {
    Rules::new(rules, SideRules::standard())
}

#[test]
fn a_rook_that_also_leaps_as_a_knight_moves_and_gives_check_that_way() {
    let rules = || white(SideRules::standard().with_atom(Kind::Rook, Atom::leap(&KNIGHT, Mode::MoveOrCapture)));

    let mut state = from_fen("4k3/8/8/8/8/1p6/P7/R3K3", Color::White, rules());
    // The pawn on a2 blocks the file. The knight leaps go to c2 and capture on b3.
    assert_eq!(targets(&mut state, "a1"), squares(&["b1", "c1", "d1", "c2", "b3"]));

    // The rook on d6 attacks the king on e8 only by a knight leap.
    let check = from_fen("4k3/8/3R4/8/8/8/8/4K2p", Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(!in_check(&from_fen("4k3/8/3R4/8/8/8/8/4K2p", Color::Black, Rules::standard()), Color::Black));
    // The rules are for White only. A black rook on the same squares gives no check.
    assert!(!in_check(&from_fen("4K3/8/3r4/8/8/8/8/4k2P", Color::White, rules()), Color::White));

    // The king must go out of the check. The rook attacks d8 and d7 on its file, and f7 by a knight leap.
    let mut state = check;
    assert_eq!(targets(&mut state, "e8"), squares(&["e7", "f8"]));
    assert!(legal(&mut state).iter().all(|m| m.from == square("e8")));
}

/// A rook that can also go two squares or more in a line and then one square to the side.
fn hook_rook() -> SideRules {
    SideRules::standard().with_hook(Kind::Rook, Hook::right_angle(&ORTHO, 2, Mode::MoveOrCapture))
}

#[test]
fn a_rook_with_a_hook_turns_after_two_empty_squares_or_more() {
    let rules = || white(hook_rook());

    // The pawn on a2 blocks the file. On the rank, the legs to c1 and to d1 turn to c2 and d2.
    // The leg to b1 is too short for b2, and the rook has no knight leap to b3.
    let mut state = from_fen("4k3/8/8/8/8/1p6/P7/R3K3", Color::White, rules());
    assert_eq!(targets(&mut state, "a1"), squares(&["b1", "c1", "d1", "c2", "d2"]));
    // The last step captures. A piece on a square of the leg stops the hook behind it.
    let mut state = from_fen("4k3/8/8/8/8/8/3p1p2/R3n2K", Color::White, rules());
    let rook = targets(&mut state, "a1");
    assert!(rook.contains(&square("d2")) && !rook.contains(&square("f2")) && !rook.contains(&square("b2")));
    assert_eq!(find_special(&mut state, "a1", "d2"), Special::None);
}

fn find_special(state: &mut State, from: &str, to: &str) -> Special {
    legal(state).iter().find(|m| (m.from, m.to) == (square(from), square(to))).expect("the move").special
}

#[test]
fn a_hook_gives_check_that_a_piece_can_block_on_the_leg() {
    let rules = || white(hook_rook());

    // The rook on a7 attacks e8 by the leg to e7. It attacks d8 and f8 in the same way, thus
    // the king has no square: checkmate.
    let mut mate = from_fen("4k3/R7/8/8/8/8/8/4K3", Color::Black, rules());
    assert!(in_check(&mate, Color::Black));
    assert!(!in_check(&from_fen("4k3/R7/8/8/8/8/8/4K3", Color::Black, Rules::standard()), Color::Black));
    assert!(legal(&mut mate).is_empty());
    // A piece on the leg stops the check.
    assert!(!in_check(&from_fen("4k3/R1n5/8/8/8/8/8/4K3", Color::Black, rules()), Color::Black));
    // The bishop ends the check on a square of the leg: b7 or d7.
    let mut state = from_fen("2b1k3/R7/8/8/8/8/8/4K3", Color::Black, rules());
    assert_eq!(targets(&mut state, "c8"), squares(&["b7", "d7"]));
    assert!(legal(&mut state).iter().all(|m| m.from == square("c8")));
    // A piece that leaves the leg opens the check, thus the knight on c7 cannot move.
    let mut pinned = from_fen("4k3/R1n5/8/8/8/8/8/4K3", Color::Black, rules());
    assert!(targets(&mut pinned, "c7").is_empty());
    // The rules are for White only.
    assert!(!in_check(&from_fen("4K3/r7/8/8/8/8/8/4k3", Color::White, rules()), Color::White));
}

#[test]
fn a_hook_of_black_is_the_mirror_of_the_hook_of_white() {
    // One bend: a leg forward, then one step toward the h file.
    let bend = Hook { bends: vec![((0, 1), (1, 0))], min_leg: 1, max_leg: 2, mode: Mode::MoveOnly };
    let side = || SideRules::standard().with_kind(Kind::Rook, Vec::new()).with_hook(Kind::Rook, bend.clone());
    let mut state = from_fen("r3k3/8/8/8/8/8/8/R3K3", Color::White, Rules::new(side(), side()));
    assert_eq!(targets(&mut state, "a1"), squares(&["b2", "b3"]));
    let mut state = from_fen("r3k3/8/8/8/8/8/8/R3K3", Color::Black, Rules::new(side(), side()));
    assert_eq!(targets(&mut state, "a8"), squares(&["b7", "b6"]));
    // A hook that only moves attacks no square.
    assert!(!is_attacked(&state, square("b7"), Color::Black));
}

/// The pawns of Black next to a bishop of Black cannot be captured.
fn blessed_black() -> Rules {
    let shield = Shield { protector: Kind::Bishop, protected: Kind::Pawn, range: 1 };
    Rules::new(SideRules::standard(), SideRules::standard().with_shield(shield))
}

#[test]
fn a_pawn_next_to_its_bishop_cannot_be_captured() {
    // The rook on d1 cannot capture the pawn on d5: the bishop on d6 is next to the pawn.
    let mut state = from_fen("4k3/8/3b4/3p4/8/8/8/3RK3", Color::White, blessed_black());
    assert_eq!(targets(&mut state, "d1"), squares(&["a1", "b1", "c1", "d2", "d3", "d4"]));
    let mut plain = from_fen("4k3/8/3b4/3p4/8/8/8/3RK3", Color::White, Rules::standard());
    assert!(targets(&mut plain, "d1").contains(&square("d5")));
    // The pawn on a5 is not next to a bishop, and the bishop has no shield.
    let mut state = from_fen("4k3/8/3b4/p7/8/8/8/R2RK3", Color::White, blessed_black());
    assert!(targets(&mut state, "a1").contains(&square("a5")));
    let mut state = from_fen("4k3/8/3b4/8/8/8/8/3RK3", Color::White, blessed_black());
    assert!(targets(&mut state, "d1").contains(&square("d6")));
    // The shield is for Black only.
    let mut state = from_fen("3rk3/8/8/8/3P4/3B4/8/4K3", Color::Black, blessed_black());
    assert!(targets(&mut state, "d8").contains(&square("d4")));

    // A pawn with a shield gives check, and the king cannot capture it.
    let mut state = from_fen("4k3/8/3b4/3p4/4K3/8/8/8", Color::White, blessed_black());
    assert!(in_check(&state, Color::White));
    assert!(!targets(&mut state, "e4").contains(&square("d5")));
    let mut plain = from_fen("4k3/8/3b4/3p4/4K3/8/8/8", Color::White, Rules::standard());
    assert!(targets(&mut plain, "e4").contains(&square("d5")));

    // An en passant capture cannot take a pawn with a shield.
    let mut state = from_fen("4k3/3p4/2b5/4P3/8/8/8/4K3", Color::Black, blessed_black());
    let double = legal(&mut state).into_iter().find(|m| m.special == Special::DoubleStep).expect("the double step");
    state.make(double);
    assert!(legal(&mut state).iter().all(|m| m.special != Special::EnPassant));
    let mut plain = from_fen("4k3/3p4/2b5/4P3/8/8/8/4K3", Color::Black, Rules::standard());
    plain.make(double);
    assert!(legal(&mut plain).iter().any(|m| m.special == Special::EnPassant));
}

#[test]
fn a_queen_with_move_only_slides_cannot_capture_or_give_check() {
    let quiet_queen = SideRules::standard()
        .with_kind(Kind::Queen, vec![Atom::slide(&ORTHO, Mode::MoveOnly), Atom::slide(&DIAG, Mode::MoveOnly)]);
    let rules = || white(quiet_queen.clone());

    // The queen on d1 has the black rook on d5 and the black king on d8 on its file.
    let mut state = from_fen("3k4/8/8/3r4/8/8/8/3QK3", Color::White, rules());
    let queen = targets(&mut state, "d1");
    assert!(queen.contains(&square("d4")));
    assert!(!queen.contains(&square("d5")), "the queen must not capture");
    assert!(!queen.contains(&square("d6")), "the rook blocks the line");
    assert!(queen.contains(&square("a4")));

    // The queen attacks no square, thus the king next to it is not in check and can capture it.
    let mut state = from_fen("8/8/8/8/8/8/3k4/3Q3K", Color::Black, rules());
    assert!(!in_check(&state, Color::Black));
    assert!(!is_attacked(&state, square("c2"), Color::White));
    assert!(targets(&mut state, "d2").contains(&square("d1")));
    assert!(targets(&mut state, "d2").contains(&square("c2")), "c2 is on a queen line but is not attacked");
    // With the ordinary queen, the same king is in check.
    assert!(in_check(&from_fen("8/8/8/8/8/8/3k4/3Q3K", Color::Black, Rules::standard()), Color::Black));

    // A mate with an ordinary queen is not a mate with this queen.
    let mate = "R5k1/5ppp/8/8/8/8/8/4K3";
    let queen_mate = "Q5k1/5ppp/8/8/8/8/8/4K3";
    let checkmate = Some(Outcome::Checkmate { winner: Color::White });
    assert_eq!(outcome(&mut from_fen(mate, Color::Black, rules())), checkmate);
    assert_eq!(outcome(&mut from_fen(queen_mate, Color::Black, Rules::standard())), checkmate);
    assert_eq!(outcome(&mut from_fen(queen_mate, Color::Black, rules())), None);
}

#[test]
fn a_capture_only_leap_captures_and_gives_check_but_does_not_move_to_an_empty_square() {
    let rules = || white(SideRules::standard().with_atom(Kind::Knight, Atom::leap(&CAMEL, Mode::CaptureOnly)));

    // The camel squares of d4 are a3, a5, c1, c7, e1, e7, g3, g5. Only c7 has an enemy piece.
    let mut state = from_fen("4k3/2p5/8/8/3N4/8/8/4K3", Color::White, rules());
    let mut expected = squares(&["b3", "b5", "c2", "c6", "e2", "e6", "f3", "f5"]);
    expected.push(square("c7"));
    expected.sort_unstable();
    assert_eq!(targets(&mut state, "d4"), expected);

    // The knight on d5 attacks the king on e8 by the camel leap.
    let check = from_fen("4k3/8/8/3N4/8/8/8/4K2p", Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(is_attacked(&check, square("a6"), Color::White), "an empty camel square is attacked");
    assert!(!in_check(&from_fen("4k3/8/8/3N4/8/8/8/4K2p", Color::Black, Rules::standard()), Color::Black));
}

#[test]
fn an_asymmetric_leap_points_forward_for_each_side() {
    // The bishop also leaps two squares straight forward. Forward is toward rank 8 for White
    // and toward rank 1 for Black.
    let lunge = || SideRules::standard().with_atom(Kind::Bishop, Atom::leap(&[(0, 2)], Mode::MoveOrCapture));
    let rules = || Rules::new(lunge(), lunge());

    let mut state = from_fen("7k/8/8/4b3/4B3/8/8/7K", Color::White, rules());
    assert!(targets(&mut state, "e4").contains(&square("e6")));
    assert!(!targets(&mut state, "e4").contains(&square("e2")));
    let mut state = state.with_turn(Color::Black);
    assert!(targets(&mut state, "e5").contains(&square("e3")));
    assert!(!targets(&mut state, "e5").contains(&square("e7")));

    // The reverse tables: the white bishop on e4 attacks e6 and not e2.
    assert!(is_attacked(&state, square("e6"), Color::White));
    assert!(!is_attacked(&state, square("e2"), Color::White));
    // The black bishop on e5 attacks e3 and not e7.
    assert!(is_attacked(&state, square("e3"), Color::Black));
    assert!(!is_attacked(&state, square("e7"), Color::Black));

    // The white bishop on e6 gives check to the king on e8. The black bishop on e3 gives
    // check to the king on e1. A bishop behind its target gives no check.
    assert!(in_check(&from_fen("4k3/8/4B3/8/8/8/8/4K2p", Color::Black, rules()), Color::Black));
    assert!(in_check(&from_fen("4k2P/8/8/8/8/4b3/8/4K3", Color::White, rules()), Color::White));
    assert!(!in_check(&from_fen("8/8/4B3/8/4k3/8/8/K6p", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("k6P/8/8/4K3/8/4b3/8/8", Color::White, rules()), Color::White));
    // The king cannot go to a square that the leap attacks: f8 is two squares in front of the bishop on f6.
    let mut state = from_fen("4k3/8/5B2/8/8/8/8/K6p", Color::Black, rules());
    assert!(!targets(&mut state, "e8").contains(&square("f8")));
    assert!(!targets(&mut state, "e8").contains(&square("d8")), "d8 is on a diagonal of the bishop");
    assert!(targets(&mut state, "e8").contains(&square("f7")));
}

#[test]
fn an_asymmetric_slide_attacks_only_along_its_direction() {
    // The knight becomes a lance: it slides straight forward only.
    let lance = || SideRules::standard().with_kind(Kind::Knight, vec![Atom::slide(&[(0, 1)], Mode::MoveOrCapture)]);
    let rules = || Rules::new(lance(), lance());

    let mut state = from_fen("4k3/8/4p3/8/8/4N3/8/K7", Color::White, rules());
    assert_eq!(targets(&mut state, "e3"), squares(&["e4", "e5", "e6"]));
    assert!(is_attacked(&state, square("e6"), Color::White));
    assert!(!is_attacked(&state, square("e7"), Color::White), "the pawn on e6 blocks the line");
    assert!(!is_attacked(&state, square("e2"), Color::White));

    // The white lance gives check up the file. The black lance gives check down the file.
    assert!(in_check(&from_fen("4k3/8/8/8/8/4N3/8/3K4", Color::Black, rules()), Color::Black));
    assert!(in_check(&from_fen("3k4/8/4n3/8/8/8/8/4K3", Color::White, rules()), Color::White));
    assert!(!in_check(&from_fen("4K3/8/8/8/8/4n3/8/3k4", Color::White, rules()), Color::White));
    assert!(!in_check(&from_fen("3K4/8/4N3/8/8/8/8/4k3", Color::Black, rules()), Color::Black));
}

#[test]
fn a_slide_in_a_knight_direction_and_a_line_with_two_modes() {
    // A nightrider slides along knight steps. No direction is special in the engine.
    let rider = SideRules::standard().with_kind(Kind::Knight, vec![Atom::slide(&KNIGHT, Mode::MoveOrCapture)]);
    let mut state = from_fen("7k/8/3p4/8/8/8/8/N3K3", Color::White, white(rider.clone()));
    // a1, b3, c5, d7 is one line. a1, c2, e3, g4 is the second line.
    assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c5", "d7", "c2", "e3", "g4"]));
    assert!(is_attacked(&state, square("d7"), Color::White));
    // The pawn on d6 is not on a line. A white pawn on c5 blocks the first line after b3.
    let mut blocked = from_fen("7k/8/8/2P5/8/8/8/N3K3", Color::White, white(rider.clone()));
    assert_eq!(targets(&mut blocked, "a1"), squares(&["b3", "c2", "e3", "g4"]));
    assert!(!is_attacked(&blocked, square("d7"), Color::White));
    // A black pawn on c5 stops the slide with a capture.
    let mut capture = from_fen("7k/8/8/2p5/8/8/8/N3K3", Color::White, white(rider));
    assert_eq!(targets(&mut capture, "a1"), squares(&["b3", "c5", "c2", "e3", "g4"]));

    // A rook that moves on files and ranks without a capture, and captures on files only.
    let rook = SideRules::standard().with_kind(
        Kind::Rook,
        vec![Atom::slide(&ORTHO, Mode::MoveOnly), Atom::slide(&[(0, 1), (0, -1)], Mode::CaptureOnly)],
    );
    let mut state = from_fen("3pk3/8/8/8/3R2p1/8/8/4K3", Color::White, white(rook));
    let rook = targets(&mut state, "d4");
    assert!(rook.contains(&square("d8")), "the rook captures on the file");
    assert!(rook.contains(&square("f4")));
    assert!(!rook.contains(&square("g4")), "the rook does not capture on the rank");
    assert!(is_attacked(&state, square("d7"), Color::White));
    assert!(!is_attacked(&state, square("f4"), Color::White));
}

#[test]
fn the_rules_of_a_side_can_remove_castling_and_change_the_promotions() {
    let kiwipete = chrogue_engine::fen::KIWIPETE;
    let no_castle = SideRules::standard().with_castling(false);
    // Kiwipete has 48 moves for White. Two of them are castles.
    assert_eq!(perft(&mut from_fen(kiwipete, Color::White, Rules::standard()), 1), 48);
    assert_eq!(perft(&mut from_fen(kiwipete, Color::White, white(no_castle)), 1), 46);

    // One promotion kind gives one move for each promotion square.
    let knight = Promotions::new(&[Kind::Knight]).unwrap();
    let knights = SideRules::standard().with_promotion(Kind::Pawn, Some(Promotion { distance: 0, kinds: knight }));
    let mut state = from_fen("7k/4P2p/8/8/8/8/8/4K3", Color::White, white(knights));
    let promotions: Vec<Move> = legal(&mut state).into_iter().filter(|m| m.from == square("e7")).collect();
    assert_eq!(
        promotions,
        vec![Move { from: square("e7"), to: square("e8"), promo: Some(Kind::Knight), special: Special::None }]
    );
    assert_eq!(perft(&mut state, 1), 6, "five king moves and one promotion");

    // Two kinds in a new order give two moves in that order.
    let two = Promotions::new(&[Kind::Rook, Kind::Bishop]).unwrap();
    let mut state = from_fen(
        "7k/4P2p/8/8/8/8/8/4K3",
        Color::White,
        white(SideRules::standard().with_promotion(Kind::Pawn, Some(Promotion { distance: 0, kinds: two }))),
    );
    let kinds: Vec<Option<Kind>> =
        legal(&mut state).iter().filter(|m| m.from == square("e7")).map(|m| m.promo).collect();
    assert_eq!(kinds, vec![Some(Kind::Rook), Some(Kind::Bishop)]);
}

#[test]
fn a_promotion_list_must_have_one_to_four_different_officers_that_are_not_the_king() {
    assert_eq!(Promotions::new(&[Kind::Knight; 4]), Err(RulesError::BadPromotion(Kind::Knight)));
    assert_eq!(Promotions::new(&[Kind::Queen, Kind::King]), Err(RulesError::BadPromotion(Kind::King)));
    assert_eq!(Promotions::new(&[Kind::Pawn]), Err(RulesError::BadPromotion(Kind::Pawn)));
    assert_eq!(Promotions::new(&[]), Err(RulesError::NoPromotion));
    let all = [Kind::Queen, Kind::Knight, Kind::Rook, Kind::Bishop];
    assert_eq!(Promotions::new(&all).unwrap().as_slice(), &all);
    assert_eq!(Promotions::STANDARD.as_slice(), &all);
}

#[test]
fn a_king_move_to_a_castle_square_gives_one_move() {
    let slide = SideRules::standard().with_atom(Kind::King, Atom::slide(&ORTHO, Mode::MoveOrCapture));
    let leap = SideRules::standard().with_atom(Kind::King, Atom::leap(&[(2, 0), (-2, 0)], Mode::MoveOnly));
    for side in [slide, leap] {
        let to_wing = |moves: &[Move]| -> Vec<(Square, Special)> {
            let mut list: Vec<(Square, Special)> = moves
                .iter()
                .filter(|m| m.from == square("e1") && [square("c1"), square("g1")].contains(&m.to))
                .map(|m| (m.to, m.special))
                .collect();
            list.sort_by_key(|&(to, _)| to);
            list
        };
        // The castles are legal: the castle is the only move to c1 and to g1.
        let mut state = from_fen("4k3/7p/8/8/8/8/8/R3K2R", Color::White, white(side.clone()));
        let castles = vec![(square("c1"), Special::Castle), (square("g1"), Special::Castle)];
        assert_eq!(to_wing(&legal(&mut state)), castles);
        assert_eq!(to_wing(&moves_from_list(&state, "e1")), castles);
        // The rooks moved: no castle, and the king move stays.
        let mut state = from_fen("4k3/7p/8/8/8/8/R7/4K2R", Color::White, white(side.clone()));
        let rook = state.piece_at(square("h1")).unwrap();
        let moved = Placement { piece: Piece { moved: true, ..rook }, square: square("h1") };
        let mut pieces: Vec<Placement> =
            (0..64).filter_map(|s| state.piece_at(s).map(|piece| Placement { piece, square: s })).collect();
        pieces.push(moved);
        let mut no_castle = State::new(&pieces, white(side.clone())).unwrap();
        assert_eq!(to_wing(&legal(&mut no_castle)), vec![(square("c1"), Special::None), (square("g1"), Special::None)]);
        assert_eq!(to_wing(&legal(&mut state)), vec![(square("c1"), Special::None), (square("g1"), Special::Castle)]);
        // Castling is off for the side: the king moves stay.
        let mut state = from_fen("4k3/7p/8/8/8/8/8/R3K2R", Color::White, white(side.with_castling(false)));
        assert_eq!(to_wing(&legal(&mut state)), vec![(square("c1"), Special::None), (square("g1"), Special::None)]);
    }
}

#[test]
fn a_castle_that_is_not_legal_keeps_the_king_move_to_its_square() {
    // A castle e1-d1 with the rook d2-f1. The rook on d2 closes the d file against the black
    // rook on d8. The castle opens the file, thus it is not legal, but the king step e1-d1 is.
    let castle = Castle {
        king_from: square("e1"),
        king_to: square("d1"),
        partner: Kind::Rook,
        partner_from: square("d2"),
        partner_to: square("f1"),
        empty: 0,
        safe: 0,
    };
    let side = SideRules::standard().with_castles(vec![castle]);
    let piece =
        |kind, color, moved, s: &str| Placement { piece: Piece { id: 0, kind, color, moved }, square: square(s) };
    let pieces = |black_rook: &str| {
        vec![
            piece(Kind::King, Color::White, false, "e1"),
            piece(Kind::Rook, Color::White, false, "d2"),
            piece(Kind::Pawn, Color::White, true, "a4"),
            piece(Kind::King, Color::Black, false, "h8"),
            piece(Kind::Rook, Color::Black, true, black_rook),
        ]
    };
    let to_d1 = |moves: &[Move]| -> Vec<Special> {
        moves.iter().filter(|m| (m.from, m.to) == (square("e1"), square("d1"))).map(|m| m.special).collect()
    };
    let mut state = State::new(&pieces("d8"), white(side.clone())).unwrap();
    assert_eq!(to_d1(&legal(&mut state)), vec![Special::None]);
    assert_eq!(to_d1(&moves_from_list(&state, "e1")), vec![Special::None]);
    // With the black rook on a8, the castle is legal and is the only move to d1.
    let mut state = State::new(&pieces("a8"), white(side)).unwrap();
    assert_eq!(to_d1(&legal(&mut state)), vec![Special::Castle]);
}

#[test]
fn a_king_cannot_make_en_passant_squares() {
    // Else an en passant capture could remove a king that no piece attacks.
    let double = Atom::slide(&[(0, 1)], Mode::MoveOnly).max_steps(2).makes_en_passant();
    let king = SideRules::standard().with_atom(Kind::King, double.clone());
    assert_eq!(king.validate(), Err(RulesError::KingMakesEnPassant));
    assert!(matches!(State::new(&[], Rules::new(SideRules::standard(), king)), Err(StateError::Rules(_))));
    // The same atom is valid for an officer.
    assert!(SideRules::standard().with_atom(Kind::Queen, double).validate().is_ok());
}

#[test]
fn a_move_list_has_no_limit_and_legal_moves_appends_to_it() {
    // A king and 15 queens that also slide as nightriders and leap as camels.
    let amazon = SideRules::standard()
        .with_atom(Kind::Queen, Atom::slide(&KNIGHT, Mode::MoveOrCapture))
        .with_atom(Kind::Queen, Atom::leap(&CAMEL, Mode::MoveOrCapture));
    // The king is on the first square. A search found these squares: they give 391 moves.
    let squares = [15, 3, 12, 16, 25, 27, 28, 29, 30, 31, 32, 44, 51, 58, 60, 63];
    let pieces: Vec<Placement> = squares
        .iter()
        .enumerate()
        .map(|(i, &s)| Placement {
            piece: Piece {
                id: i as u16,
                kind: if i == 0 { Kind::King } else { Kind::Queen },
                color: Color::White,
                moved: true,
            },
            square: s,
        })
        .collect();
    let mut state = State::new(&pieces, white(amazon)).unwrap();
    let mut list = MoveList::new();
    pseudo_moves(&state, Color::White, false, &mut list);
    assert!(list.len() > MoveList::CAPACITY, "the position must need the heap, it has {} moves", list.len());
    let pseudo = list.as_slice().to_vec();
    // White has no enemy piece, thus each pseudo move is legal.
    let all = legal(&mut state);
    assert_eq!(all, pseudo);
    assert_eq!(perft(&mut state, 1), pseudo.len() as u64);
    // The search gives a move. (`choose_move` gives none: Black has no piece, thus the battle
    // has ended by rout.)
    let (limits, variant) = (chrogue_engine::Limits::nodes(2000), chrogue_engine::EvalVariant::Derived);
    let (options, flaws) = (chrogue_engine::Level::OPTIONS, chrogue_engine::Flaws::NONE);
    assert!(chrogue_engine::search(&mut state, &limits, variant, options, flaws, 1).is_some());

    // A list goes back from the heap when it gets small.
    list.retain(|m| m.from == 0);
    assert!(list.len() < 10 && list.iter().all(|m| m.from == 0));
    list.clear();
    assert!(list.is_empty());

    // `legal_moves` keeps the moves that the list has and filters only its own moves.
    let mut state = from_fen("4k3/8/8/8/8/8/4r3/4K2P", Color::White, Rules::standard());
    let alone = legal(&mut state);
    assert!(!alone.is_empty() && alone.iter().all(|m| m.from == square("e1")));
    let mut list = MoveList::new();
    let marker = Move::new(square("a8"), square("a7"));
    list.push(marker);
    legal_moves(&mut state, &mut list);
    assert_eq!(list.as_slice()[0], marker);
    assert_eq!(&list.as_slice()[1..], &alone[..]);
    // Also across the end of the inline part of the list.
    let mut list = MoveList::new();
    for _ in 0..MoveList::CAPACITY - 1 {
        list.push(marker);
    }
    legal_moves(&mut state, &mut list);
    assert_eq!(list.len(), MoveList::CAPACITY - 1 + alone.len());
    assert!(list.as_slice()[..MoveList::CAPACITY - 1].iter().all(|&m| m == marker));
    assert_eq!(&list.as_slice()[MoveList::CAPACITY - 1..], &alone[..]);
}

#[test]
fn rules_and_states_that_are_not_valid_give_an_error() {
    let zero = SideRules::standard().with_atom(Kind::Rook, Atom::leap(&[(0, 0)], Mode::MoveOrCapture));
    assert!(zero.validate().is_err());
    assert!(Tables::new(Rules::new(zero.clone(), SideRules::standard())).is_err());
    assert!(matches!(State::new(&[], Rules::new(SideRules::standard(), zero)), Err(StateError::Rules(_))));
    let far = SideRules::standard().with_atom(Kind::Rook, Atom::slide(&[(8, 0)], Mode::MoveOrCapture));
    assert_eq!(far.validate(), Err(RulesError::BadOffset(Kind::Rook, (8, 0))));
    for steps in [0, 8] {
        let bad = SideRules::standard().with_atom(Kind::Pawn, Atom::slide(&[(0, 1)], Mode::MoveOnly).max_steps(steps));
        assert_eq!(bad.validate(), Err(RulesError::BadSteps(Kind::Pawn, steps)));
    }
    let distance = SideRules::standard()
        .with_promotion(Kind::Pawn, Some(Promotion { distance: 7, kinds: chrogue_engine::Promotions::STANDARD }));
    assert_eq!(distance.validate(), Err(RulesError::BadPromoDistance(7)));
    assert!(State::new(&[], Rules::new(distance, SideRules::standard())).is_err());
    assert!(SideRules::from_flags(["noSuchFlag"]).is_err());
    assert!(SideRules::from_flags(chrogue_engine::rules::FLAG_NAMES).unwrap().validate().is_ok());

    // A hook needs a leg of 1 to 7 squares, offsets on the board, and a kind with plain atoms
    // and no promotion.
    let hook = |min_leg, max_leg| Hook { bends: vec![((1, 0), (0, 1))], min_leg, max_leg, mode: Mode::MoveOrCapture };
    assert!(SideRules::standard().with_hook(Kind::Rook, hook(1, 7)).validate().is_ok());
    for bad in [hook(0, 7), hook(3, 2), hook(1, 8), Hook { bends: vec![((1, 0), (0, 0))], ..hook(1, 7) }] {
        assert_eq!(SideRules::standard().with_hook(Kind::Rook, bad).validate(), Err(RulesError::BadHook(Kind::Rook)));
    }
    assert_eq!(
        SideRules::standard().with_hook(Kind::Pawn, hook(1, 7)).validate(),
        Err(RulesError::BadHook(Kind::Pawn))
    );
    let first_move = SideRules::standard()
        .with_atom(Kind::Rook, Atom::leap(&KNIGHT, Mode::MoveOnly).if_unmoved())
        .with_hook(Kind::Rook, hook(1, 7));
    assert_eq!(first_move.validate(), Err(RulesError::BadHook(Kind::Rook)));

    // A shield cannot protect the king, and its range is from 1 to 7.
    let shield =
        |protected, range| SideRules::standard().with_shield(Shield { protector: Kind::Bishop, protected, range });
    assert!(shield(Kind::Pawn, 7).validate().is_ok());
    for bad in [shield(Kind::King, 1), shield(Kind::Pawn, 0), shield(Kind::Pawn, 8)] {
        assert_eq!(bad.validate(), Err(RulesError::BadShield(0)));
    }

    // An officer can have an atom that makes en passant squares.
    let rook = SideRules::standard().with_atom(Kind::Rook, Atom::slide(&ORTHO, Mode::MoveOnly).makes_en_passant());
    assert!(rook.validate().is_ok());

    // Castles with a square off the board, the king and the partner on one square, or the same
    // king squares two times.
    let castle = Castle::KING_SIDE;
    for (index, castles) in [
        (0, vec![Castle { partner_from: 64, ..castle }]),
        (0, vec![Castle { partner_from: castle.king_from, ..castle }]),
        (0, vec![Castle { king_to: castle.king_from, ..castle }]),
        (0, vec![Castle { partner_to: castle.king_to, ..castle }]),
        (1, vec![castle, Castle { partner: Kind::Queen, ..castle }]),
    ] {
        assert_eq!(SideRules::standard().with_castles(castles).validate(), Err(RulesError::BadCastle(index)));
    }

    // A square off the board, and en passant squares that no double step can make.
    let king = Piece { id: 0, kind: Kind::King, color: Color::White, moved: false };
    assert_eq!(State::new(&[Placement { piece: king, square: 64 }], Rules::standard()), Err(StateError::BadSquare(64)));
    let state = from_fen("4k3/8/8/3pP3/8/8/8/4K3", Color::White, Rules::standard());
    for ep in [64, 200, 255, square("e6"), square("c6"), square("d5"), square("d4")] {
        assert_eq!(state.clone().with_en_passant(Some(ep)), Err(StateError::BadEnPassant(ep)), "square {ep}");
    }
    assert_eq!(state.clone().with_en_passant(Some(square("d6"))).unwrap().ep(), Some(square("d6")));
    // A new side to move clears the en passant square.
    let black = state.with_en_passant(Some(square("d6"))).unwrap().with_turn(Color::Black);
    assert_eq!((black.turn(), black.ep()), (Color::Black, None));
    assert!(matches!(
        chrogue_engine::fen::from_fen("8/8/8", Color::White, Rules::standard()),
        Err(StateError::BadFen(_))
    ));
    assert!(chrogue_engine::fen::placements("4k3/8/8/8/8/8/8/4K4").is_err());
    assert!(chrogue_engine::fen::placements("4x3/8/8/8/8/8/8/4K3").is_err());
}

// ---- The movement atoms of the relics ----
//
// The game layer adds each of these atoms to `SideRules::standard()` with `with_atom`.

/// Rampart Vault: the rook jumps two squares on a file or a rank to an empty square.
fn vault() -> SideRules {
    SideRules::standard().with_atom(Kind::Rook, Atom::leap(&DABBABA, Mode::MoveOnly))
}

/// Crossfire: the rook captures on the next square of each diagonal.
fn crossfire() -> SideRules {
    SideRules::standard().with_atom(Kind::Rook, Atom::leap(&DIAG, Mode::CaptureOnly))
}

/// Close Quarters: the knight captures on the next square of its file and its rank.
fn close_quarters() -> SideRules {
    SideRules::standard().with_atom(Kind::Knight, Atom::leap(&ORTHO, Mode::CaptureOnly))
}

/// Pilgrim's Leap: the bishop jumps two squares on a diagonal.
fn pilgrims_leap() -> SideRules {
    SideRules::standard().with_atom(Kind::Bishop, Atom::leap(&ALFIL, Mode::MoveOrCapture))
}

/// Queen's Flight: the queen jumps as a knight to an empty square.
fn queens_flight() -> SideRules {
    SideRules::standard().with_atom(Kind::Queen, Atom::leap(&KNIGHT, Mode::MoveOnly))
}

/// Huntress: the queen captures as a knight.
fn huntress() -> SideRules {
    SideRules::standard().with_atom(Kind::Queen, Atom::leap(&KNIGHT, Mode::CaptureOnly))
}

/// The atom of Gallop: one jump of a knight, or two jumps in the same direction.
fn gallop_atom() -> Atom {
    Atom::slide(&KNIGHT, Mode::MoveOrCapture).max_steps(2)
}

/// Gallop: the knight also has `gallop_atom`.
fn gallop() -> SideRules {
    SideRules::standard().with_atom(Kind::Knight, gallop_atom())
}

/// Crusade: the bishop slides straight forward.
fn crusade() -> SideRules {
    SideRules::standard().with_atom(Kind::Bishop, Atom::slide(&FORWARD, Mode::MoveOrCapture))
}

/// Royal March: the king goes one or two squares in each of its directions.
fn royal_march() -> SideRules {
    SideRules::standard().with_atom(Kind::King, Atom::slide(&KING, Mode::MoveOrCapture).max_steps(2))
}

/// The number of legal move sequences of `depth` half moves. `checks` counts the nodes where
/// the side that has the move is in check. At each node:
///
/// - No two legal moves have the same squares and the same promotion.
/// - The moves of `moves_from` for the pieces of the side are the legal moves.
/// - For a side in check, the legal moves of `evasion_moves` are the legal moves, in the same order.
/// - `may_give_check` is true for each move that gives check. The search relies on it.
fn walk(state: &mut State, depth: u32, checks: &mut u32) -> u64 {
    let color = state.turn();
    let moves = legal(state);
    let distinct: BTreeSet<_> = moves.iter().map(|m| (m.from, m.to, m.promo.map(|kind| kind as u8))).collect();
    assert_eq!(distinct.len(), moves.len(), "two legal moves have the same squares: {moves:?}");
    let mut by_piece = MoveList::new();
    for from in 0..64 {
        if state.piece_at(from).is_some_and(|piece| piece.color == color) {
            moves_from(state, from, &mut by_piece);
        }
    }
    let sorted = |list: &[Move]| {
        let mut keys: Vec<_> =
            list.iter().map(|m| (m.from, m.to, m.promo.map(|kind| kind as u8), m.special as u8)).collect();
        keys.sort_unstable();
        keys
    };
    assert_eq!(sorted(by_piece.as_slice()), sorted(&moves), "`moves_from` and `legal_moves` differ");
    if in_check(state, color) {
        *checks += 1;
        let mut list = MoveList::new();
        evasion_moves(state, color, evasion_squares(state, color), &mut list);
        let evasions: Vec<Move> = list.iter().copied().filter(|&m| is_legal(state, m, color)).collect();
        assert_eq!(evasions, moves, "the evasions differ from the legal moves");
    }
    let mut nodes = 0;
    for m in moves {
        let undo = state.make(m);
        assert!(
            !in_check(state, color.other()) || may_give_check(state, m, &undo),
            "the search skips the check of {m:?}"
        );
        nodes += if depth == 1 { 1 } else { walk(state, depth - 1, checks) };
        state.unmake(m, undo);
    }
    nodes
}

/// Makes sure that the rules are valid, and walks the move tree of Kiwipete to depth 3 with
/// the rules for the two sides. Thus Black plays the mirror of the atom. Gives the number of
/// move sequences.
fn walk_kiwipete(side: SideRules) -> u64 {
    assert_eq!(side.validate(), Ok(()));
    let rules = Rules::new(side.clone(), side);
    assert_eq!(rules.validate(), Ok(()));
    assert!(Tables::new(rules.clone()).is_ok());
    let mut state = from_fen(KIWIPETE, Color::White, rules);
    let before = state.clone();
    let mut checks = 0;
    let nodes = walk(&mut state, 3, &mut checks);
    assert_eq!(perft(&mut state, 3), nodes);
    assert!(state == before, "the walk changed the state");
    assert!(checks > 0, "the tree has no check");
    // Ordinary chess has 97 862 sequences. A different number shows that the tree has moves of the rule.
    assert_ne!(nodes, 97_862);
    nodes
}

#[test]
fn rampart_vault_lets_a_rook_jump_two_squares_to_an_empty_square() {
    let rules = || white(vault());

    // The pawn on a2 and the bishop on b1 close the file and the rank of the rook on a1. The
    // rook jumps over them to a3 and c1.
    let fen = "4k3/8/8/8/8/8/P7/RB2K3";
    assert_eq!(targets(&mut from_fen(fen, Color::White, rules()), "a1"), squares(&["a3", "c1"]));
    assert!(targets(&mut from_fen(fen, Color::White, Rules::standard()), "a1").is_empty());
    // A black pawn on a3 and a white knight on c1: the jump does not capture, and it does not
    // go to a square with a friend.
    assert!(targets(&mut from_fen("4k3/8/8/8/8/p7/P7/RBN1K3", Color::White, rules()), "a1").is_empty());
    // The rook on d4 captures the pawn on d5 by its slide, and it jumps over the pawn to d6.
    // The other jump squares (b4, f4, d2) are also squares of the slides. Each target occurs one time.
    let mut state = from_fen("4k3/8/8/3p4/3R4/8/8/4K3", Color::White, rules());
    let expected = squares(&["a4", "b4", "c4", "e4", "f4", "g4", "h4", "d1", "d2", "d3", "d5", "d6"]);
    assert_eq!(targets(&mut state, "d4"), expected);
    assert_eq!(targets_from(&state, "d4"), expected);

    // The jump attacks no square. The rook on e6 gives no check over the pawn on e7, and the
    // king on d8 can go to e8.
    assert!(!in_check(&from_fen("4k3/4p3/4R3/8/8/8/8/K7", Color::Black, rules()), Color::Black));
    let mut state = from_fen("3k4/4p3/4R3/8/8/8/8/K7", Color::Black, rules());
    assert!(!is_attacked(&state, square("e8"), Color::White));
    assert_eq!(targets(&mut state, "d8"), squares(&["c7", "c8", "d7", "e8"]));

    walk_kiwipete(vault());
}

#[test]
fn crossfire_lets_a_rook_capture_and_give_check_on_the_next_diagonal_squares() {
    let rules = || white(crossfire());

    // The diagonal squares of d4 are c3, c5, e3, and e5. Only e5 has an enemy piece.
    let mut state = from_fen("4k3/8/8/4p3/3R4/8/8/4K3", Color::White, rules());
    let slides = ["a4", "b4", "c4", "e4", "f4", "g4", "h4", "d1", "d2", "d3", "d5", "d6", "d7", "d8"];
    assert_eq!(targets(&mut state, "d4"), squares(&[&slides[..], &["e5"]].concat()));

    // The rook on d7 attacks the king on e8 only by the diagonal capture.
    let fen = "4k3/3R4/8/8/8/8/8/4K3";
    let mut check = from_fen(fen, Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    // The king captures the rook or goes to f8. The slides of the rook attack d8, e7, and f7.
    assert_eq!(targets(&mut check, "e8"), squares(&["d7", "f8"]));

    // The rook on d6 attacks the empty square e7 by the diagonal capture, thus the king cannot go there.
    let fen = "4k3/8/3R4/8/8/8/8/4K3";
    let mut state = from_fen(fen, Color::Black, rules());
    assert!(is_attacked(&state, square("e7"), Color::White));
    assert_eq!(targets(&mut state, "e8"), squares(&["f7", "f8"]));
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "e8"), squares(&["e7", "f7", "f8"]));

    walk_kiwipete(crossfire());
}

#[test]
fn close_quarters_lets_a_knight_capture_and_give_check_on_the_next_squares_of_its_file_and_rank() {
    let rules = || white(close_quarters());

    // The squares next to d4 on its file and its rank are c4, d3, d5, and e4. Only d5 has an enemy piece.
    let mut state = from_fen("4k3/8/8/3p4/3N4/8/8/4K3", Color::White, rules());
    assert_eq!(targets(&mut state, "d4"), squares(&["b3", "b5", "c2", "c6", "e2", "e6", "f3", "f5", "d5"]));

    // The knight on e7 attacks the king on e8 only by the new capture.
    let fen = "4k3/4N3/8/8/8/8/8/4K3";
    let mut check = from_fen(fen, Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    // The king captures the knight or goes to d8 or f8. The knight attacks d7 and f7.
    assert_eq!(targets(&mut check, "e8"), squares(&["d8", "e7", "f8"]));

    // The knight on e6 attacks d8 and f8 by its jumps, and the empty square e7 by the new capture.
    let fen = "4k3/8/4N3/8/8/8/8/4K3";
    let mut state = from_fen(fen, Color::Black, rules());
    assert!(is_attacked(&state, square("e7"), Color::White));
    assert_eq!(targets(&mut state, "e8"), squares(&["d7", "f7"]));
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "e8"), squares(&["d7", "e7", "f7"]));

    walk_kiwipete(close_quarters());
}

#[test]
fn pilgrims_leap_lets_a_bishop_jump_two_squares_on_a_diagonal_and_give_a_check_that_no_piece_can_block() {
    let rules = || white(pilgrims_leap());

    // The pawns on b2 and d2 close the diagonals of the bishop on c1. It jumps over them to
    // a3, and it captures the pawn on e3.
    let fen = "4k3/8/8/8/8/4p3/1P1P4/2B1K3";
    assert_eq!(targets(&mut from_fen(fen, Color::White, rules()), "c1"), squares(&["a3", "e3"]));
    assert!(targets(&mut from_fen(fen, Color::White, Rules::standard()), "c1").is_empty());
    // The jump does not go to a square with a friend: a white pawn on a3.
    assert_eq!(targets(&mut from_fen("4k3/8/8/8/8/P3p3/1P1P4/2B1K3", Color::White, rules()), "c1"), squares(&["e3"]));
    // The bishop on d4 captures the pawn on e5 by its slide, and the knight on f6 by a jump
    // over the pawn. The other jump squares (b2, b6, f2) are also squares of the slides.
    let mut state = from_fen("4k3/8/5n2/4p3/3B4/8/8/4K3", Color::White, rules());
    let expected = squares(&["a1", "b2", "c3", "e3", "f2", "g1", "c5", "b6", "a7", "e5", "f6"]);
    assert_eq!(targets(&mut state, "d4"), expected);
    assert_eq!(targets_from(&state, "d4"), expected);

    // The bishop on c6 gives check to the king on e8 over the pawn on d7.
    let fen = "4k3/3p4/2B5/8/8/8/8/4K3";
    let mut check = from_fen(fen, Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    // The king goes out of the check, or the pawn captures the bishop. A pawn step does not end the check.
    assert_eq!(targets(&mut check, "e8"), squares(&["d8", "e7", "f7", "f8"]));
    assert_eq!(targets(&mut check, "d7"), squares(&["c6"]));
    assert_eq!(legal(&mut check).len(), 5);

    // The bishop on c6 attacks e8 by its slide and by the jump. A piece on d7 blocks only the
    // slide, thus the rook on d2 cannot end the check on d7.
    let fen = "4k3/8/2B5/8/8/8/3r4/K7";
    let mut check = from_fen(fen, Color::Black, rules());
    assert!(targets(&mut check, "d2").is_empty());
    assert_eq!(targets(&mut check, "e8"), squares(&["d8", "e7", "f7", "f8"]));
    assert_eq!(evasion_squares(&check, Color::Black), 1 << square("c6"));
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "d2"), squares(&["d7"]));

    walk_kiwipete(pilgrims_leap());
}

#[test]
fn queens_flight_lets_a_queen_jump_as_a_knight_to_an_empty_square_with_no_check() {
    let rules = || white(queens_flight());

    // The queen on a1 has friends on a2, b1, and b2. Its knight squares are b3 and c2. It
    // jumps to c2, and it does not capture the pawn on b3.
    let fen = "4k3/8/8/8/8/1p6/PP6/QN2K3";
    assert_eq!(targets(&mut from_fen(fen, Color::White, rules()), "a1"), squares(&["c2"]));
    assert!(targets(&mut from_fen(fen, Color::White, Rules::standard()), "a1").is_empty());

    // The queen on d6 is a knight jump from e8 and from f7. The jump attacks nothing: the king
    // on e8 is not in check and can go to f7.
    let mut state = from_fen("4k3/8/3Q4/8/8/8/8/4K2p", Color::Black, rules());
    assert!(!in_check(&state, Color::Black));
    assert!(!is_attacked(&state, square("f7"), Color::White));
    assert_eq!(targets(&mut state, "e8"), squares(&["f7"]));
    assert_eq!(outcome(&mut state), None);

    walk_kiwipete(queens_flight());
}

#[test]
fn huntress_lets_a_queen_capture_and_give_check_as_a_knight() {
    let rules = || white(huntress());

    // The same queen on a1 captures the pawn on b3, and it does not jump to the empty square c2.
    let mut state = from_fen("4k3/8/8/8/8/1p6/PP6/QN2K3", Color::White, rules());
    assert_eq!(targets(&mut state, "a1"), squares(&["b3"]));

    // The queen on d6 gives check to the king on e8 by the knight capture. It also attacks f7,
    // the only square that its slides leave to the king. Thus the queen alone gives mate.
    let fen = "4k3/8/3Q4/8/8/8/8/4K2p";
    let mut mate = from_fen(fen, Color::Black, rules());
    assert!(in_check(&mate, Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    assert_eq!(outcome(&mut mate), Some(Outcome::Checkmate { winner: Color::White }));
    // The king on g8 cannot go to f7, a knight square of the queen. The slides attack f8.
    let fen = "6k1/8/3Q4/8/8/8/8/4K3";
    let mut state = from_fen(fen, Color::Black, rules());
    assert_eq!(targets(&mut state, "g8"), squares(&["g7", "h7", "h8"]));
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "g8"), squares(&["f7", "g7", "h7", "h8"]));

    walk_kiwipete(huntress());
}

#[test]
fn queens_flight_and_huntress_together_give_each_knight_target_one_time() {
    let (quiet, capture) = (Atom::leap(&KNIGHT, Mode::MoveOnly), Atom::leap(&KNIGHT, Mode::CaptureOnly));
    let flight_first = queens_flight().with_atom(Kind::Queen, capture);
    let huntress_first = huntress().with_atom(Kind::Queen, quiet);
    let amazon = SideRules::standard().with_atom(Kind::Queen, Atom::leap(&KNIGHT, Mode::MoveOrCapture));
    let nodes = walk_kiwipete(amazon);
    for side in [flight_first, huntress_first] {
        // The queen on a1 jumps to c2 and captures on b3.
        let mut state = from_fen("4k3/8/8/8/8/1p6/PP6/QN2K3", Color::White, white(side.clone()));
        assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c2"]));
        assert_eq!(targets_from(&state, "a1"), squares(&["b3", "c2"]));
        assert!(in_check(&from_fen("4k3/8/3Q4/8/8/8/8/4K2p", Color::Black, white(side.clone())), Color::Black));
        // The two atoms give the same moves as one knight leap that moves and captures.
        assert_eq!(walk_kiwipete(side), nodes);
    }
}

#[test]
fn gallop_lets_a_knight_make_two_jumps_in_one_direction() {
    let rules = || white(gallop());

    // The lines of the knight on a1 are b3, c5 and c2, e3.
    let mut state = from_fen("7k/8/8/8/8/8/8/N3K3", Color::White, rules());
    assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c5", "c2", "e3"]));
    assert_eq!(targets_from(&state, "a1"), squares(&["b3", "c5", "c2", "e3"]));
    // A white pawn on b3 blocks the first line. The knight captures the black pawn on e3 with its second jump.
    let mut state = from_fen("7k/8/8/8/8/1P2p3/8/N3K3", Color::White, rules());
    assert_eq!(targets(&mut state, "a1"), squares(&["c2", "e3"]));
    // The knight captures a black pawn on b3, and the line stops there.
    let mut state = from_fen("7k/8/8/8/8/1p6/8/N3K3", Color::White, rules());
    assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c2", "e3"]));

    // The knight gives check from two jumps away. A piece of either color on b3 blocks the check.
    let fen = "8/8/8/2k5/8/8/8/N3K3";
    assert!(in_check(&from_fen(fen, Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    assert!(!in_check(&from_fen("8/8/8/2k5/8/1P6/8/N3K3", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("8/8/8/2k5/8/1p6/8/N3K3", Color::Black, rules()), Color::Black));

    // The evasions: the rook on a8 captures the knight, the rook on b8 goes between the knight
    // and the king, or the king moves. The knight and the white king attack no square next to c5.
    let mut check = from_fen("rr6/8/8/2k5/8/8/8/N3K3", Color::Black, rules());
    assert_eq!(evasion_squares(&check, Color::Black), 1 << square("a1") | 1 << square("b3"));
    assert_eq!(targets(&mut check, "a8"), squares(&["a1"]));
    assert_eq!(targets(&mut check, "b8"), squares(&["b3"]));
    assert_eq!(targets(&mut check, "c5").len(), 8);
    assert_eq!(legal(&mut check).len(), 10);

    // A piece between the knight and the king cannot go away: the bishop on b3 has no legal move.
    let fen = "8/8/8/2k5/8/1b6/8/N3K3";
    assert!(targets(&mut from_fen(fen, Color::Black, rules()), "b3").is_empty());
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "b3").len(), 9);

    walk_kiwipete(gallop());
}

#[test]
fn gallop_gives_no_second_jump_to_the_camel_jumps_of_long_leap() {
    let flag_first = SideRules::standard().long_leap().with_atom(Kind::Knight, gallop_atom());
    let gallop_first = gallop().long_leap();
    for side in [flag_first, gallop_first] {
        // The camel jumps of a1 go to b4 and d2. The squares of a second camel jump, c7 and g3,
        // are not targets.
        let mut state = from_fen("7k/8/8/8/8/8/8/N3K3", Color::White, white(side.clone()));
        assert_eq!(targets(&mut state, "a1"), squares(&["b3", "c5", "c2", "e3", "b4", "d2"]));
        // The knight gives check by one camel jump, and not by two.
        assert!(in_check(&from_fen("8/8/8/8/1k6/8/8/N3K3", Color::Black, white(side.clone())), Color::Black));
        assert!(!in_check(&from_fen("8/2k5/8/8/8/8/8/N3K3", Color::Black, white(side.clone())), Color::Black));
        walk_kiwipete(side);
    }
}

#[test]
fn crusade_lets_a_bishop_slide_forward_on_its_file_for_each_side() {
    let rules = || Rules::new(crusade(), crusade());

    // White pawns close the four diagonals of the bishop on e3. It slides up the file and
    // captures the pawn on e6. It does not go down the file to e2.
    let mut state = from_fen("7k/8/4p3/8/3P1P2/4B3/3P1P2/K7", Color::White, rules());
    assert_eq!(targets(&mut state, "e3"), squares(&["e4", "e5", "e6"]));
    // A white pawn on e5 stops the slide.
    let mut state = from_fen("7k/8/8/4P3/3P1P2/4B3/3P1P2/K7", Color::White, rules());
    assert_eq!(targets(&mut state, "e3"), squares(&["e4"]));
    // The mirror for Black: the bishop on e6 slides down the file and captures the pawn on e3.
    let mut state = from_fen("k7/3p1p2/4b3/3p1p2/8/4P3/8/7K", Color::Black, rules());
    assert_eq!(targets(&mut state, "e6"), squares(&["e5", "e4", "e3"]));

    // The white bishop gives check up the file. A piece on the file blocks the check.
    let fen = "4k3/8/8/8/8/4B3/8/K7";
    assert!(in_check(&from_fen(fen, Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    assert!(!in_check(&from_fen("4k3/8/8/4P3/8/4B3/8/K7", Color::Black, rules()), Color::Black));
    // The black bishop gives check down the file. A bishop gives no check to a king behind it.
    assert!(in_check(&from_fen("k7/8/4b3/8/8/8/8/4K3", Color::White, rules()), Color::White));
    assert!(!in_check(&from_fen("8/8/4B3/8/8/4k3/8/K7", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("4K3/8/8/8/8/4b3/8/k7", Color::White, rules()), Color::White));

    // The king on d8 cannot go to the file of the bishop.
    let fen = "3k4/8/8/8/8/4B3/8/K7";
    assert_eq!(targets(&mut from_fen(fen, Color::Black, rules()), "d8"), squares(&["c7", "c8", "d7"]));
    assert_eq!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "d8").len(), 5);
    // The evasions: the king leaves the file, or the rook on a5 goes between the bishop and the king.
    let mut check = from_fen("4k3/8/8/r7/8/4B3/8/7K", Color::Black, rules());
    assert_eq!(targets(&mut check, "e8"), squares(&["d7", "d8", "f7", "f8"]));
    assert_eq!(targets(&mut check, "a5"), squares(&["e5"]));

    walk_kiwipete(crusade());
}

#[test]
fn royal_march_lets_a_king_go_two_squares_in_each_direction() {
    let rules = || white(royal_march());
    let to = |state: &mut State, from: &str| (targets(state, from), targets_from(state, from));

    // The king on d4 goes one or two squares. The white pawn on d5 and the black pawn on d3
    // block the file after one square. The king captures the pawn on d3, and the pawn on b2 with
    // a move of two squares. The leap and the slide of the king give the same eight squares
    // next to it. Each target occurs one time.
    let mut state = from_fen("7k/8/8/3P4/3K4/3p4/1p6/8", Color::White, rules());
    let near = ["c3", "c4", "c5", "d3", "e3", "e4", "e5"];
    let far = ["b2", "b4", "b6", "f2", "f4", "f6"];
    let expected = squares(&[&near[..], &far[..]].concat());
    assert_eq!(to(&mut state, "d4"), (expected.clone(), expected));

    // The engine tests only the target square of a king move, except for a castle. The rook on
    // f8 attacks f1 and f2. The king cannot stop there, but it goes across them to g1 and g3.
    let mut state = from_fen("5r1k/8/8/8/8/8/8/4K3", Color::White, rules());
    assert!(is_attacked(&state, square("f1"), Color::Black) && is_attacked(&state, square("f2"), Color::Black));
    assert_eq!(targets(&mut state, "e1"), squares(&["c1", "c3", "d1", "d2", "e2", "e3", "g1", "g3"]));

    // The king attacks the squares two away, thus the black king cannot go to e6.
    let fen = "8/4k3/8/8/4K3/8/8/8";
    let free = ["d6", "d7", "d8", "e8", "f6", "f7", "f8"];
    assert_eq!(targets(&mut from_fen(fen, Color::Black, rules()), "e7"), squares(&free));
    assert!(targets(&mut from_fen(fen, Color::Black, Rules::standard()), "e7").contains(&square("e6")));
    // The king gives check from two squares away. A piece between the kings blocks the check.
    let fen = "8/8/4k3/8/4K3/8/8/8";
    assert!(in_check(&from_fen(fen, Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen(fen, Color::Black, Rules::standard()), Color::Black));
    assert!(!in_check(&from_fen("8/8/4k3/4p3/4K3/8/8/8", Color::Black, rules()), Color::Black));
    // The evasions: the black king leaves the squares that the white king attacks, or the rook
    // on a5 goes between the kings. The black king cannot capture a king that is two squares away.
    let mut check = from_fen("8/8/4k3/r7/4K3/8/8/8", Color::Black, rules());
    assert_eq!(targets(&mut check, "e6"), squares(&["d6", "d7", "e7", "f6", "f7"]));
    assert_eq!(targets(&mut check, "a5"), squares(&["e5"]));

    walk_kiwipete(royal_march());
}

#[test]
fn royal_march_keeps_the_castles() {
    let rules = || white(royal_march());
    let to_wing = |state: &mut State| -> Vec<(Square, Special)> {
        let mut list: Vec<(Square, Special)> = legal(state)
            .iter()
            .filter(|m| m.from == square("e1") && [square("c1"), square("g1")].contains(&m.to))
            .map(|m| (m.to, m.special))
            .collect();
        list.sort_by_key(|&(to, _)| to);
        list
    };
    let (castle, march) = (Special::Castle, Special::None);

    // The king can also go to c1 and g1 by its own move. When the castle is possible, the castle
    // is the only move to its square.
    let mut state = from_fen("4k3/7p/8/8/8/8/8/R3K2R", Color::White, rules());
    assert_eq!(to_wing(&mut state), vec![(square("c1"), castle), (square("g1"), castle)]);
    // With no rook on h1, the move to g1 is a move of the king alone.
    let mut state = from_fen("4k3/7p/8/8/8/8/8/R3K3", Color::White, rules());
    assert_eq!(to_wing(&mut state), vec![(square("c1"), castle), (square("g1"), march)]);
    // The rook on f8 attacks f1, thus the castle to g1 is not possible. The king still goes to
    // g1 across f1 by its own move, and the rook stays on h1.
    let mut state = from_fen("4kr2/8/8/8/8/8/8/R3K2R", Color::White, rules());
    assert_eq!(to_wing(&mut state), vec![(square("c1"), castle), (square("g1"), march)]);
    state.make(Move::new(square("e1"), square("g1")));
    assert_eq!(state.piece_at(square("h1")).map(|piece| piece.kind), Some(Kind::Rook));
    assert_eq!(state.piece_at(square("f1")), None);
}
