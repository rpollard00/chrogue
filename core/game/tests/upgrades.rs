//! The upgrades that change the start of a run, the shop, and the rewards.

use std::collections::HashSet;
use std::ops::Range;

use chrogue_game::battle::{Battle, Next};
use chrogue_game::chess::{Kind, Outcome, Square};
use chrogue_game::content::{FLOORS, RELICS_MAX, RelicId, UpgradeId};
use chrogue_game::run::{Enemy, EnemyPiece, Meta, Offer, Run, relic_pool, roll_draft};
use chrogue_game::tuning::Tuning;
use chrogue_game::{MemoryStorage, Session};
use serde_json::{Value, json};

/// The seeds of the runs of a test.
fn seeds() -> Range<u64> {
    0..200
}

fn sq(name: &str) -> Square {
    let b = name.as_bytes();
    (b[0] - b'a') + 8 * (b[1] - b'1')
}

fn relic(key: &str) -> RelicId {
    RelicId::parse(key).unwrap()
}

fn meta_with(upgrades: &[(&str, u64)]) -> Meta {
    let mut meta = Meta::default();
    for &(key, level) in upgrades {
        meta.upgrades.insert(UpgradeId::parse(key).unwrap(), level);
    }
    meta
}

fn defaults() -> Tuning {
    Tuning::default()
}

fn debug_session() -> Session {
    Session::with_debug(Box::new(MemoryStorage::default()), 1, true)
}

fn ask(session: &mut Session, request: Value) -> Value {
    serde_json::from_str(&session.command(&request.to_string())).unwrap()
}

fn send(session: &mut Session, request: Value) -> Value {
    let reply = ask(session, request.clone());
    assert_eq!(reply["ok"], json!(true), "{request} -> {reply}");
    reply
}

/// The reward before a floor from 2 to 8 of a run with this seed.
fn draft(seed: u64, meta: &Meta, tuning: &Tuning, change: impl FnOnce(&mut Run)) -> Vec<Offer> {
    let mut run = Run::new(&Meta::default(), seed, tuning);
    run.floor = 2 + seed as usize % 7;
    change(&mut run);
    let draft = roll_draft(&run, meta, tuning);
    assert_eq!(draft.len(), 3);
    assert!(draft.iter().all(|a| draft.iter().filter(|&b| a == b).count() == 1), "{draft:?}");
    draft
}

fn is_relic(offer: &Offer) -> bool {
    matches!(offer, Offer::Relic(_))
}

#[test]
fn squire_gives_a_second_knight_to_a_new_run() {
    let knights = |meta: &Meta| Run::new(meta, 1, &defaults()).army.iter().filter(|u| u.kind == Kind::Knight).count();
    assert_eq!((knights(&Meta::default()), knights(&meta_with(&[("knight", 1)]))), (1, 2));
}

#[test]
fn heirloom_gives_one_relic_that_comes_from_the_seed_of_the_run() {
    let meta = meta_with(&[("heirloom", 1)]);
    let start = |seed: u64, tuning: &Tuning| Run::new(&meta, seed, tuning).relics;
    assert_eq!(Run::new(&Meta::default(), 7, &defaults()).relics, vec![]);
    let relics: HashSet<RelicId> = seeds()
        .map(|seed| {
            let relics = start(seed, &defaults());
            assert_eq!(relics.len(), 1);
            relics[0]
        })
        .collect();
    assert!(relics.len() > 10, "{} different relics", relics.len());
}

#[test]
fn heirloom_does_not_give_a_barred_relic() {
    let meta = meta_with(&[("heirloom", 1)]);
    let pool = relic_pool(&defaults());
    let one = Tuning { barred: pool[1..].to_vec(), ..defaults() };
    let none = Tuning { barred: pool.clone(), ..defaults() };
    for seed in seeds() {
        assert_eq!(Run::new(&meta, seed, &one).relics, vec![pool[0]]);
        assert_eq!(Run::new(&meta, seed, &none).relics, vec![]);
    }
}

#[test]
fn antiquary_takes_gold_from_the_price_of_a_relic_before_the_factor_of_haggler() {
    let relic = Offer::Relic(relic("bounty"));
    let price = |upgrades: &[(&str, u64)]| meta_with(upgrades).price_of(relic);
    assert_eq!((price(&[]), price(&[("antiquary", 1)]), price(&[("antiquary", 2)])), (16, 14, 12));
    // With Haggler at level 2: 16 * 0.8 = 12.8, 14 * 0.8 = 11.2, and 12 * 0.8 = 9.6.
    let haggler = |level: u64| price(&[("haggle", 2), ("antiquary", level)]);
    assert_eq!((price(&[("haggle", 2)]), haggler(1), haggler(2)), (13, 11, 10));
    assert_eq!(price(&[("haggle", 1), ("antiquary", 2)]), 11);
    // The price of a piece stays.
    assert_eq!(meta_with(&[("antiquary", 2)]).price_of(Offer::Piece(Kind::Rook)), 20);
    // A level that is too large gives a price of 1 gold.
    assert_eq!(price(&[("antiquary", u64::MAX)]), 1);
}

