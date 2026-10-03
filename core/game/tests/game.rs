//! The game layer: relics, upgrades, offers, the results of a battle in a run, and saved data.

use std::collections::HashSet;

use chrogue_game::battle::{Battle, BattleResult, BattleReward, Bonus, MoveReport, Next};
use chrogue_game::chess::{self, Color, Kind, Outcome, SideRules, Square};
use chrogue_game::content::{FLOORS, RelicId, UPGRADE_NAME_MAX, UPGRADE_SLOTS, UPGRADES, UpgradeId, gold_value};
use chrogue_game::random::Dice;
use chrogue_game::run::{
    CONSCRIPT_ID, Enemy, EnemyPiece, Meta, Offer, Phase, Run, RunSummary, generate_enemy, relic_pool, roll_draft,
    roll_shop, trait_pool,
};
use chrogue_game::save::{parse_meta, parse_run, run_json};
use chrogue_game::{MemoryStorage, Session};
use serde_json::{Value, json};

fn sq(name: &str) -> Square {
    let b = name.as_bytes();
    (b[0] - b'a') + 8 * (b[1] - b'1')
}

fn relic(key: &str) -> RelicId {
    RelicId::parse(key).unwrap()
}

fn upgrade(key: &str) -> UpgradeId {
    UpgradeId::parse(key).unwrap()
}

fn meta_with(upgrades: &[(&str, u64)], crowns: u64) -> Meta {
    let mut meta = Meta { crowns, ..Meta::default() };
    for &(key, level) in upgrades {
        meta.upgrades.insert(upgrade(key), level);
    }
    meta
}

fn new_run(meta: &Meta) -> Run {
    Run::new(meta, &mut Dice::new(1), &[])
}

/// A run against an enemy that the test selects. The army is: Ke1, Ra1, Ng1, and pawns on c2, d2, e2, f2.
fn run_against(pieces: &[(Kind, &str)], change: impl FnOnce(&mut Run)) -> Run {
    let mut run = new_run(&Meta::default());
    run.enemy =
        Enemy { pieces: pieces.iter().map(|&(kind, s)| EnemyPiece { kind, square: sq(s) }).collect(), traits: vec![] };
    change(&mut run);
    run
}

fn play(battle: &mut Battle, run: &Run, from: &str, to: &str) -> MoveReport {
    let mv = battle.find_move(sq(from), sq(to), None).unwrap_or_else(|_| panic!("{from}-{to} is not a legal move"));
    battle.play(run, mv)
}

const KING: (Kind, &str) = (Kind::King, "e8");

fn clock_draw() -> Option<BattleResult> {
    Some(BattleResult { outcome: Outcome::Clock, reward: BattleReward::default() })
}

#[test]
fn each_floor_makes_an_enemy_army_that_uses_the_budget() {
    let mut dice = Dice::new(7);
    for (i, spec) in FLOORS.iter().enumerate() {
        for _ in 0..50 {
            let Enemy { pieces, traits } = generate_enemy(i + 1, &mut dice, &[]);
            // The piece limits can leave up to 4 points of the budget.
            let total: u32 = pieces.iter().map(|p| gold_value(p.kind)).sum();
            assert!(total <= spec.budget && total + 4 >= spec.budget, "floor {} total {total}", i + 1);
            assert_eq!(pieces.iter().map(|p| p.square).collect::<HashSet<_>>().len(), pieces.len());
            assert_eq!(traits.len(), spec.traits);
            assert!(traits.iter().all(|id| id.def().foe_text.is_some()));
        }
    }
}

#[test]
fn upgrades_change_a_new_run() {
    let run = new_run(&meta_with(&[("pawn", 3), ("bishop", 1), ("gold", 2)], 0));
    assert_eq!(run.army.len(), 11);
    assert_eq!(run.army.iter().map(|u| u.home).collect::<HashSet<_>>().len(), 11);
    assert_eq!(run.army.iter().filter(|u| u.kind == Kind::Bishop).count(), 1);
    assert_eq!(run.gold, 10);
}

