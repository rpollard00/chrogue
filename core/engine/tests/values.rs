//! The material values that the engine derives from the movement rules.

use chrogue_engine::eval::{FIXED_VALUES, kind_profile, officer_profile, officer_value};
use chrogue_engine::rules::{ALFIL, CAMEL, DABBABA, DIAG, FLAG_NAMES, FORWARD, FORWARD_DIAG, KING, KNIGHT, ORTHO};
use chrogue_engine::{
    Atom, Color, EvalTables, EvalVariant, Evaluator, Hook, Kind, MATE_BOUND, Mode, Offset, Promotion, Promotions,
    Rules, SideRules, fen,
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
