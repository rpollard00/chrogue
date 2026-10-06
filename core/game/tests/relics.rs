//! The relics: the move that each rule relic gives to the player and to the enemy, the effects
//! after a battle, and the relic slots of a run.

use chrogue_game::battle::{Battle, BattlePhase, BattleReward, Bonus, MoveReport, Recruit};
use chrogue_game::chess::{self, Color, Kind, Move, Outcome, Piece, Placement, Square};
use chrogue_game::content::{RELIC_SLOTS, RELICS_MAX, RelicId};
use chrogue_game::protocol::Code;
use chrogue_game::run::{
    Blocked, CONSCRIPT_ID, Enemy, EnemyPiece, EnemyPieces, Meta, Offer, Run, Unit, generate_enemy, roll_shop,
    trait_pool,
};
use chrogue_game::save::{parse_run, run_json};
use chrogue_game::tuning::Tuning;
use chrogue_game::view::view;
use chrogue_game::{MemoryStorage, Screen, Session};
use serde_json::{Value, json};

type Pieces<'a> = &'a [(Kind, &'a str)];

const RULE_RELICS: [&str; 14] = [
    "vault",
    "crossfire",
    "closeQuarters",
    "pilgrimLeap",
    "queenFlight",
    "gallop",
    "crusade",
    "royalMarch",
    "huntress",
    "shieldWall",
    "echelon",
    "enfilade",
    "divineRight",
    "blessing",
];
const EFFECT_RELICS: [&str; 3] = ["apprenticeship", "coup", "gambit"];

fn sq(name: &str) -> Square {
    let b = name.as_bytes();
    (b[0] - b'a') + 8 * (b[1] - b'1')
}

fn relic(key: &str) -> RelicId {
    RelicId::parse(key).unwrap()
}

fn relics(keys: &[&str]) -> Vec<RelicId> {
    keys.iter().map(|key| relic(key)).collect()
}

/// A run on floor 1 with this army, this enemy, and these relics. The home of a unit is its
/// square, and the id of a unit is its position in the list plus 1.
fn run_of(army: Pieces, enemy: Pieces, keys: &[&str]) -> Run {
    let mut run = Run::new(&Meta::default(), 1, &Tuning::default());
    run.army = army.iter().enumerate().map(|(i, &(kind, s))| Unit { id: i as u16 + 1, kind, home: sq(s) }).collect();
    run.next_id = army.len() as u16 + 1;
    run.enemy = Enemy {
        pieces: EnemyPieces::Placed(enemy.iter().map(|&(kind, s)| EnemyPiece { kind, square: sq(s) }).collect()),
        traits: vec![],
    };
    run.relics = relics(keys);
    run
}

/// The battle of the run. The piece on each `from` square starts on the `to` square.
fn battle_with(run: &Run, starts: &[(&str, &str)]) -> Battle {
    let mut pieces = Battle::placements(run).unwrap();
    for &(from, to) in starts {
        pieces.iter_mut().find(|p| p.square == sq(from)).expect("No piece on the square").square = sq(to);
    }
    Battle::from_placements(run, &pieces).unwrap()
}

/// Plays a legal move. A pawn that promotes becomes a queen.
fn play(battle: &mut Battle, run: &Run, from: &str, to: &str) -> MoveReport {
    let mv = [None, Some(Kind::Queen)].into_iter().find_map(|promo| battle.find_move(sq(from), sq(to), promo).ok());
    battle.play(run, mv.unwrap_or_else(|| panic!("{from}-{to} is not a legal move")))
}

fn reward(battle: &Battle) -> BattleReward {
    battle.result.clone().expect("The battle has no result").reward
}

fn winner(battle: &Battle) -> Option<Color> {
    battle.result.as_ref().expect("The battle has no result").outcome.winner()
}

/// The position with White to move. `mirror` gives the pieces of `white` to Black and the pieces
/// of `black` to White, on the mirrored ranks.
fn position(white: Pieces, black: Pieces, white_relics: &[&str], black_traits: &[&str], mirror: bool) -> Battle {
    let mut run = Run::new(&Meta::default(), 1, &Tuning::default());
    run.relics = relics(white_relics);
    run.enemy.traits = relics(black_traits);
    let side = |pieces: Pieces, color: Color, base: u16| -> Vec<Placement> {
        let place = |(i, &(kind, s)): (usize, &(Kind, &str))| Placement {
            piece: Piece { id: base + i as u16, kind, color, moved: false },
            square: if mirror { sq(s) ^ 56 } else { sq(s) },
        };
        pieces.iter().enumerate().map(place).collect()
    };
    let (first, second) = if mirror { (black, white) } else { (white, black) };
    let pieces = [side(first, Color::White, 1), side(second, Color::Black, 100)].concat();
    Battle::from_placements(&run, &pieces).unwrap()
}

/// True if the piece on `from` can move to `to` when its side has the move.
fn can_move(battle: &Battle, from: Square, to: Square) -> bool {
    chess::moves_from(&battle.state, from).iter().any(|m| m.to == to)
}

/// The relic gives the move to the player. The same relic as a trait gives the mirrored move to
/// the enemy. With no relic, no side has the move. `own` has the pieces of the side that moves.
fn assert_gives_move(key: &str, own: Pieces, other: Pieces, from: &str, to: &str) {
    let mut player = position(own, other, &[key], &[], false);
    assert!(player.find_move(sq(from), sq(to), None).is_ok(), "{key}: the player has no move {from}-{to}");
    let mut plain = position(own, other, &[], &[key], false);
    assert!(plain.find_move(sq(from), sq(to), None).is_err(), "{key}: {from}-{to} is a move with no relic");

    let (from, to) = (sq(from) ^ 56, sq(to) ^ 56);
    assert!(can_move(&position(own, other, &[], &[key], true), from, to), "{key}: the enemy has no move");
    assert!(!can_move(&position(own, other, &[key], &[], true), from, to), "{key}: a move with no trait");
}

const KING: (Kind, &str) = (Kind::King, "e1");
const FOE: (Kind, &str) = (Kind::King, "e8");

// ---- The rule relics ----

#[test]
fn rampart_vault_lets_a_rook_jump_over_a_piece_to_an_empty_square() {
    assert_gives_move("vault", &[KING, (Kind::Rook, "a1"), (Kind::Pawn, "a2")], &[FOE], "a1", "a3");
}

#[test]
fn crossfire_lets_a_rook_capture_on_the_next_diagonal_square() {
    assert_gives_move("crossfire", &[KING, (Kind::Rook, "a1")], &[FOE, (Kind::Pawn, "b2")], "a1", "b2");
}

#[test]
fn close_quarters_lets_a_knight_capture_on_the_next_square_of_its_file() {
    assert_gives_move("closeQuarters", &[KING, (Kind::Knight, "b1")], &[FOE, (Kind::Pawn, "b2")], "b1", "b2");
}

#[test]
fn pilgrims_leap_lets_a_bishop_jump_over_a_piece_on_its_diagonal() {
    assert_gives_move("pilgrimLeap", &[KING, (Kind::Bishop, "c1"), (Kind::Pawn, "d2")], &[FOE], "c1", "e3");
}

#[test]
fn queens_flight_lets_a_queen_jump_as_a_knight_to_an_empty_square() {
    assert_gives_move("queenFlight", &[KING, (Kind::Queen, "d1")], &[FOE], "d1", "c3");
}

#[test]
fn gallop_lets_a_knight_make_a_second_jump_in_the_same_direction() {
    assert_gives_move("gallop", &[KING, (Kind::Knight, "b1")], &[FOE], "b1", "d5");
}

#[test]
fn crusade_lets_a_bishop_move_straight_forward() {
    assert_gives_move("crusade", &[KING, (Kind::Bishop, "c1")], &[FOE], "c1", "c4");
}

#[test]
fn royal_march_lets_a_king_move_two_squares() {
    assert_gives_move("royalMarch", &[KING], &[FOE], "e1", "e3");
}

#[test]
fn huntress_lets_a_queen_capture_as_a_knight() {
    assert_gives_move("huntress", &[KING, (Kind::Queen, "d1")], &[FOE, (Kind::Pawn, "c3")], "d1", "c3");
}

#[test]
fn shield_wall_lets_a_pawn_capture_straight_forward() {
    assert_gives_move("shieldWall", &[KING, (Kind::Pawn, "c2")], &[FOE, (Kind::Knight, "c3")], "c2", "c3");
}

#[test]
fn echelon_lets_a_pawn_move_diagonally_forward_to_an_empty_square() {
    assert_gives_move("echelon", &[KING, (Kind::Pawn, "c2")], &[FOE], "c2", "d3");
}

#[test]
fn enfilade_lets_a_rook_turn_to_the_side_after_two_empty_squares() {
    assert_gives_move("enfilade", &[KING, (Kind::Rook, "a1")], &[FOE], "a1", "c2");
    // A piece on the line stops the turn behind it.
    let mut blocked = position(&[KING, (Kind::Rook, "a1"), (Kind::Pawn, "b1")], &[FOE], &["enfilade"], &[], false);
    assert!(blocked.find_move(sq("a1"), sq("c2"), None).is_err());
}

#[test]
fn divine_right_lets_a_bishop_next_to_its_king_move_as_a_rook() {
    assert_gives_move("divineRight", &[KING, (Kind::Bishop, "d1")], &[FOE], "d1", "d4");
    // A bishop that is two squares from the king moves only as a bishop.
    let mut far = position(&[KING, (Kind::Bishop, "c1")], &[FOE], &["divineRight"], &[], false);
    assert!(far.find_move(sq("c1"), sq("c4"), None).is_err());
}

#[test]
fn blessing_stops_the_capture_of_a_piece_next_to_a_bishop() {
    let own: Pieces = &[KING, (Kind::Bishop, "d3"), (Kind::Pawn, "d4"), (Kind::Pawn, "h4")];
    let other: Pieces = &[FOE, (Kind::Rook, "d8"), (Kind::Rook, "h8")];
    // The enemy rook cannot capture the pawn next to the bishop. It can capture the other pawn.
    let blessed = position(own, other, &["blessing"], &[], false);
    assert!(!can_move(&blessed, sq("d8"), sq("d4")) && can_move(&blessed, sq("h8"), sq("h4")));
    // An officer next to the bishop has the protection too. The bishop has it only next to another bishop.
    let officers: Pieces =
        &[KING, (Kind::Bishop, "c3"), (Kind::Knight, "d4"), (Kind::Bishop, "h3"), (Kind::Bishop, "h4")];
    let rooks: Pieces = &[FOE, (Kind::Rook, "d8"), (Kind::Rook, "c8"), (Kind::Rook, "h8")];
    let blessed = position(officers, rooks, &["blessing"], &[], false);
    assert!(!can_move(&blessed, sq("d8"), sq("d4")) && can_move(&blessed, sq("c8"), sq("c3")));
    assert!(!can_move(&blessed, sq("h8"), sq("h4")));
    assert!(can_move(&position(own, other, &[], &["blessing"], false), sq("d8"), sq("d4")));
    // The same relic as a trait protects the pawns of the enemy.
    let (from, to) = (sq("d8") ^ 56, sq("d4") ^ 56);
    assert!(!can_move(&position(own, other, &[], &["blessing"], true), from, to));
    assert!(can_move(&position(own, other, &["blessing"], &[], true), from, to));
}

#[test]
fn a_boss_can_have_each_rule_relic_and_no_effect_relic() {
    let pool = trait_pool(&Tuning::default());
    assert!(RULE_RELICS.iter().all(|key| pool.contains(&relic(key))));
    assert!(EFFECT_RELICS.iter().all(|key| !pool.contains(&relic(key))));
}

// ---- Apprenticeship ----

/// The base army: Ke1, Ra1, Ng1, and pawns on c2, d2, e2, f2.
const ARMY: Pieces = &[
    KING,
    (Kind::Rook, "a1"),
    (Kind::Knight, "g1"),
    (Kind::Pawn, "c2"),
    (Kind::Pawn, "d2"),
    (Kind::Pawn, "e2"),
    (Kind::Pawn, "f2"),
];
const ALONE: Pieces = &[(Kind::King, "h8")];

fn pawns(run: &Run) -> usize {
    run.army.iter().filter(|unit| unit.kind == Kind::Pawn).count()
}

#[test]
fn apprenticeship_gives_one_pawn_after_a_win_with_a_promotion() {
    let mut run = run_of(ARMY, ALONE, &["apprenticeship"]);
    let mut battle = battle_with(&run, &[("e2", "e7")]);
    let report = play(&mut battle, &run, "e7", "e8");
    assert_eq!(winner(&battle), Some(Color::White));
    assert_eq!(report.relics, relics(&["apprenticeship"]));
    assert_eq!(reward(&battle).recruits, vec![Recruit { id: relic("apprenticeship"), kind: Kind::Pawn }]);
    let screen = Screen::Battle { run: run.clone(), battle: Box::new(battle.clone()), settled: None };
    assert_eq!(view(&screen, &Meta::default())["result"]["recruits"], json!(["p"]));
    battle.settle(&mut run, &Meta::default(), &Tuning::default());
    // The pawn of e2 is a queen, and one more pawn takes the place of it.
    assert_eq!((run.army.len(), pawns(&run)), (8, 4));
}

#[test]
fn apprenticeship_gives_one_pawn_after_two_promotions() {
    let mut run = run_of(ARMY, &[(Kind::King, "h5"), (Kind::Pawn, "a2")], &["apprenticeship"]);
    let mut battle = battle_with(&run, &[("d2", "d7"), ("e2", "e7")]);
    for (from, to) in [("d7", "d8"), ("h5", "h6"), ("e7", "e8"), ("h6", "h7"), ("a1", "a2")] {
        play(&mut battle, &run, from, to);
    }
    assert_eq!(winner(&battle), Some(Color::White));
    assert_eq!(reward(&battle).recruits.len(), 1);
    battle.settle(&mut run, &Meta::default(), &Tuning::default());
    assert_eq!((run.army.len(), pawns(&run)), (8, 3));
}

#[test]
fn apprenticeship_gives_no_pawn_after_a_draw() {
    let mut run = run_of(ARMY, &[(Kind::King, "h8"), (Kind::Pawn, "a2")], &["apprenticeship"]);
    let mut battle = battle_with(&run, &[("e2", "e7")]);
    // The promotion is the last move before the limit of the clock.
    battle.state.clock = 99;
    let report = play(&mut battle, &run, "e7", "e8");
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Clock));
    assert_eq!((report.relics, reward(&battle).recruits), (vec![], vec![]));
    battle.settle(&mut run, &Meta::default(), &Tuning::default());
    assert_eq!(run.army.len(), 7);
}