#[test]
fn buy_upgrade_takes_crowns_and_stops_at_the_maximum_level() {
    let mut meta = meta_with(&[], 10);
    assert!(meta.buy_upgrade(upgrade("bishop")).is_ok());
    assert_eq!((meta.crowns, meta.level(upgrade("bishop"))), (4, 1));
    assert!(meta.buy_upgrade(upgrade("bishop")).is_err());
    assert!(meta.buy_upgrade(upgrade("pawn")).is_ok());
    assert!(meta.buy_upgrade(upgrade("pawn")).is_err());
    assert_eq!(meta.crowns, 1);
}

#[test]
fn each_upgrade_has_a_slot_on_the_upgrades_screen_and_its_name_fits_the_slot() {
    assert!(UPGRADES.len() <= UPGRADE_SLOTS);
    for def in &UPGRADES {
        assert!(def.name.chars().count() <= UPGRADE_NAME_MAX, "{}", def.name);
    }
}

#[test]
fn scout_lets_the_player_see_the_moves_of_an_enemy_piece() {
    let mut meta = meta_with(&[], 4);
    assert!(!meta.can_scout());
    assert!(meta.buy_upgrade(upgrade("scout")).is_ok());
    assert!(meta.can_scout());
    assert_eq!(meta.crowns, 0);
}

#[test]
fn a_win_gives_gold_keeps_the_army_and_opens_the_camp() {
    let mut run = run_against(&[KING, (Kind::Pawn, "a2")], |_| {});
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "a1", "a2");
    let result = battle.result.clone().unwrap();
    assert_eq!(result.outcome, Outcome::Rout { winner: Color::White });
    assert_eq!(result.reward, BattleReward { captures: 1, clear: 4, bonuses: vec![] });
    assert_eq!(battle.settle(&mut run, &mut Dice::new(1), &[]), Some(Next::Camp));
    assert_eq!((run.floor, run.gold, run.phase), (2, 5, Phase::Camp));
    assert_eq!(run.army.len(), 7);
    assert_eq!(run.draft.as_ref().map(Vec::len), Some(3));
}

#[test]
fn bounty_and_interest_add_gold() {
    let run = run_against(&[KING, (Kind::Rook, "a2")], |r| {
        r.relics = vec![relic("bounty"), relic("interest")];
        r.gold = 20;
    });
    let mut battle = Battle::new(&run).unwrap();
    let report = play(&mut battle, &run, "a1", "a2");
    assert_eq!(report.capture.map(|(_, square, gold)| (square, gold)), Some((sq("a2"), 7.5)));
    assert_eq!(report.relics, vec![relic("bounty"), relic("interest")]);
    // Captures: 5 * 1.5 = 7.5, which rounds to 8. Interest: floor((20 + 8 + 4) / 5) = 6.
    let reward = battle.result.unwrap().reward;
    assert_eq!(reward, BattleReward { captures: 8, clear: 4, bonuses: vec![Bonus { id: relic("interest"), gold: 6 }] });
}

#[test]
fn a_captured_unit_leaves_the_army_and_second_wind_returns_the_first_one() {
    let enemy = [KING, (Kind::Rook, "a8"), (Kind::Rook, "h8")];
    let lose = |run: &Run| {
        let mut battle = Battle::new(run).unwrap();
        play(&mut battle, run, "e2", "e3");
        play(&mut battle, run, "a8", "a1");
        // The rook on a1 gives check, thus the king moves.
        play(&mut battle, run, "e1", "e2");
        play(&mut battle, run, "a1", "g1");
        battle
    };
    let plain = run_against(&enemy, |_| {});
    let battle = lose(&plain);
    assert_eq!((battle.lost, battle.rescued), (vec![2, 3], vec![]));

    let mut run = run_against(&enemy, |r| r.relics = vec![relic("secondWind")]);
    let mut battle = lose(&run);
    assert_eq!((battle.lost.clone(), battle.rescued.clone()), (vec![3], vec![2]));
    battle.result = clock_draw();
    assert_eq!(battle.settle(&mut run, &mut Dice::new(1), &[]), Some(Next::Camp));
    assert_eq!(run.army.iter().map(|u| u.id).collect::<Vec<_>>(), vec![1, 2, 4, 5, 6, 7]);
    assert_eq!(run.draft, None);
}

