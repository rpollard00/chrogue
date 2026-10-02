//! Rules that the TypeScript engine does not have. Each test defines its rules through the
//! public rules data only. This proves that a new movement rule needs no new engine code.

mod common;

use chrogue_engine::fen::{from_fen, square};
use chrogue_engine::rules::{CAMEL, DIAG, KNIGHT, ORTHO};
use chrogue_engine::{Atom, Color, Kind, Mode, Outcome, Rules, SideRules, in_check, is_attacked, outcome, perft};
use common::{legal, squares, targets};

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
    state.turn = Color::Black;
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

    let knights = SideRules::standard().with_pawn(|pawn| pawn.promotions = [Kind::Knight; 4]);
    let mut state = from_fen("7k/4P2p/8/8/8/8/8/4K3", Color::White, white(knights));
    assert!(legal(&mut state).iter().filter(|m| m.from == square("e7")).all(|m| m.promo == Some(Kind::Knight)));
}

#[test]
fn rules_that_are_not_valid_give_an_error() {
    let zero = SideRules::standard().with_atom(Kind::Rook, Atom::leap(&[(0, 0)], Mode::MoveOrCapture));
    assert!(zero.validate().is_err());
    assert!(SideRules::standard().with_pawn(|pawn| pawn.promo_distance = 7).validate().is_err());
    assert!(SideRules::from_flags(["noSuchFlag"]).is_err());
    assert!(SideRules::from_flags(chrogue_engine::rules::FLAG_NAMES).unwrap().validate().is_ok());
}