#[test]
fn apprenticeship_gives_no_pawn_if_the_enemy_captured_the_promoted_unit() {
    // Second Wind returns the unit to the army.
    for (keys, units) in [(&["apprenticeship"][..], 6), (&["secondWind", "apprenticeship"], 7)] {
        let mut run = run_of(ARMY, &[(Kind::King, "d7"), (Kind::Pawn, "a2")], keys);
        let mut battle = battle_with(&run, &[("e2", "e7")]);
        for (from, to) in [("e7", "e8"), ("d7", "e8"), ("a1", "a2")] {
            play(&mut battle, &run, from, to);
        }
        assert_eq!(winner(&battle), Some(Color::White));
        assert_eq!(reward(&battle).recruits, vec![]);
        battle.settle(&mut run, &Meta::default(), &Tuning::default());
        assert_eq!(run.army.len(), units);
    }
}

#[test]
fn apprenticeship_gives_no_pawn_to_a_full_army() {
    let mut run = run_of(ARMY, ALONE, &["apprenticeship"]);
    while run.add_unit(Kind::Pawn).is_some() {}
    let mut battle = battle_with(&run, &[("e2", "e7")]);
    let report = play(&mut battle, &run, "e7", "e8");
    assert_eq!(winner(&battle), Some(Color::White));
    assert_eq!((report.relics, reward(&battle).recruits), (vec![], vec![]));
    battle.settle(&mut run, &Meta::default(), &Tuning::default());
    assert_eq!(run.army.len(), 16);
}