#[test]
fn conscription_adds_a_pawn_that_does_not_join_the_army() {
    let run = run_against(&[KING, (Kind::Pawn, "a7")], |r| r.relics = vec![relic("conscription")]);
    let battle = Battle::new(&run).unwrap();
    let conscripts: Vec<_> = chess::pieces(&battle.state).into_iter().filter(|(_, p)| p.id == CONSCRIPT_ID).collect();
    assert_eq!(conscripts.len(), 1);
    let (square, pawn) = conscripts[0];
    assert_eq!((square, pawn.kind, pawn.color), (sq("a2"), Kind::Pawn, Color::White));
    assert_eq!(run.army.len(), 7);
}

#[test]
fn a_relic_gives_its_movement_rule_to_the_battle() {
    let mut run = run_against(&[KING, (Kind::Pawn, "a7")], |r| r.relics = vec![relic("kingKnight")]);
    run.enemy.traits = vec![relic("forcedMarch")];
    let battle = Battle::new(&run).unwrap();
    assert_eq!(*chess::side_rules(&battle.state, Color::White), SideRules::standard().king_knight());
    assert_eq!(*chess::side_rules(&battle.state, Color::Black), SideRules::standard().forced_march());
}

#[test]
fn a_promoted_pawn_stays_promoted_after_the_battle() {
    let mut run = run_against(&[KING, (Kind::Pawn, "h7"), (Kind::Pawn, "a7")], |_| {});
    run.army.retain(|u| u.kind != Kind::Pawn || u.home == sq("e2"));
    // The pawn of e2 starts on c7.
    let mut pieces = Battle::placements(&run);
    for p in &mut pieces {
        if p.square == sq("e2") {
            p.square = sq("c7");
        }
    }
    let mut battle = Battle::from_placements(&run, &pieces).unwrap();
    let promote = battle.find_move(sq("c7"), sq("c8"), Some(Kind::Queen)).expect("No promotion move");
    battle.play(&run, promote);
    battle.result = clock_draw();
    battle.settle(&mut run, &mut Dice::new(1), &[]);
    let mut kinds: Vec<char> = run.army.iter().map(|u| chess::kind_letter(u.kind)).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, vec!['k', 'n', 'q', 'r']);
}

#[test]
fn the_last_floor_ends_the_run_with_a_win() {
    let mut run = run_against(&[KING, (Kind::Pawn, "a2")], |r| r.floor = FLOORS.len());
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "a1", "a2");
    assert_eq!(battle.settle(&mut run, &mut Dice::new(1), &[]), Some(Next::Won));
    let mut meta = Meta::default();
    let summary = RunSummary { won: true, cleared: 8, bonus: 5, crowns: 13, new_best: true };
    assert_eq!(meta.finish_run(&run, true), summary);
    assert_eq!((meta.crowns, meta.best, meta.runs), (13, 8, 1));
    assert!(!meta.finish_run(&run, true).new_best);
}

#[test]
fn offers_change_the_run_and_a_blocked_offer_does_nothing() {
    let mut run = new_run(&Meta::default());
    assert!(Offer::Gold(12).take(&mut run));
    assert!(Offer::Relic(relic("bounty")).take(&mut run));
    assert!(!Offer::Relic(relic("bounty")).take(&mut run));
    assert_eq!((run.gold, run.relics.clone()), (12, vec![relic("bounty")]));
    while run.army.len() < 16 {
        assert!(Offer::Piece(Kind::Pawn).take(&mut run));
    }
    assert!(!Offer::Piece(Kind::Knight).take(&mut run));
    assert_eq!(run.army.iter().map(|u| u.home).collect::<HashSet<_>>().len(), 16);
}