#[test]
fn fixer_decreases_the_cost_of_new_shop_items() {
    let cost = |level: u64| meta_with(&[("fixer", level)]).reroll_cost();
    assert_eq!((Meta::default().reroll_cost(), cost(1), cost(2), cost(u64::MAX)), (3, 2, 1, 0));

    // The camp view and `reroll` use the cost of the player.
    let mut session = debug_session();
    send(&mut session, json!({ "cmd": "new_run" }));
    send(
        &mut session,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 0 }] }),
    );
    send(
        &mut session,
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "p", "square": 8 }] }),
    );
    send(&mut session, json!({ "cmd": "move", "from": 0, "to": 8 }));
    let camp = send(&mut session, json!({ "cmd": "continue" }));
    assert_eq!(camp["view"]["shop"]["reroll_cost"], json!(3));
    send(&mut session, json!({ "cmd": "debug_set_upgrade", "upgrade": "fixer", "level": 2 }));
    let shop = send(&mut session, json!({ "cmd": "debug_set_gold", "gold": 1 }))["view"]["shop"].clone();
    assert_eq!((&shop["reroll_cost"], &shop["can_reroll"]), (&json!(1), &json!(true)));
    let reply = send(&mut session, json!({ "cmd": "reroll" }));
    assert_eq!((&reply["events"][0]["gold_before"], &reply["events"][0]["gold"]), (&json!(1), &json!(0)));
    assert_eq!(reply["view"]["shop"]["can_reroll"], json!(false));
    let reply = ask(&mut session, json!({ "cmd": "reroll" }));
    assert_eq!(reply["error"]["code"], json!("not_affordable"));
}

/// The camp after a battle that ends in a draw at the limit of the clock.
fn camp_after_a_draw(meta: &Meta, floor: usize) -> Run {
    let mut run = Run::new(&Meta::default(), 1, &defaults());
    run.floor = floor;
    run.enemy = Enemy {
        pieces: vec![
            EnemyPiece { kind: Kind::King, square: sq("e8") },
            EnemyPiece { kind: Kind::Pawn, square: sq("a7") },
        ],
        traits: vec![],
    };
    let mut battle = Battle::new(&run).unwrap();
    battle.state.clock = 99;
    let mv = battle.find_move(sq("e2"), sq("e3"), None).unwrap();
    battle.play(&run, mv);
    assert_eq!(battle.result.as_ref().map(|r| r.outcome), Some(Outcome::Clock));
    assert_eq!(battle.settle(&mut run, meta, &defaults()), Some(Next::Camp));
    run
}

#[test]
fn envoy_gives_a_reward_after_a_draw() {
    let envoy = meta_with(&[("envoy", 1)]);
    assert_eq!(camp_after_a_draw(&Meta::default(), 1).draft, None);
    let run = camp_after_a_draw(&envoy, 1);
    assert_eq!(run.floor, 2);
    assert_eq!(run.draft, Some(roll_draft(&run, &envoy, &defaults())));
    assert_eq!(run.draft.map(|draft| draft.len()), Some(3));
}

#[test]
fn envoy_gives_no_reward_after_a_draw_on_the_last_floor() {
    let run = camp_after_a_draw(&meta_with(&[("envoy", 1)]), FLOORS.len());
    assert_eq!((run.floor, run.draft), (FLOORS.len(), None));
}

/// The kinds of the units that the army gets when the run takes the reward card.
fn units_of_reward(meta: &Meta, card: Offer, change: impl FnOnce(&mut Run)) -> Vec<Kind> {
    let mut run = Run::new(&Meta::default(), 1, &defaults());
    change(&mut run);
    let before = run.army.len();
    run.draft = Some(vec![card]);
    assert_eq!(run.take_draft(meta, 0), Ok(card));
    run.army[before..].iter().map(|unit| unit.kind).collect()
}

#[test]
fn muster_gives_a_pawn_with_a_unit_of_a_reward() {
    let muster = meta_with(&[("muster", 1)]);
    assert_eq!(units_of_reward(&muster, Offer::Piece(Kind::Knight), |_| {}), vec![Kind::Knight, Kind::Pawn]);
    assert_eq!(units_of_reward(&muster, Offer::Piece(Kind::Pawn), |_| {}), vec![Kind::Pawn, Kind::Pawn]);
    assert_eq!(units_of_reward(&Meta::default(), Offer::Piece(Kind::Knight), |_| {}), vec![Kind::Knight]);
}