#[test]
fn apprenticeship_does_not_count_the_pawn_of_conscription() {
    let mut run = run_of(ARMY, ALONE, &["conscription", "apprenticeship"]);
    let mut battle = battle_with(&run, &[("a2", "a7")]);
    assert_eq!(chess::piece_at(&battle.state, sq("a7")).map(|p| p.id), Some(CONSCRIPT_ID));
    play(&mut battle, &run, "a7", "a8");
    assert_eq!(winner(&battle), Some(Color::White));
    assert_eq!(reward(&battle).recruits, vec![]);
    battle.settle(&mut run, &Meta::default(), &Tuning::default());
    assert_eq!(run.army.len(), 7);
}

// ---- Coup de Grace ----

/// The rook on a1 gives checkmate on a8. The enemy pieces have a gold value of 5.
const BACK_RANK: Pieces = &[(Kind::King, "h8"), (Kind::Pawn, "g7"), (Kind::Pawn, "h7"), (Kind::Knight, "h3")];

#[test]
fn coup_de_grace_gives_the_value_of_the_enemy_pieces_after_a_checkmate() {
    // Bounty does not change the gold of Coup de Grace.
    let run = run_of(ARMY, BACK_RANK, &["bounty", "coup"]);
    let mut battle = Battle::new(&run).unwrap();
    let report = play(&mut battle, &run, "a1", "a8");
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Checkmate { winner: Color::White }));
    assert_eq!(report.relics, relics(&["coup"]));
    let bonuses = vec![Bonus { id: relic("coup"), gold: 5 }];
    assert_eq!(reward(&battle), BattleReward { captures: 0, clear: 4, bonuses, recruits: vec![] });
}

