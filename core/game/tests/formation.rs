//! The formation of an enemy army: the squares that the battle gives to the kinds of a floor.
//! The player has no win on the first move and no forced win in two moves, for the start army,
//! for a rook on each home square, and for each relic that changes a movement.

use std::collections::HashSet;

use chrogue_game::battle::Battle;
use chrogue_game::chess::{self, Color, Kind, Placement, Square};
use chrogue_game::content::{self, FLOORS, RelicId};
use chrogue_game::formation::{self, Flaw};
use chrogue_game::run::{ENEMY_ID_BASE, Enemy, EnemyPiece, EnemyPieces, Meta, Run, generate_enemy};
use chrogue_game::tuning::Tuning;

fn sq(name: &str) -> Square {
    let b = name.as_bytes();
    (b[0] - b'a') + 8 * (b[1] - b'1')
}

/// A run with the start army (Ke1, Ra1, Ng1, and pawns on c2, d2, e2, f2) before the battle of a floor.
fn run_on(seed: u64, floor: usize) -> Run {
    let tuning = Tuning::default();
    let mut run = Run::new(&Meta::default(), seed, &tuning);
    run.floor = floor;
    run.enemy = generate_enemy(seed, floor, &tuning);
    run
}

/// The flaw of the start of the battle of the run, with the rules of its relics and traits.
fn start_flaw(run: &Run) -> Option<Flaw> {
    let pieces = Battle::placements(run).unwrap();
    let tables = chess::tables(content::rules_for(&run.relics), content::rules_for(&run.enemy.traits)).unwrap();
    formation::flaw(&pieces, &tables).unwrap()
}

fn enemy_of(pieces: &[Placement]) -> Vec<Placement> {
    pieces.iter().copied().filter(|p| p.piece.color == Color::Black).collect()
}

fn against(pieces: &[(Kind, &str)]) -> Run {
    let mut run = run_on(1, 1);
    let pieces = pieces.iter().map(|&(kind, s)| EnemyPiece { kind, square: sq(s) }).collect();
    run.enemy = Enemy { pieces: EnemyPieces::Placed(pieces), traits: vec![] };
    run
}

#[test]
fn flaw_finds_a_check_a_win_on_the_first_move_and_a_forced_win_in_two_moves() {
    const P: Kind = Kind::Pawn;
    // The squares that each kind had before the battle selected the formation. The rook on a1
    // gives checkmate on a8.
    let boxed = against(&[(Kind::King, "e8"), (P, "e7"), (P, "d7"), (P, "f7"), (P, "c7"), (P, "g7")]);
    assert_eq!(start_flaw(&boxed), Some(Flaw::WinInOne));
    // The capture of the only piece of the enemy is a rout.
    assert_eq!(start_flaw(&against(&[(Kind::King, "e8"), (Kind::Rook, "a8")])), Some(Flaw::WinInOne));
    // With a second rook on b1, Rb7 takes rank 7 from the king, and Ra8 is checkmate.
    let mut two = against(&[(Kind::King, "e8"), (P, "d6"), (P, "e6"), (P, "f6")]);
    assert_eq!(start_flaw(&two), None);
    two.add_unit(Kind::Rook);
    two.move_unit(sq("d1"), sq("b1"));
    assert_eq!(start_flaw(&two), Some(Flaw::WinInTwo));

    let mut check = against(&[(Kind::King, "e8"), (P, "a7")]);
    check.move_unit(sq("a1"), sq("e2"));
    assert_eq!(start_flaw(&check), Some(Flaw::Check));
    assert_eq!(start_flaw(&against(&[(Kind::King, "e8"), (P, "e7"), (P, "a7")])), None);
}

#[test]
fn the_start_army_has_no_fast_win_on_each_floor() {
    for floor in 1..=FLOORS.len() {
        for seed in 0..100 {
            assert_eq!(start_flaw(&run_on(seed, floor)), None, "floor {floor} seed {seed}");
        }
    }
}

#[test]
fn a_rook_on_each_home_square_has_no_fast_win() {
    for floor in 1..=FLOORS.len() {
        for seed in 0..25 {
            for home in 0..16 {
                let mut run = run_on(seed, floor);
                run.move_unit(sq("a1"), home);
                assert_eq!(start_flaw(&run), None, "floor {floor} seed {seed} home {home}");
            }
        }
    }
}

#[test]
fn each_relic_that_changes_a_movement_has_no_fast_win() {
    let relics: Vec<RelicId> = RelicId::all().filter(|id| !id.def().rules.is_empty()).collect();
    assert!(relics.len() > 10);
    for &id in &relics {
        for floor in 1..=FLOORS.len() {
            for seed in 0..10 {
                let mut run = run_on(seed, floor);
                run.relics = vec![id];
                assert_eq!(start_flaw(&run), None, "{} floor {floor} seed {seed}", id.key());
            }
        }
    }
}