#[test]
fn muster_gives_no_pawn_with_a_relic_or_with_gold() {
    let muster = meta_with(&[("muster", 1)]);
    assert_eq!(units_of_reward(&muster, Offer::Relic(relic("bounty")), |_| {}), vec![]);
    assert_eq!(units_of_reward(&muster, Offer::Gold(12), |_| {}), vec![]);
}

#[test]
fn muster_gives_no_pawn_with_a_unit_of_the_shop() {
    let mut run = Run::new(&Meta::default(), 1, &defaults());
    (run.gold, run.shop) = (100, vec![Offer::Piece(Kind::Knight)]);
    assert!(run.buy_offer(&meta_with(&[("muster", 1)]), 0).is_ok());
    assert_eq!(run.army.len(), 8);
}

#[test]
fn muster_gives_no_pawn_if_the_army_is_full_after_the_unit() {
    let one_free = |run: &mut Run| {
        while run.army.len() < 15 {
            run.add_unit(Kind::Pawn);
        }
    };
    assert_eq!(units_of_reward(&meta_with(&[("muster", 1)]), Offer::Piece(Kind::Knight), one_free), vec![Kind::Knight]);
}

#[test]
fn the_event_of_a_reward_with_muster_has_the_unit_and_the_pawn() {
    let mut session = debug_session();
    send(&mut session, json!({ "cmd": "debug_set_upgrade", "upgrade": "muster", "level": 1 }));
    send(&mut session, json!({ "cmd": "new_run" }));
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
    send(&mut session, json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "piece", "type": "n" }] }));
    let reply = send(&mut session, json!({ "cmd": "take_reward", "index": 0 }));
    let units = json!([{ "id": 3, "kind": "n", "home": 3 }, { "id": 4, "kind": "p", "home": 12 }]);
    assert_eq!(reply["events"][0]["units"], units);
    assert_eq!(reply["view"]["army"].as_array().unwrap().len(), 4);
}

#[test]
fn curator_puts_a_relic_card_in_each_reward() {
    let curator = meta_with(&[("curator", 1)]);
    assert!(seeds().all(|seed| is_relic(&draft(seed, &curator, &defaults(), |_| {})[0])));
    assert!(seeds().any(|seed| !draft(seed, &Meta::default(), &defaults(), |_| {}).iter().any(is_relic)));
}

#[test]
fn curator_guarantees_no_relic_card_to_a_run_with_the_most_relics() {
    let curator = meta_with(&[("curator", 1)]);
    let fill = |run: &mut Run| run.relics = RelicId::all().take(RELICS_MAX).collect();
    for seed in seeds() {
        assert_eq!(draft(seed, &curator, &defaults(), fill), draft(seed, &Meta::default(), &defaults(), fill));
    }
    assert!(seeds().any(|seed| !draft(seed, &curator, &defaults(), fill).iter().any(is_relic)));
}

#[test]
fn curator_guarantees_no_relic_card_if_no_relic_remains() {
    let curator = meta_with(&[("curator", 1)]);
    let barred = Tuning { barred: relic_pool(&defaults()), ..defaults() };
    for seed in seeds() {
        let draft = draft(seed, &curator, &barred, |_| {});
        assert!(!draft.iter().any(is_relic), "{draft:?}");
    }
}

#[test]
fn buy_upgrade_works_for_each_new_upgrade() {
    let mut session = debug_session();
    send(&mut session, json!({ "cmd": "open_upgrades" }));
    send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 100 }));
    let levels =
        [("knight", 1), ("heirloom", 1), ("antiquary", 2), ("fixer", 2), ("envoy", 1), ("muster", 1), ("curator", 1)];
    for (key, max) in levels {
        for level in 1..=max {
            let reply = send(&mut session, json!({ "cmd": "buy_upgrade", "upgrade": key }));
            assert_eq!((&reply["events"][0]["id"], &reply["events"][0]["level"]), (&json!(key), &json!(level)));
        }
        let reply = ask(&mut session, json!({ "cmd": "buy_upgrade", "upgrade": key }));
        assert_eq!(reply["error"]["code"], json!("max_level"), "{key}");
    }
    // The costs: 6, 8, 4 + 7, 3 + 5, 5, 5, and 5 crowns.
    let view = send(&mut session, json!({ "cmd": "view" }))["view"].clone();
    assert_eq!(view["meta"]["crowns"], json!(52));
    let slots = view["slots"].as_array().unwrap();
    assert_eq!((slots.len(), slots.iter().filter(|slot| !slot.is_null()).count()), (16, 12));

    // A new run has the second knight of Squire and the relic of Heirloom.
    send(&mut session, json!({ "cmd": "back" }));
    let battle = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    let knights = battle["pieces"].as_array().unwrap().iter().filter(|p| p["color"] == "w" && p["kind"] == "n").count();
    assert_eq!((knights, battle["relics"].as_array().unwrap().len()), (2, 1));
}