#[test]
fn coup_de_grace_gives_nothing_after_a_rout() {
    let run = run_of(ARMY, &[FOE, (Kind::Pawn, "a2")], &["coup"]);
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "a1", "a2");
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Rout { winner: Color::White }));
    assert_eq!(reward(&battle).bonuses, vec![]);
}

#[test]
fn coup_de_grace_gives_nothing_after_a_stalemate() {
    // The queen on c7 leaves no move to the king on a8 and to the pawn on a7.
    let run =
        run_of(&[KING, (Kind::Queen, "c1"), (Kind::Pawn, "a2")], &[(Kind::King, "a8"), (Kind::Pawn, "a7")], &["coup"]);
    let mut battle = battle_with(&run, &[("a2", "a6")]);
    play(&mut battle, &run, "c1", "c7");
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Stalemate { winner: Color::White }));
    assert_eq!(reward(&battle).bonuses, vec![]);
}

// ---- Gambit ----

const ROOK_TRADE: (Pieces, Pieces) =
    (&[KING, (Kind::Rook, "d1"), (Kind::Pawn, "h2")], &[FOE, (Kind::Rook, "d8"), (Kind::Pawn, "a7")]);

#[test]
fn gambit_gives_10_gold_for_a_captured_rook() {
    let run = run_of(ROOK_TRADE.0, &[FOE, (Kind::Rook, "d8")], &["gambit"]);
    let mut battle = Battle::new(&run).unwrap();
    for (from, to) in [("h2", "h3"), ("d8", "d1"), ("e1", "d1")] {
        play(&mut battle, &run, from, to);
    }
    assert_eq!(winner(&battle), Some(Color::White));
    let bonuses = vec![Bonus { id: relic("gambit"), gold: 10 }];
    assert_eq!(reward(&battle), BattleReward { captures: 5, clear: 4, bonuses, recruits: vec![] });
}