#[test]
fn the_formation_is_sound_for_the_rules_of_the_relics_of_the_run() {
    let relics: Vec<RelicId> = RelicId::all().filter(|id| !id.def().rules.is_empty()).collect();
    // The number of starts where the formation for the rules of chess has a flaw with the relic.
    let mut differ = 0;
    for &id in &relics {
        for seed in 0..6 {
            for home in 0..16 {
                let mut plain = run_on(seed, 3);
                plain.move_unit(sq("g1"), home);
                let mut run = plain.clone();
                run.relics = vec![id];
                assert_eq!(start_flaw(&run), None, "{} seed {seed} home {home}", id.key());

                let tables = chess::tables(content::rules_for(&run.relics), content::rules_for(&[])).unwrap();
                let flaw = formation::flaw(&Battle::placements(&plain).unwrap(), &tables).unwrap();
                differ += flaw.is_some() as u32;
            }
        }
    }
    assert!(differ > 0);
}

#[test]
fn the_battle_takes_the_smallest_flaw_if_each_formation_has_a_flaw() {
    // A queen on each file of rank 2. The king of a loose formation is in check, unless the pawn
    // is in front of it. The shielded formations have the pawn on e7.
    let mut run = run_on(1, 1);
    run.army.retain(|unit| unit.kind == Kind::King);
    for home in 8..16 {
        run.add_unit(Kind::Queen);
        run.move_unit(run.army.last().unwrap().home, home);
    }
    run.enemy.pieces = EnemyPieces::Kinds(vec![Kind::King, Kind::Pawn]);
    let flaw = start_flaw(&run);
    assert!(matches!(flaw, Some(Flaw::WinInOne | Flaw::WinInTwo)), "{flaw:?}");
}

#[test]
fn the_formation_has_the_kinds_of_the_army_on_the_last_two_ranks() {
    for floor in 1..=FLOORS.len() {
        for seed in 0..50 {
            let run = run_on(seed, floor);
            let enemy = enemy_of(&Battle::placements(&run).unwrap());
            let kinds: Vec<Kind> = enemy.iter().map(|p| p.piece.kind).collect();
            assert_eq!(kinds, run.enemy.pieces.kinds());
            let ids: Vec<u16> = enemy.iter().map(|p| p.piece.id).collect();
            assert_eq!(ids, (0..enemy.len() as u16).map(|i| ENEMY_ID_BASE + i).collect::<Vec<_>>());
            assert_eq!(enemy.iter().map(|p| p.square).collect::<HashSet<_>>().len(), enemy.len());
            for p in &enemy {
                let rank = if p.piece.kind == Kind::Pawn { 6 } else { 7 };
                assert_eq!(p.square / 8, rank, "floor {floor} seed {seed}: {:?} on {}", p.piece.kind, p.square);
            }
        }
    }
}

#[test]
fn the_same_run_gives_the_same_formation_and_a_different_army_can_give_a_different_one() {
    // The squares of the enemy of floor 2, for each of 40 seeds.
    let boards = |change: fn(&mut Run)| -> Vec<Vec<Square>> {
        (0..40)
            .map(|seed| {
                let mut run = run_on(seed, 2);
                change(&mut run);
                enemy_of(&Battle::placements(&run).unwrap()).iter().map(|p| p.square).collect()
            })
            .collect()
    };
    let first = boards(|_| {});
    assert_eq!(first, boards(|_| {}));
    assert_eq!(first, boards(|run| run.gold += 5));
    // The same kinds on another floor get other formations.
    assert_ne!(first, boards(|run| run.floor = 3));
    // With the rook on h1, some of the formations have a flaw, and the battle takes other ones.
    assert_ne!(first, boards(|run| _ = run.move_unit(sq("a1"), sq("h1"))));
    // The seed of the run selects the formation.
    assert!(first.iter().collect::<HashSet<_>>().len() > 10);
}

#[test]
fn a_king_with_no_pawn_leaves_the_file_of_a_rook() {
    for seed in 0..20 {
        let mut run = run_on(seed, 1);
        run.move_unit(sq("a1"), sq("e2"));
        run.army.retain(|unit| unit.kind != Kind::Pawn);
        run.enemy.pieces = EnemyPieces::Kinds(vec![Kind::King, Kind::Rook]);
        assert_eq!(start_flaw(&run), None, "seed {seed}");
    }
}

#[test]
fn an_army_that_does_not_fit_on_the_last_two_ranks_has_no_formation() {
    let with = |kinds: Vec<Kind>| {
        let mut run = run_on(1, 1);
        run.enemy.pieces = EnemyPieces::Kinds(kinds);
        Battle::new(&run).map(|_| ())
    };
    let army = |officers: usize, pawns: usize| {
        let mut kinds = vec![Kind::King];
        kinds.extend(std::iter::repeat_n(Kind::Queen, officers));
        kinds.extend(std::iter::repeat_n(Kind::Pawn, pawns));
        kinds
    };
    assert_eq!(with(army(7, 8)), Ok(()));
    assert_eq!(with(army(0, 0)), Ok(()));
    assert!(with(army(8, 0)).is_err());
    assert!(with(army(0, 9)).is_err());
    assert!(with(vec![Kind::Queen]).is_err());
    assert!(with(vec![Kind::King, Kind::King]).is_err());
}

#[test]
fn an_enemy_with_set_squares_keeps_its_squares() {
    // The formation of a floor does not have these squares, and the start has a flaw.
    let run = against(&[(Kind::King, "e8"), (Kind::Rook, "a8"), (Kind::Pawn, "h3")]);
    let enemy: Vec<Square> = enemy_of(&Battle::placements(&run).unwrap()).iter().map(|p| p.square).collect();
    assert_eq!(enemy, vec![sq("e8"), sq("a8"), sq("h3")]);
}
