//! The material values that the engine derives from the movement rules.

use chrogue_engine::eval::{FIXED_VALUES, kind_profile, officer_profile, officer_value};
use chrogue_engine::fen::{KIWIPETE, START, square};
use chrogue_engine::movegen::{condition_holds, shielded};
use chrogue_engine::rules::{ALFIL, CAMEL, DABBABA, DIAG, FLAG_NAMES, FORWARD, FORWARD_DIAG, KING, KNIGHT, ORTHO};
use chrogue_engine::{
    Atom, Color, Condition, EvalTables, EvalVariant, Evaluator, Flaws, Hook, Kind, Level, Limits, MATE_BOUND, Mode,
    Move, Offset, Promotion, Promotions, Rules, SearchOptions, Shield, SideRules, fen, search,
};

/// The values of White with these rules.
fn values(white: SideRules) -> [i32; 6] {
    let eval = EvalTables::new(&Rules::new(white, SideRules::standard()));
    Kind::ALL.map(|kind| eval.value(Color::White, kind))
}

fn value(white: SideRules, kind: Kind) -> i32 {
    values(white)[kind.index()]
}

#[test]
fn the_values_of_ordinary_chess_are_near_the_usual_values() {
    let derived = values(SideRules::standard());
    for kind in [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen] {
        let (got, usual) = (derived[kind.index()] as f64, FIXED_VALUES[kind.index()] as f64);
        assert!((got - usual).abs() <= 0.15 * usual, "{kind:?} has the value {got}, but the usual value is {usual}");
    }
    // The order of the kinds is the usual order.
    assert!(derived[0] < derived[1] && derived[1] <= derived[2] && derived[2] < derived[3] && derived[3] < derived[4]);
}

#[test]
fn the_two_sides_get_the_same_values_from_the_same_rules() {
    for flags in [vec![], vec!["longLeap"], FLAG_NAMES.to_vec()] {
        let side = || SideRules::from_flags(flags.iter().copied()).unwrap();
        let eval = EvalTables::new(&Rules::new(side(), side()));
        for kind in Kind::ALL {
            assert_eq!(eval.value(Color::White, kind), eval.value(Color::Black, kind), "{kind:?} with {flags:?}");
        }
    }
    // The rules of one side do not change the values of the other side.
    let eval = EvalTables::new(&Rules::new(SideRules::standard().long_leap(), SideRules::standard()));
    assert!(eval.value(Color::White, Kind::Knight) > eval.value(Color::Black, Kind::Knight));
    assert_eq!(eval.value(Color::Black, Kind::Knight), value(SideRules::standard(), Kind::Knight));
}

#[test]
fn each_rule_flag_for_a_kind_makes_the_value_of_that_kind_higher() {
    let standard = values(SideRules::standard());
    let cases = [
        ("kingKnight", Kind::King),
        ("longLeap", Kind::Knight),
        ("sidestep", Kind::Bishop),
        ("forcedMarch", Kind::Pawn),
        ("earlyPromo", Kind::Pawn),
        ("backpedal", Kind::Pawn),
    ];
    for (flag, kind) in cases {
        let with_flag = values(SideRules::from_flags([flag]).unwrap());
        assert!(with_flag[kind.index()] > standard[kind.index()], "{flag} did not make {kind:?} more valuable");
        for other in Kind::ALL {
            if other != kind {
                assert_eq!(with_flag[other.index()], standard[other.index()], "{flag} changed {other:?}");
            }
        }
    }
}