#[test]
fn the_draft_and_the_shop_do_not_offer_a_relic_that_the_run_has() {
    let mut run = new_run(&Meta::default());
    let all: Vec<RelicId> = RelicId::all().collect();
    run.relics = all[1..].to_vec();
    let mut dice = Dice::new(3);
    for _ in 0..50 {
        for offer in roll_draft(&run, &mut dice, &[]).into_iter().chain(roll_shop(&run, &mut dice, &[])) {
            if let Offer::Relic(id) = offer {
                assert_eq!(id, all[0]);
            }
        }
    }
}

#[test]
fn the_game_does_not_offer_a_barred_relic_and_a_boss_does_not_get_it_as_a_trait() {
    let pool = trait_pool(&[]);
    let (kept, rest) = (pool[0], pool[1..].to_vec());
    assert!(!relic_pool(&rest).contains(&rest[0]));
    let run = new_run(&Meta::default());
    let mut dice = Dice::new(5);
    for _ in 0..50 {
        for offer in roll_draft(&run, &mut dice, &rest).into_iter().chain(roll_shop(&run, &mut dice, &rest)) {
            if let Offer::Relic(id) = offer {
                assert!(!rest.contains(&id));
            }
        }
        assert_eq!(generate_enemy(4, &mut dice, &rest).traits, vec![kept]);
    }
    assert_eq!(relic_pool(&[]), RelicId::all().collect::<Vec<_>>());
}

#[test]
fn the_shop_takes_gold_and_haggler_decreases_the_price() {
    let mut run = new_run(&Meta::default());
    let offer = Offer::Piece(Kind::Rook);
    run.shop = vec![offer];
    let meta = meta_with(&[("haggle", 2)], 0);
    assert_eq!(Meta::default().price_of(offer), 20);
    assert_eq!(meta.price_of(offer), 16);
    run.gold = 15;
    assert!(run.buy_offer(&meta, 0).is_err());
    run.gold = 16;
    assert!(run.buy_offer(&meta, 0).is_ok());
    assert_eq!((run.gold, run.shop.len()), (0, 0));
    assert_eq!(run.army.len(), 8);
}

#[test]
fn take_draft_takes_one_reward_and_closes_the_draft() {
    let mut run = new_run(&Meta::default());
    run.draft = Some(vec![Offer::Gold(12), Offer::Piece(Kind::Knight)]);
    assert!(run.take_draft(0).is_ok());
    assert_eq!((run.gold, run.draft), (12, None));
}

#[test]
fn saved_data_survives_a_round_trip_and_unknown_ids_are_removed() {
    let mut run = new_run(&Meta::default());
    run.relics = vec![relic("bounty")];
    run.shop = vec![Offer::Relic(relic("interest")), Offer::Piece(Kind::Queen)];
    let mut saved = run_json(&run);
    assert_eq!(parse_run(&saved), Ok(run.clone()));

    saved["relics"].as_array_mut().unwrap().push(json!("removedRelic"));
    let shop = saved["shop"].as_array_mut().unwrap();
    shop.push(json!({ "kind": "relic", "id": "removedRelic" }));
    shop.push(json!({ "kind": "unknownKind" }));
    assert_eq!(parse_run(&saved), Ok(run));
    let mut no_army = saved.clone();
    no_army["army"] = json!([]);
    assert!(parse_run(&no_army).is_err());
    assert!(parse_run(&Value::Null).is_err());

    let meta = parse_meta(&json!({ "crowns": 4, "upgrades": { "pawn": 2, "removedUpgrade": 1 } }));
    assert_eq!(meta, meta_with(&[("pawn", 2)], 4));
}