#[test]
fn gambit_gives_5_gold_for_two_captured_pawns() {
    let army: Pieces = &[KING, (Kind::Rook, "a1"), (Kind::Pawn, "g2"), (Kind::Pawn, "h2")];
    let run = run_of(army, &[FOE, (Kind::Rook, "h8")], &["gambit"]);
    let mut battle = Battle::new(&run).unwrap();
    for (from, to) in [("a1", "b1"), ("h8", "h2"), ("e1", "f1"), ("h2", "g2"), ("f1", "g2")] {
        play(&mut battle, &run, from, to);
    }
    assert_eq!(winner(&battle), Some(Color::White));
    assert_eq!(reward(&battle).bonuses, vec![Bonus { id: relic("gambit"), gold: 5 }]);
}

#[test]
fn gambit_gives_gold_after_a_draw() {
    let run = run_of(ROOK_TRADE.0, ROOK_TRADE.1, &["gambit"]);
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "h2", "h3");
    play(&mut battle, &run, "d8", "d1");
    // The move of the king is the last move before the limit of the clock.
    battle.state.clock = 99;
    let report = play(&mut battle, &run, "e1", "e2");
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Clock));
    assert_eq!(report.relics, relics(&["gambit"]));
    let bonuses = vec![Bonus { id: relic("gambit"), gold: 10 }];
    assert_eq!(reward(&battle), BattleReward { captures: 0, clear: 0, bonuses, recruits: vec![] });
}

#[test]
fn gambit_gives_nothing_after_a_loss() {
    let run = run_of(&[KING, (Kind::Rook, "d1")], ROOK_TRADE.1, &["gambit"]);
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "d1", "d2");
    let report = play(&mut battle, &run, "d8", "d2");
    assert_eq!(winner(&battle), Some(Color::Black));
    assert_eq!((report.relics, reward(&battle)), (vec![], BattleReward::default()));
}

#[test]
fn interest_counts_the_bonuses_of_the_relics_before_it() {
    let interest = |keys: &[&str]| {
        let mut run = run_of(ARMY, BACK_RANK, keys);
        run.gold = 1;
        let mut battle = Battle::new(&run).unwrap();
        play(&mut battle, &run, "a1", "a8");
        reward(&battle).bonuses.into_iter().find(|bonus| bonus.id == relic("interest")).map(|bonus| bonus.gold)
    };
    // With Coup de Grace first: (1 + 4 + 5) / 5. With Interest first: (1 + 4) / 5.
    assert_eq!(interest(&["coup", "interest"]), Some(2));
    assert_eq!(interest(&["interest", "coup"]), Some(1));
}

// ---- The relic slots ----

/// A run with a relic in each of its slots.
fn full_run() -> Run {
    let mut run = Run::new(&Meta::default(), 1, &Tuning::default());
    run.relics = RelicId::all().take(run.slots).collect();
    run
}

fn ask(session: &mut Session, request: Value) -> Value {
    serde_json::from_str(&session.command(&request.to_string())).unwrap()
}

fn send(session: &mut Session, request: Value) -> Value {
    let reply = ask(session, request.clone());
    assert_eq!(reply["ok"], json!(true), "{request} -> {reply}");
    reply
}

/// A debug session in the camp before floor 2. The run has these relics and 50 gold.
fn camp_with(keys: &[&str]) -> Session {
    let mut session = Session::with_debug(Box::new(MemoryStorage::default()), 1, true);
    send(&mut session, json!({ "cmd": "new_run" }));
    for key in keys {
        send(&mut session, json!({ "cmd": "debug_set_relic", "relic": key, "on": true }));
    }
    send(
        &mut session,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 0 }] }),
    );
    send(
        &mut session,
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "p", "square": 8 }] }),
    );
    send(&mut session, json!({ "cmd": "move", "from": 0, "to": 8 }));
    send(&mut session, json!({ "cmd": "continue" }));
    send(&mut session, json!({ "cmd": "debug_set_gold", "gold": 50 }));
    session
}