#[test]
fn a_hook_makes_the_value_of_its_kind_higher() {
    let standard = values(SideRules::standard());
    let hook = |mode| values(SideRules::standard().with_hook(Kind::Rook, Hook::right_angle(&ORTHO, 2, mode)));
    let (both, quiet) = (hook(Mode::MoveOrCapture), hook(Mode::MoveOnly));
    let rook = Kind::Rook.index();
    // A hook that captures is worth more than a hook that only moves. The rook stays below the queen.
    assert!(standard[rook] < quiet[rook] && quiet[rook] < both[rook] && both[rook] < standard[Kind::Queen.index()]);
    for other in [Kind::Knight, Kind::Bishop, Kind::Queen] {
        assert_eq!(both[other.index()], standard[other.index()], "the hook changed {other:?}");
    }
    // A hook with a longer leg at the least has fewer targets.
    let long = values(SideRules::standard().with_hook(Kind::Rook, Hook::right_angle(&ORTHO, 4, Mode::MoveOrCapture)));
    assert!(standard[rook] < long[rook] && long[rook] < both[rook]);
}

#[test]
fn an_atom_for_a_piece_near_another_piece_counts_for_a_part_of_its_reach() {
    let bishop = |atom: Option<Atom>| {
        values(atom.into_iter().fold(SideRules::standard(), |side, atom| side.with_atom(Kind::Bishop, atom)))
            [Kind::Bishop.index()]
    };
    let slide = || Atom::slide(&ORTHO, Mode::MoveOrCapture);
    let (plain, near, always) = (bishop(None), bishop(Some(slide().if_near(Kind::King, 1))), bishop(Some(slide())));
    assert!(plain < near && near < always, "{plain} {near} {always}");
    // The bishop with the condition is nearer to the bishop of chess than to the queen.
    assert!(near - plain < always - near, "{plain} {near} {always}");
}

#[test]
fn each_relic_atom_makes_the_value_of_its_kind_higher() {
    let standard = values(SideRules::standard());
    let with_atoms = |kind, atoms: &[Atom]| {
        values(atoms.iter().fold(SideRules::standard(), |side, atom| side.with_atom(kind, atom.clone())))
    };
    let flight = Atom::leap(&KNIGHT, Mode::MoveOnly);
    let huntress = Atom::leap(&KNIGHT, Mode::CaptureOnly);
    let cases = [
        (Kind::Rook, Atom::leap(&DABBABA, Mode::MoveOnly)),
        (Kind::Rook, Atom::leap(&DIAG, Mode::CaptureOnly)),
        (Kind::Knight, Atom::leap(&ORTHO, Mode::CaptureOnly)),
        (Kind::Bishop, Atom::leap(&ALFIL, Mode::MoveOrCapture)),
        (Kind::Queen, flight.clone()),
        (Kind::Queen, huntress.clone()),
        (Kind::Knight, Atom::slide(&KNIGHT, Mode::MoveOrCapture).max_steps(2)),
        (Kind::Bishop, Atom::slide(&FORWARD, Mode::MoveOrCapture)),
        (Kind::King, Atom::slide(&KING, Mode::MoveOrCapture).max_steps(2)),
        (Kind::Pawn, Atom::leap(&FORWARD, Mode::CaptureOnly)),
        (Kind::Pawn, Atom::leap(&FORWARD_DIAG, Mode::MoveOnly)),
    ];
    for (kind, atom) in cases {
        let with_atom = with_atoms(kind, std::slice::from_ref(&atom));
        assert!(with_atom[kind.index()] > standard[kind.index()], "{atom:?} did not make {kind:?} more valuable");
        for other in Kind::OFFICERS {
            if other != kind {
                assert_eq!(with_atom[other.index()], standard[other.index()], "{atom:?} changed {other:?}");
            }
        }
        // The pawn has the value of the queen in its promotion term.
        assert!(with_atom[Kind::Pawn.index()] >= standard[Kind::Pawn.index()], "{atom:?} made the pawn less valuable");
        // No atom makes a knight, a bishop, or a rook as valuable as the queen of ordinary chess.
        if !matches!(kind, Kind::Queen | Kind::King) {
            assert!(with_atom[kind.index()] < standard[Kind::Queen.index()], "{atom:?} makes {kind:?} a queen");
        }
    }

    // A knight jump that only moves is worth less than a knight jump that only captures. The
    // two atoms together are worth the same as one knight jump that moves and captures.
    let queen = |atoms: &[Atom]| with_atoms(Kind::Queen, atoms)[Kind::Queen.index()];
    assert!(queen(std::slice::from_ref(&flight)) < queen(std::slice::from_ref(&huntress)));
    let both = queen(&[flight, huntress.clone()]);
    assert!(queen(&[huntress]) < both);
    assert_eq!(both, queen(&[Atom::leap(&KNIGHT, Mode::MoveOrCapture)]));
}

