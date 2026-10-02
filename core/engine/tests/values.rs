//! The material values that the engine derives from the movement rules.

use chrogue_engine::eval::{officer_profile, officer_value};
use chrogue_engine::reference::FIXED_VALUES;
use chrogue_engine::rules::{CAMEL, DIAG, FLAG_NAMES, KING, KNIGHT, ORTHO};
use chrogue_engine::{Atom, Color, EvalTables, Kind, Mode, Rules, SideRules};

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
    let bishop = officer_profile(&SideRules::standard().kind(Kind::Bishop).atoms);
    let sidestep = officer_profile(&SideRules::standard().sidestep().kind(Kind::Bishop).atoms);
    assert!(bishop.coverage < 0.5 && sidestep.coverage == 1.0);
}

#[test]
fn the_pawn_bonus_grows_toward_the_promotion_zone_of_each_side() {
    let rules = Rules::new(SideRules::standard().early_promo(), SideRules::standard().forced_march());
    let eval = EvalTables::new(&rules);
    let white = &eval.sides[0].pawn_advance;
    let black = &eval.sides[1].pawn_advance;
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
    assert!(standard.sides[0].pawn_advance[6 * 8] > 60, "a pawn on rank 7 of ordinary chess");
}