fn relic_offer(key: &str) -> Value {
    json!([{ "kind": "relic", "id": key }])
}

fn ids(relics: &Value) -> Vec<&str> {
    relics.as_array().unwrap().iter().map(|relic| relic["id"].as_str().unwrap()).collect()
}

#[test]
fn a_new_run_has_4_relic_slots() {
    assert_eq!(Run::new(&Meta::default(), 1, &Tuning::default()).slots, 4);
    assert_eq!((RELIC_SLOTS, Tuning::default().relic_slots), (4, 4));
}

#[test]
fn a_run_with_a_relic_in_each_slot_cannot_take_one_more_relic() {
    let mut run = full_run();
    let (owned, new) = (Offer::Relic(run.relics[0]), Offer::Relic(relic("gambit")));
    assert_eq!(new.blocked(&run).map(Blocked::code), Some("relics_full"));
    // `owned` has priority.
    assert_eq!(owned.blocked(&run), Some(Blocked::Owned));

    run.gold = 100;
    run.draft = Some(vec![new]);
    run.shop = vec![new];
    assert!(!new.take(&mut run));
    assert_eq!(run.take_draft(&Meta::default(), 0).map_err(|fail| fail.code), Err(Code::Blocked));
    assert_eq!(run.buy_offer(&Meta::default(), 0).map_err(|fail| fail.code), Err(Code::Blocked));
    assert_eq!((run.relics.len(), run.gold, run.shop.len()), (RELIC_SLOTS, 100, 1));
}

#[test]
fn a_run_can_take_a_relic_after_it_discards_one() {
    let mut run = full_run();
    let (old, new) = (run.relics[1], Offer::Relic(relic("gambit")));
    run.draft = Some(vec![new]);
    assert_eq!(run.discard_relic(old), Ok(()));
    assert_eq!((run.relics.len(), run.relics.contains(&old), run.gold), (RELIC_SLOTS - 1, false, 0));
    assert_eq!(new.blocked(&run), None);
    assert_eq!(run.take_draft(&Meta::default(), 0), Ok(new));
    assert_eq!(run.relics.last(), Some(&relic("gambit")));
    // The run does not have the relic a second time.
    assert_eq!(run.discard_relic(old).map_err(|fail| fail.code), Err(Code::BadArgs));
}

#[test]
fn the_fifth_relic_offer_is_blocked_until_the_player_discards_a_relic() {
    let mut session = camp_with(&["bounty", "interest", "vault", "gallop"]);
    send(&mut session, json!({ "cmd": "debug_set_shop", "offers": relic_offer("gambit") }));
    let camp = send(&mut session, json!({ "cmd": "debug_set_draft", "offers": relic_offer("coup") }))["view"].clone();
    assert_eq!(camp["relic_slots"], json!(4));
    assert_eq!(camp["reward"]["offers"][0]["blocked"], json!("relics_full"));
    assert_eq!(camp["shop"]["offers"][0]["blocked"], json!("relics_full"));
    for request in [json!({ "cmd": "take_reward", "index": 0 }), json!({ "cmd": "buy", "index": 0 })] {
        let reply = ask(&mut session, request);
        assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!("blocked"), &camp));
    }

    // The discard gives no gold, and the reward stays open.
    let reply = send(&mut session, json!({ "cmd": "discard_relic", "relic": "vault" }));
    let event = json!({ "type": "camp_action", "action": "discard_relic", "gold_before": 50, "gold": 50,
                        "units": [], "relics": [], "discarded": ["vault"], "rolled": false });
    assert_eq!(reply["events"], json!([event]));
    let view = &reply["view"];
    assert_eq!(ids(&view["relics"]), ["bounty", "interest", "gallop"]);
    assert_eq!((&view["reward"]["state"], &view["can_start"]), (&json!("open"), &json!(false)));
    assert_eq!(view["reward"]["offers"][0]["blocked"], Value::Null);
    assert_eq!(view["shop"]["offers"][0]["blocked"], Value::Null);

    let reply = send(&mut session, json!({ "cmd": "take_reward", "index": 0 }));
    assert_eq!((&reply["events"][0]["relics"], &reply["events"][0]["discarded"]), (&json!(["coup"]), &json!([])));
    assert_eq!(ids(&reply["view"]["relics"]), ["bounty", "interest", "gallop", "coup"]);
    assert_eq!(reply["view"]["shop"]["offers"][0]["blocked"], json!("relics_full"));
}