/// The debug changes run through the protocol.
#[test]
fn the_debug_changes_keep_the_saved_data_valid() {
    let mut session = Session::with_debug(Box::new(MemoryStorage::default()), 1, true);
    let mut send = |request: Value| -> Value {
        let reply: Value = serde_json::from_str(&session.command(&request.to_string())).unwrap();
        reply
    };
    let level = |reply: &Value| reply["view"]["slots"][0]["level"].clone();
    send(json!({ "cmd": "open_upgrades" }));
    assert_eq!(level(&send(json!({ "cmd": "debug_set_upgrade", "upgrade": "pawn", "level": 9 }))), json!(3));
    let reply = send(json!({ "cmd": "debug_set_upgrade", "upgrade": "pawn", "level": -1 }));
    assert_eq!(level(&reply), json!(0));
    assert_eq!(reply["view"]["meta"]["crowns"], json!(0));

    send(json!({ "cmd": "back" }));
    send(json!({ "cmd": "new_run" }));
    let reply = send(json!({ "cmd": "debug_set_floor", "floor": 8 }));
    assert_eq!(reply["view"]["traits"].as_array().unwrap().len(), FLOORS[7].traits);
    let units = |reply: &Value| -> Vec<(u64, String)> {
        let pieces = reply["view"]["pieces"].as_array().unwrap();
        let white = pieces.iter().filter(|p| p["color"] == "w");
        white.map(|p| (p["id"].as_u64().unwrap(), p["kind"].as_str().unwrap().to_string())).collect()
    };
    let army = units(&reply);
    let id_of = |kind: &str| army.iter().find(|(_, k)| k == kind).unwrap().0;
    let king = send(json!({ "cmd": "debug_remove_unit", "unit": id_of("k") }));
    assert_eq!(king["ok"], json!(false));
    let reply = send(json!({ "cmd": "debug_remove_unit", "unit": id_of("r") }));
    assert!(!units(&reply).iter().any(|(_, k)| k == "r"));
    send(json!({ "cmd": "debug_set_relic", "relic": "bounty", "on": true }));
    let reply = send(json!({ "cmd": "debug_set_relic", "relic": "bounty", "on": true }));
    assert_eq!(reply["view"]["relics"].as_array().unwrap().len(), 1);
    let reply = send(json!({ "cmd": "debug_set_relic", "relic": "bounty", "on": false }));
    assert_eq!(reply["view"]["relics"], json!([]));
    let saved = send(json!({ "cmd": "view", "run": true }))["data"]["run"].clone();
    let run = parse_run(&saved).unwrap();
    assert_eq!(run_json(&run), saved);
}

/// The engine keeps the last piece of a square and takes a side with no king, thus the debug
/// commands refuse such boards and change nothing.
#[test]
fn debug_boards_with_two_pieces_on_a_square_or_no_king_are_refused() {
    let mut session = Session::with_debug(Box::new(MemoryStorage::default()), 1, true);
    let mut send = |request: Value| -> Value { serde_json::from_str(&session.command(&request.to_string())).unwrap() };
    let start = send(json!({ "cmd": "new_run" }))["view"].clone();
    let refused = [
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "q", "square": 60 }] }),
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "q", "square": 4 }] }),
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "q", "square": 59 }] }),
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "k", "square": 62 }] }),
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "q", "home": 3 }] }),
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 4 }] }),
    ];
    for request in &refused {
        let reply = send(request.clone());
        assert_eq!(reply["error"]["code"], json!("bad_args"), "{request} -> {reply}");
        assert_eq!(reply["view"], start, "{request}");
    }
    // An enemy pawn on the empty square 8, then a unit that the debug menu adds there: refused.
    send(
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "p", "home": 0 }, { "kind": "p", "home": 1 }, { "kind": "p", "home": 2 }, { "kind": "p", "home": 3 }] }),
    );
    let reply = send(
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "p", "square": 5 }] }),
    );
    assert_eq!(reply["ok"], json!(true), "{reply}");
    let before = reply["view"].clone();
    let reply = send(json!({ "cmd": "debug_add_unit", "kind": "n" }));
    assert_eq!(reply["error"]["code"], json!("bad_args"), "{reply}");
    assert!(reply["error"]["message"].as_str().unwrap().contains("square 5"), "{reply}");
    assert_eq!(reply["view"], before);
    // The pawn of Conscription goes around an enemy piece on rank 2.
    send(json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "p", "square": 8 }] }));
    let reply = send(json!({ "cmd": "debug_set_relic", "relic": "conscription", "on": true }));
    let squares: Vec<u64> =
        reply["view"]["pieces"].as_array().unwrap().iter().map(|p| p["square"].as_u64().unwrap()).collect();
    assert!(squares.contains(&8) && squares.contains(&9), "{squares:?}");
}