#[test]
fn a_new_atom_never_makes_a_value_lower() {
    let modes = [Mode::MoveOrCapture, Mode::MoveOnly, Mode::CaptureOnly];
    let mut atoms = Vec::new();
    for mode in modes {
        atoms.push(Atom::leap(&KNIGHT, mode));
        atoms.push(Atom::leap(&CAMEL, mode));
        atoms.push(Atom::leap(&KING, mode));
        atoms.push(Atom::leap(&[(0, 1)], mode));
        atoms.push(Atom::leap(&[(7, 7), (-7, 0)], mode));
        atoms.push(Atom::slide(&ORTHO, mode));
        atoms.push(Atom::slide(&DIAG, mode));
        atoms.push(Atom::slide(&[(1, 2), (0, -1)], mode));
    }
    let mut checks = 0;
    for kind in Kind::OFFICERS {
        let base = SideRules::standard();
        for first in &atoms {
            let one = base.clone().with_atom(kind, first.clone());
            assert!(value(one.clone(), kind) >= value(base.clone(), kind), "{first:?} made {kind:?} less valuable");
            for second in &atoms {
                let two = one.clone().with_atom(kind, second.clone());
                assert!(value(two, kind) >= value(one.clone(), kind), "{second:?} after {first:?} for {kind:?}");
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 5 * 24 * 24);

    // An atom that the kind has already adds no value.
    let twice = SideRules::standard().with_atom(Kind::Rook, Atom::slide(&ORTHO, Mode::MoveOrCapture));
    assert_eq!(value(twice, Kind::Rook), value(SideRules::standard(), Kind::Rook));
    // A pawn with a better promotion kind is not less valuable.
    let strong_queen = SideRules::standard().with_atom(Kind::Queen, Atom::leap(&KNIGHT, Mode::MoveOrCapture));
    assert!(value(strong_queen, Kind::Pawn) > value(SideRules::standard(), Kind::Pawn));
}

#[test]
fn reach_that_can_only_move_counts_less_than_reach_that_can_capture() {
    for (offsets, slide) in [(&KNIGHT[..], false), (&ORTHO[..], true), (&DIAG[..], true), (&CAMEL[..], false)] {
        let atom = |mode| if slide { vec![Atom::slide(offsets, mode)] } else { vec![Atom::leap(offsets, mode)] };
        let quiet = officer_profile(&atom(Mode::MoveOnly)).reach;
        let capture = officer_profile(&atom(Mode::CaptureOnly)).reach;
        let both = officer_profile(&atom(Mode::MoveOrCapture)).reach;
        assert!(0.0 < quiet && quiet < capture && capture < both, "{offsets:?}: {quiet} {capture} {both}");
        assert!(officer_value(&atom(Mode::MoveOnly)) < officer_value(&atom(Mode::CaptureOnly)));
        assert!(officer_value(&atom(Mode::CaptureOnly)) < officer_value(&atom(Mode::MoveOrCapture)));
    }
    // The sidestep of the bishop is a `MoveOnly` leap. The same leap with captures is worth more.
    let sidestep = value(SideRules::standard().sidestep(), Kind::Bishop);
    let with_captures =
        value(SideRules::standard().with_atom(Kind::Bishop, Atom::leap(&ORTHO, Mode::MoveOrCapture)), Kind::Bishop);
    assert!(sidestep < with_captures);
}

#[test]
fn a_bishop_that_can_get_to_each_square_is_worth_more_than_its_added_reach() {
    let bishop = officer_profile(SideRules::standard().atoms(Kind::Bishop));
    let sidestep = officer_profile(SideRules::standard().sidestep().atoms(Kind::Bishop));
    assert!(bishop.coverage < 0.5 && sidestep.coverage == 1.0);
}

#[test]
fn the_pawn_bonus_grows_toward_the_promotion_zone_of_each_side() {
    let rules = Rules::new(SideRules::standard().early_promo(), SideRules::standard().forced_march());
    let eval = EvalTables::new(&rules);
    let white = &eval.sides[0].advance[Kind::Pawn.index()];
    let black = &eval.sides[1].advance[Kind::Pawn.index()];
    // White promotes on rank 7: a pawn on rank 6 is one move from the zone.
    for rank in 2..5 {
        assert!(white[rank * 8 + 8] > white[rank * 8], "white rank {}", rank + 1);
    }
    // With the double step, rank 2 and rank 3 are the same number of moves from the zone.
    assert_eq!(white[2 * 8], white[8]);
    assert_eq!(white[8], 0);
    assert_eq!(white[6 * 8], 0, "rank 7 is in the promotion zone of White");
    // Black can always move two squares: a pawn on rank 3 and a pawn on rank 2 are one move from rank 1.
    assert_eq!(black[2 * 8], black[8]);
    assert!(black[2 * 8] > black[3 * 8 + 8]);
    assert_eq!(black[6 * 8], 0);
    let standard = EvalTables::new(&Rules::standard());
    assert!(standard.sides[0].advance[Kind::Pawn.index()][6 * 8] > 60, "a pawn on rank 7 of ordinary chess");
}

#[test]
fn no_derived_value_is_nan_or_negative_for_each_promotion_distance() {
    // With the distance 6, the zone starts on the second rank: the pawn has no square before
    // its zone, and it promotes on its first step.
    for distance in 0..=6 {
        let promotion = Some(Promotion { distance, kinds: Promotions::STANDARD });
        for pawn in [SideRules::standard(), SideRules::standard().backpedal(), SideRules::standard().forced_march()] {
            let side = pawn.with_promotion(Kind::Pawn, promotion);
            assert!(side.validate().is_ok(), "the distance {distance} is valid");
            let profile = kind_profile(side.kind(Kind::Pawn));
            assert!(profile.reach >= 0.0 && profile.coverage >= 0.0, "distance {distance}: {profile:?}");
            let eval = EvalTables::new(&Rules::new(side.clone(), side));
            for (color, tables) in Color::ALL.iter().zip(&eval.sides) {
                for kind in Kind::ALL {
                    let (value, reach) = (tables.value[kind.index()], tables.reach[kind.index()]);
                    assert!(value >= 0 && reach >= 0.0, "distance {distance}, {color:?} {kind:?}: {value} {reach}");
                }
                assert!(tables.value[Kind::Pawn.index()] > 0, "distance {distance}: the pawn has no value");
            }
        }
    }
}

#[test]
fn the_evaluation_is_never_the_score_of_a_win() {
    // A queen that also leaps to each square of the board. Fifteen of them have more material
    // than the score of a win.
    let everywhere: Vec<Offset> =
        (-7..=7).flat_map(|df| (-7..=7).map(move |dr| (df, dr))).filter(|&offset| offset != (0, 0)).collect();
    let side = SideRules::standard().with_atom(Kind::Queen, Atom::leap(&everywhere, Mode::MoveOrCapture));
    let rules = Rules::new(side, SideRules::standard());
    assert!(15 * EvalTables::new(&rules).value(Color::White, Kind::Queen) > MATE_BOUND);
    for turn in Color::ALL {
        let state = fen::from_fen("7k/7p/8/QQQQQQQQ/QQQQQQQ1/8/8/K7", turn, rules.clone()).unwrap();
        let score = Evaluator::new(&state, EvalVariant::Derived).evaluate(&state);
        assert!(score.abs() < MATE_BOUND, "{turn:?}: the evaluation is {score}");
    }
}

#[test]
fn the_threat_term_does_not_count_a_piece_with_a_shield() {
    // The rook on d1 attacks the pawn on d5, and no piece of Black defends the pawn.
    let score = |black: SideRules| {
        let state =
            fen::from_fen("4k3/8/3b4/3p4/8/8/8/3RK3", Color::White, Rules::new(SideRules::standard(), black)).unwrap();
        Evaluator::new(&state, EvalVariant::Derived).evaluate(&state)
    };
    let shield = chrogue_engine::Shield { protector: Kind::Bishop, protected: Kind::Pawn, range: 1 };
    assert!(score(SideRules::standard().with_shield(shield)) < score(SideRules::standard()));
}

// ---- The formation term ----

/// The bishop also slides as a rook while its king is `range` squares away or less.
fn near_king(range: u8) -> Atom {
    Atom::slide(&ORTHO, Mode::MoveOrCapture).if_near(Kind::King, range)
}

fn with_near_bishop() -> SideRules {
    SideRules::standard().with_atom(Kind::Bishop, near_king(1))
}

fn with_shield(protected: Kind, range: u8) -> SideRules {
    SideRules::standard().with_shield(Shield { protector: Kind::Bishop, protected, range })
}

/// The formation term of a position for White: the evaluation with the term minus the
/// evaluation without it. White has the move, thus the other terms cancel.
fn formation_term(pieces: &str, rules: Rules) -> i32 {
    let state = fen::from_fen(pieces, Color::White, rules).unwrap();
    let with_term = Evaluator::new(&state, EvalVariant::Derived);
    let mut without = with_term.clone();
    without.forget_formation();
    with_term.evaluate(&state) - without.evaluate(&state)
}

/// The position with the colours and the ranks exchanged.
fn mirror(pieces: &str) -> String {
    let swap = |ch: char| if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() } else { ch.to_ascii_uppercase() };
    pieces.split('/').rev().map(|row| row.chars().map(swap).collect::<String>()).collect::<Vec<_>>().join("/")
}

/// Positions with pawns and bishops of the two sides at different distances.
const FORMATION_POSITIONS: [&str; 6] = [
    START,
    KIWIPETE,
    "4k3/8/8/8/8/2P5/3B4/4K3",
    "r1bqk2r/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP3PPP/RN1QK2R",
    "4k3/2b5/1p1p4/p3p3/P3P1B1/1P1P1P2/2B3P1/4K3",
    "2b1k3/pp4pp/8/2PpP3/3B4/8/PP1B2PP/4K3",
];

#[test]
fn rules_with_no_formation_have_no_formation_term() {
    let mut sides = vec![SideRules::standard(), SideRules::from_flags(FLAG_NAMES).unwrap()];
    sides.extend(FLAG_NAMES.map(|flag| SideRules::from_flags([flag]).unwrap()));
    for side in sides {
        let rules = Rules::new(side.clone(), side);
        assert!(EvalTables::new(&rules).sides.iter().all(|side| side.formation.is_empty()));
        for pieces in FORMATION_POSITIONS {
            assert_eq!(formation_term(pieces, rules.clone()), 0, "{pieces}");
        }
    }
}

#[test]
fn the_formation_term_grows_as_a_piece_gets_near_its_anchor() {
    let term = |pieces| formation_term(pieces, Rules::new(with_near_bishop(), SideRules::standard()));
    let (d1, c1, b1, a1) = (
        term("4k3/8/8/8/8/8/8/3BK3"),
        term("4k3/8/8/8/8/8/8/2B1K3"),
        term("4k3/8/8/8/8/8/8/1B2K3"),
        term("4k3/8/8/8/8/8/8/B3K3"),
    );
    assert!(d1 > c1 && c1 > b1 && b1 > a1, "{d1} {c1} {b1} {a1}");
    assert_eq!(a1, 0);
    // The king is not the anchor of a second bishop that is far from it.
    assert_eq!(term("4k3/8/8/8/8/8/8/B2BK3"), d1);
    // A bishop is not the anchor of a bishop: the atom names the king.
    assert_eq!(term("4k3/8/8/8/8/8/BB6/7K"), 0);
}

#[test]
fn the_size_of_the_formation_term_comes_from_the_rule() {
    let bishop_next_to_king = "4k3/8/8/8/8/8/8/3BK3";
    let near = |atom: Atom| {
        let side = SideRules::standard().with_atom(Kind::Bishop, atom);
        formation_term(bishop_next_to_king, Rules::new(side, SideRules::standard()))
    };
    // An atom that adds less reach gives a smaller term.
    let (slide, step) = (near(near_king(1)), near(near_king(1).max_steps(1)));
    assert!(0 < step && step < slide, "{step} {slide}");

    // A shield of a more valuable kind gives a larger term.
    let shield =
        |protected, pieces| formation_term(pieces, Rules::new(with_shield(protected, 1), SideRules::standard()));
    let (pawn, queen) = (shield(Kind::Pawn, "4k3/8/8/8/8/2P5/3B4/4K3"), shield(Kind::Queen, "4k3/8/8/8/8/2Q5/3B4/4K3"));
    assert!(0 < pawn && pawn < queen, "{pawn} {queen}");
    // Each piece with the shield counts.
    assert_eq!(shield(Kind::Pawn, "4k3/8/8/8/8/2P1P3/3B4/4K3"), 2 * pawn);
    // Two shields of the same kinds are one row with the larger range.
    let twice =
        with_shield(Kind::Pawn, 1).with_shield(Shield { protector: Kind::Bishop, protected: Kind::Pawn, range: 2 });
    for pieces in FORMATION_POSITIONS {
        let wide = formation_term(pieces, Rules::new(with_shield(Kind::Pawn, 2), SideRules::standard()));
        assert_eq!(formation_term(pieces, Rules::new(twice.clone(), SideRules::standard())), wide, "{pieces}");
    }
    // A pawn that is far from each bishop adds nothing.
    assert_eq!(shield(Kind::Pawn, "4k3/8/8/7P/8/2P5/3B4/4K3"), pawn);
}

#[test]
fn the_pieces_in_formation_by_a_shield_are_the_pieces_with_the_shield() {
    for range in [1, 2] {
        let side = with_shield(Kind::Pawn, range);
        let rules = Rules::new(side.clone(), side);
        let mut with_a_shield = 0;
        for pieces in FORMATION_POSITIONS {
            let state = fen::from_fen(pieces, Color::White, rules.clone()).unwrap();
            let eval = Evaluator::new(&state, EvalVariant::Derived);
            for color in Color::ALL {
                assert_eq!(eval.formed(&state, color), shielded(&state, color), "{pieces}, {color:?}, range {range}");
                with_a_shield += shielded(&state, color).count_ones();
            }
        }
        assert!(with_a_shield > 20, "range {range}: {with_a_shield}");
    }
    // The term of White is the number of pawns with the shield times the term of one such
    // pawn, when no other pawn is one or two squares too far from a bishop.
    let rules = Rules::new(with_shield(Kind::Pawn, 1), SideRules::standard());
    let one = formation_term("4k3/8/8/8/8/2P5/3B4/4K3", rules.clone());
    for pieces in ["4k3/8/8/8/2P1P3/3B4/2P1P3/4K3", "4k3/8/8/8/8/1PPP4/2B5/4K3", "4k3/P7/8/8/8/8/4PPP1/4KB2"] {
        let state = fen::from_fen(pieces, Color::White, rules.clone()).unwrap();
        let count = shielded(&state, Color::White).count_ones() as i32;
        assert!(count > 0);
        assert_eq!(formation_term(pieces, rules.clone()), count * one, "{pieces}");
    }
}

#[test]
fn the_formation_term_is_the_same_for_the_two_colours() {
    let sides = [
        with_near_bishop(),
        with_shield(Kind::Pawn, 1),
        with_shield(Kind::Pawn, 2).with_atom(Kind::Bishop, near_king(2)),
    ];
    for side in sides {
        let mut sum = 0;
        for pieces in FORMATION_POSITIONS {
            let white = formation_term(pieces, Rules::new(side.clone(), SideRules::standard()));
            let black = formation_term(&mirror(pieces), Rules::new(SideRules::standard(), side.clone()));
            assert_eq!(black, -white, "{pieces}");
            sum += white;
        }
        assert!(sum > 0);
    }
}

/// The move of a search of the levels with depth 1 and no flaw, with or without the formation term.
fn formation_move(pieces: &str, rules: Rules, formation: bool) -> Move {
    let mut state = fen::from_fen(pieces, Color::White, rules).unwrap();
    let options = SearchOptions { formation, ..Level::OPTIONS };
    search(&mut state, &Limits::depth(1), EvalVariant::Derived, options, Flaws::NONE, 1).expect("a legal move").mv
}

#[test]
fn the_search_moves_a_piece_into_formation() {
    // The bishop on d3 attacks no piece. From f1 it is next to its king, and its lines are not open.
    let pieces = "6k1/5ppp/8/8/8/3B4/5PPP/6K1";
    let rules = Rules::new(with_near_bishop(), SideRules::standard());
    let in_formation_after = |m: Move| {
        let mut state = fen::from_fen(pieces, Color::White, rules.clone()).unwrap();
        state.make(m);
        let bishop = state.pieces(Color::White, Kind::Bishop).trailing_zeros() as u8;
        condition_holds(&state, Condition::Near { kind: Kind::King, range: 1 }, Color::White, bishop)
    };
    let with_term = formation_move(pieces, rules.clone(), true);
    assert_eq!((with_term.from, with_term.to), (square("d3"), square("f1")));
    assert!(in_formation_after(with_term));
    // The search without the term does not go there.
    assert!(!in_formation_after(formation_move(pieces, rules.clone(), false)));

    // A bishop that cannot get next to its king in one move goes one square nearer.
    let far = formation_move("6k1/5ppp/8/8/3B4/8/5PPP/6K1", rules.clone(), true);
    assert_eq!((far.from, far.to), (square("d4"), square("e3")));
}

#[test]
fn the_search_leaves_a_formation_to_capture_a_piece() {
    // The bishop on d2 is next to its king, thus it slides as a rook. The rook on d6 has no defender.
    let rules = Rules::new(with_near_bishop(), SideRules::standard());
    for formation in [true, false] {
        let m = formation_move("4k3/p7/3r4/8/8/8/3B4/3K4", rules.clone(), formation);
        assert_eq!((m.from, m.to), (square("d2"), square("d6")));
    }
}

#[test]
fn each_near_condition_of_a_kind_has_its_own_row() {
    // The slide starts at the distance 1 from the king, and the leap starts at the distance 3.
    let (slide, leap) = (near_king(1), Atom::leap(&KNIGHT, Mode::MoveOrCapture).if_near(Kind::King, 3));
    let term = |atoms: &[&Atom], pieces: &str| {
        let side = atoms.iter().fold(SideRules::standard(), |side, &atom| side.with_atom(Kind::Bishop, atom.clone()));
        formation_term(pieces, Rules::new(side, SideRules::standard()))
    };
    // The bishop at the distances 1, 2, 3, and 4 from its king.
    let distances = ["4k3/8/8/8/8/8/8/3BK3", "4k3/8/8/8/8/8/8/2B1K3", "4k3/8/8/8/8/8/8/1B2K3", "4k3/8/8/8/8/8/8/B3K3"];
    let both = distances.map(|pieces| term(&[&slide, &leap], pieces));
    // The term grows at each step toward the king, also inside the range of the leap.
    assert!(both[0] > both[1] && both[1] > both[2] && both[2] > both[3] && both[3] > 0, "{both:?}");
    // The two rows add: the bishop next to its king gets more than from one of the atoms.
    let alone = distances.map(|pieces| term(&[&slide], pieces));
    assert!(both[0] > alone[0] && both[0] > term(&[&leap], distances[0]), "{both:?} {alone:?}");
    // A second atom with the same condition makes the row larger.
    let step = Atom::leap(&ALFIL, Mode::MoveOrCapture).if_near(Kind::King, 1);
    let one_row = term(&[&slide, &step], distances[0]);
    assert!(one_row > alone[0], "{one_row} {alone:?}");
}

#[test]
fn a_piece_is_not_its_own_anchor() {
    let side = SideRules::standard()
        .with_atom(Kind::Bishop, Atom::slide(&ORTHO, Mode::MoveOrCapture).if_near(Kind::Bishop, 1));
    let term = |pieces| formation_term(pieces, Rules::new(side.clone(), SideRules::standard()));
    // One bishop has no other bishop near it, also next to its king.
    assert_eq!(term("4k3/8/8/8/8/8/8/3BK3"), 0);
    // Each of two bishops on adjacent squares is the anchor of the other bishop.
    let pair = term("4k3/8/8/8/8/8/8/1BB1K3");
    assert!(pair > 0 && pair % 2 == 0, "{pair}");
    // A third bishop that is far from the two adds nothing, and one bishop of the pair is half.
    assert_eq!(term("4k3/8/8/8/7B/8/8/1BB1K3"), pair);
    assert_eq!(term("4k3/8/8/8/8/8/8/1BBBK3"), 3 * pair / 2);
}

#[test]
fn the_anchor_of_the_formation_term_is_the_kind_of_the_condition() {
    let side =
        SideRules::standard().with_atom(Kind::Bishop, Atom::slide(&ORTHO, Mode::MoveOrCapture).if_near(Kind::Rook, 1));
    let term = |pieces| formation_term(pieces, Rules::new(side.clone(), SideRules::standard()));
    // The bishop is next to its king, and the side has no rook.
    assert_eq!(term("4k3/8/8/8/8/8/8/3BK3"), 0);
    // The rook is far from the bishop.
    assert_eq!(term("4k3/R7/8/8/8/8/8/3BK3"), 0);
    // The rook is next to the bishop, and the king is far.
    assert!(term("4k3/8/8/8/8/1BR5/8/7K") > 0);
}

#[test]
fn one_row_of_an_atom_pays_half_of_the_value_of_its_kind_at_most() {
    // A knight that also leaps to each square of the board while it is next to its king. A
    // fourth of the added value is more than half of the value of this knight.
    let everywhere: Vec<Offset> =
        (-7..=7).flat_map(|df| (-7..=7).map(move |dr| (df, dr))).filter(|&offset| offset != (0, 0)).collect();
    let leap = Atom::leap(&everywhere, Mode::MoveOrCapture);
    let side = SideRules::standard().with_atom(Kind::Knight, leap.clone().if_near(Kind::King, 1));
    let knight = value(side.clone(), Kind::Knight);
    let added =
        officer_value(&[Atom::leap(&KNIGHT, Mode::MoveOrCapture), leap]) - value(SideRules::standard(), Kind::Knight);
    assert!(added / 4 > knight / 2, "{added} {knight}");
    assert_eq!(formation_term("4k3/8/8/8/8/8/8/3NK3", Rules::new(side, SideRules::standard())), knight / 2);
}