#[test]
fn only_a_discard_has_a_relic_in_the_discarded_list_of_its_event() {
    let mut session = camp_with(&["bounty"]);
    send(&mut session, json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "gold", "amount": 3 }] }));
    send(&mut session, json!({ "cmd": "debug_set_shop", "offers": [{ "kind": "piece", "type": "p" }] }));
    for request in [
        json!({ "cmd": "skip_reward" }),
        json!({ "cmd": "buy", "index": 0 }),
        json!({ "cmd": "reroll" }),
        json!({ "cmd": "discard_relic", "relic": "bounty" }),
    ] {
        let event = send(&mut session, request.clone())["events"][0].clone();
        let discarded = if request["cmd"] == "discard_relic" { json!(["bounty"]) } else { json!([]) };
        assert_eq!((&event["action"], &event["discarded"]), (&request["cmd"], &discarded));
    }
}

#[test]
fn discard_relic_refuses_a_relic_that_the_run_does_not_have() {
    let mut session = camp_with(&["bounty"]);
    let camp = session.view();
    for (request, code) in [
        (json!({ "cmd": "discard_relic", "relic": "noRelic" }), "bad_args"),
        (json!({ "cmd": "discard_relic", "relic": "interest" }), "bad_args"),
        (json!({ "cmd": "discard_relic" }), "bad_args"),
    ] {
        let reply = ask(&mut session, request.clone());
        assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!(code), &camp), "{request}");
    }
    // A battle has no discard.
    send(&mut session, json!({ "cmd": "skip_reward" }));
    let battle = send(&mut session, json!({ "cmd": "start_battle" }))["view"].clone();
    let reply = ask(&mut session, json!({ "cmd": "discard_relic", "relic": "bounty" }));
    assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!("wrong_screen"), &battle));
}

#[test]
fn the_game_offers_a_discarded_relic_again() {
    let shops = |discard: bool| {
        let offers = |seed: u64| {
            let mut run = full_run();
            run.seed = seed;
            let old = run.relics[0];
            if discard {
                run.discard_relic(old).unwrap();
            }
            roll_shop(&run, &Tuning::default()).contains(&Offer::Relic(old))
        };
        (0..100).filter(|&seed| offers(seed)).count()
    };
    assert_eq!(shops(false), 0);
    assert!(shops(true) > 0);
}

#[test]
fn debug_set_relic_gives_a_run_more_relics_than_slots_up_to_the_most_relics() {
    let mut session = Session::with_debug(Box::new(MemoryStorage::default()), 1, true);
    send(&mut session, json!({ "cmd": "new_run" }));
    let keys: Vec<&str> = RelicId::all().map(RelicId::key).collect();
    for key in &keys[..RELICS_MAX] {
        send(&mut session, json!({ "cmd": "debug_set_relic", "relic": key, "on": true }));
    }
    let before = session.view();
    assert_eq!((before["relics"].as_array().unwrap().len(), &before["relic_slots"]), (RELICS_MAX, &json!(4)));
    let reply = ask(&mut session, json!({ "cmd": "debug_set_relic", "relic": keys[RELICS_MAX], "on": true }));
    assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!("blocked"), &before));
    // A relic that the run has stays, and a relic can leave.
    for (key, on) in [(keys[0], true), (keys[0], false), (keys[RELICS_MAX], true)] {
        send(&mut session, json!({ "cmd": "debug_set_relic", "relic": key, "on": on }));
    }
}

#[test]
fn the_views_and_hello_have_the_relic_slots() {
    let mut session = Session::with_debug(Box::new(MemoryStorage::default()), 1, true);
    let content = send(&mut session, json!({ "cmd": "hello" }))["data"]["content"].clone();
    assert_eq!((&content["relic_slots"], &content["relics_max"]), (&json!(RELIC_SLOTS), &json!(RELICS_MAX)));
    let battle = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    assert_eq!((&battle["screen"], &battle["relic_slots"]), (&json!("battle"), &json!(RELIC_SLOTS)));
    let camp = camp_with(&[]).view();
    assert_eq!((&camp["screen"], &camp["relic_slots"]), (&json!("camp"), &json!(RELIC_SLOTS)));
}

#[test]
fn a_saved_run_keeps_its_relic_slots() {
    let mut run = full_run();
    run.slots = 7;
    let saved = run_json(&run);
    assert_eq!(saved["slots"], json!(7));
    assert_eq!(parse_run(&saved), Ok(run.clone()));

    // A run with no slots loads with the default.
    let mut bare = saved.clone();
    bare.as_object_mut().unwrap().remove("slots");
    assert_eq!(parse_run(&bare), Ok(Run { slots: RELIC_SLOTS, ..run }));
    for value in [json!(RELICS_MAX + 1), json!(-1), json!(2.5), json!("4"), Value::Null] {
        let mut bad = saved.clone();
        bad["slots"] = value.clone();
        assert!(parse_run(&bad).is_err(), "slots: {value}");
    }
    for slots in [0, RELICS_MAX] {
        let mut good = saved.clone();
        good["slots"] = json!(slots);
        assert_eq!(parse_run(&good).map(|loaded| loaded.slots), Ok(slots));
    }
}

#[test]
fn a_saved_run_keeps_its_first_relics_up_to_the_most_relics() {
    let mut run = full_run();
    run.relics = RelicId::all().take(RELICS_MAX).collect();
    let mut saved = run_json(&run);
    assert_eq!(parse_run(&saved), Ok(run.clone()));
    saved["relics"] = json!(RelicId::all().map(RelicId::key).collect::<Vec<_>>());
    assert_eq!(parse_run(&saved).map(|loaded| loaded.relics), Ok(run.relics));
}

#[test]
fn a_saved_run_keeps_its_first_traits_up_to_the_limit_and_no_relic_that_is_not_a_trait() {
    let mut saved = run_json(&full_run());
    saved["enemy"]["traits"] = json!(["bounty", "vault", "gambit", "gallop", "crusade"]);
    let traits = parse_run(&saved).map(|loaded| loaded.enemy.traits);
    assert_eq!(traits, Ok(vec![relic("vault"), relic("gallop")]));
}

/// The move of an AI level for White, or None if the search does not end in 20 seconds.
fn ai_move_in_time(mut battle: Battle, level: usize) -> Option<Move> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || sender.send(chess::ai_move(&mut battle.state, level, 1)));
    receiver.recv_timeout(std::time::Duration::from_secs(20)).ok().flatten()
}

/// A position of a battle on floor 8 with Royal Steed against Tactical Retreat and Royal March.
/// The search of level 3 did not end: in the quiescence search, each side answered each check
/// with a quiet move that gave check.
#[test]
fn the_ai_moves_when_each_side_can_answer_a_check_with_a_check() {
    let white: Pieces = &[
        (Kind::King, "f3"),
        (Kind::Rook, "g6"),
        (Kind::Bishop, "a2"),
        (Kind::Knight, "b2"),
        (Kind::Knight, "d2"),
        (Kind::Pawn, "c3"),
        (Kind::Pawn, "h3"),
        (Kind::Pawn, "b4"),
    ];
    let black: Pieces = &[
        (Kind::King, "d7"),
        (Kind::Rook, "e8"),
        (Kind::Bishop, "c2"),
        (Kind::Knight, "b7"),
        (Kind::Pawn, "f4"),
        (Kind::Pawn, "b6"),
        (Kind::Pawn, "d6"),
        (Kind::Pawn, "c7"),
    ];
    let battle = position(white, black, &["kingKnight"], &["backpedal", "royalMarch"], false);
    assert!(ai_move_in_time(battle, 3).is_some(), "the AI did not move in 20 seconds");
}

/// A battle on floor 8 with Royal March against Royal Steed, with level 2 against level 3. The
/// search of move 80 did not end: each check made the search one half move longer, and each
/// move out of check gave check.
#[test]
fn the_ai_moves_when_a_line_of_checks_has_no_end() {
    let tuning = Tuning::default();
    let mut run = Run::new(&Meta::default(), 504_533_058, &tuning);
    run.floor = 8;
    run.enemy = generate_enemy(run.seed, run.floor, &tuning);
    // The traits that the seed gave when the battle was found. The pool of traits has more relics now.
    run.enemy.traits = relics(&["kingKnight", "earlyPromo"]);
    run.relics = relics(&["sidestep", "royalMarch"]);
    let army = [
        (Kind::King, 4),
        (Kind::Rook, 0),
        (Kind::Knight, 6),
        (Kind::Pawn, 10),
        (Kind::Pawn, 11),
        (Kind::Pawn, 12),
        (Kind::Pawn, 13),
        (Kind::Knight, 3),
        (Kind::Rook, 2),
        (Kind::Pawn, 14),
        (Kind::Rook, 5),
        (Kind::Bishop, 1),
        (Kind::Knight, 7),
        (Kind::Bishop, 9),
        (Kind::Pawn, 15),
        (Kind::Bishop, 8),
    ];
    run.army = army.iter().enumerate().map(|(i, &(kind, home))| Unit { id: i as u16 + 1, kind, home }).collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut battle = Battle::new(&run).unwrap();
        while battle.result.is_none() {
            let level = if battle.phase() == BattlePhase::Player { 2 } else { 3 };
            let mv = battle.ai_move(&run, level).unwrap();
            battle.play(&run, mv);
        }
        sender.send(battle.plies)
    });
    let plies = receiver.recv_timeout(std::time::Duration::from_secs(20));
    assert!(plies.is_ok(), "the battle did not end in 20 seconds");
}
