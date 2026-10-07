//! The relic unlocks: the starter relics, the relics that crowns buy, and the relics of the feats.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;

use chrogue_game::battle::Battle;
use chrogue_game::chess::{Kind, Square};
use chrogue_game::content::{FLOORS, RelicId, Unlock, UpgradeId, board_order};
use chrogue_game::feat::{BattleFacts, Feat};
use chrogue_game::protocol::Code;
use chrogue_game::run::{
    Enemy, EnemyPiece, EnemyPieces, Meta, Offer, Run, Unit, generate_enemy, offer_pool, relic_pool, roll_draft,
    roll_shop,
};
use chrogue_game::save::{meta_json, parse_meta};
use chrogue_game::tuning::Tuning;
use chrogue_game::{Doc, MemoryStorage, Session, Storage};
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

fn defaults() -> Tuning {
    Tuning::default()
}

fn is_starter(id: RelicId) -> bool {
    id.def().unlock == Unlock::Start
}

/// The meta with the Heirloom upgrade.
fn with_heirloom(mut meta: Meta) -> Meta {
    meta.upgrades.insert(UpgradeId::parse("heirloom").unwrap(), 1);
    meta
}

/// The relics that a run with this seed gets: the relic of Heirloom, the relics of the reward,
/// and the relics of the shop before a floor from 2 to 8.
fn offered(seed: u64, meta: &Meta) -> [Vec<RelicId>; 3] {
    let relics = |offers: Vec<Offer>| -> Vec<RelicId> {
        offers.into_iter().filter_map(|offer| if let Offer::Relic(id) = offer { Some(id) } else { None }).collect()
    };
    let mut run = Run::new(meta, seed, &defaults());
    let heirloom = std::mem::take(&mut run.relics);
    run.floor = 2 + seed as usize % 7;
    [heirloom, relics(roll_draft(&run, meta, &defaults())), relics(roll_shop(&run, meta, &defaults()))]
}

/// A run with these units and this enemy on set squares.
fn run_with(army: &[(Kind, &str)], enemy: &[(Kind, &str)]) -> Run {
    let mut run = Run::new(&Meta::default(), 1, &defaults());
    run.army =
        army.iter().enumerate().map(|(i, &(kind, home))| Unit { id: i as u16 + 1, kind, home: sq(home) }).collect();
    run.enemy = Enemy {
        pieces: EnemyPieces::Placed(enemy.iter().map(|&(kind, s)| EnemyPiece { kind, square: sq(s) }).collect()),
        traits: vec![],
    };
    run
}

fn play(battle: &mut Battle, run: &Run, from: &str, to: &str) {
    let mv = battle.find_move(sq(from), sq(to), None).unwrap_or_else(|_| panic!("{from}-{to} is not a legal move"));
    battle.play(run, mv);
}

/// The facts of a win before the last floor with no checkmate, no lost piece, and no promotion.
const PLAIN_WIN: BattleFacts =
    BattleFacts { last_floor: false, won: true, mate_by: None, pieces_lost: 0, promotions: 0 };

#[test]
fn a_new_save_has_only_the_starter_relics() {
    let meta = Meta::default();
    let unlocked: Vec<&str> = RelicId::all().filter(|&id| meta.has_relic(id)).map(RelicId::key).collect();
    let starters =
        ["forcedMarch", "bounty", "interest", "crossfire", "closeQuarters", "crusade", "royalMarch", "huntress"];
    assert_eq!(unlocked, starters);
    assert!(RelicId::all().all(|id| Meta::complete().has_relic(id)));
    assert_eq!(offer_pool(&Meta::complete(), &defaults()), relic_pool(&defaults()));
}

#[test]
fn a_new_save_gets_only_starter_relics_in_the_reward_in_the_shop_and_from_heirloom() {
    let meta = with_heirloom(Meta::default());
    for seed in seeds() {
        let [heirloom, draft, shop] = offered(seed, &meta);
        assert_eq!((heirloom.len(), shop.len()), (1, 2), "seed {seed}");
        for id in heirloom.into_iter().chain(draft).chain(shop) {
            assert!(is_starter(id), "seed {seed}: {}", id.key());
        }
    }
}

#[test]
fn a_save_with_each_relic_unlocked_gets_relics_that_are_not_starters() {
    let meta = with_heirloom(Meta::complete());
    for source in 0..3 {
        let other = seeds().filter(|&seed| offered(seed, &meta)[source].iter().any(|&id| !is_starter(id))).count();
        assert!(other > 0, "source {source}");
    }
}

#[test]
fn a_barred_relic_is_not_in_the_offers_of_a_save_that_unlocked_it() {
    let tuning = Tuning { barred: vec![relic("gallop")], ..defaults() };
    assert!(offer_pool(&Meta::complete(), &defaults()).contains(&relic("gallop")));
    assert!(!offer_pool(&Meta::complete(), &tuning).contains(&relic("gallop")));
}

#[test]
fn a_boss_can_have_a_relic_that_the_player_did_not_unlock_as_a_trait() {
    let meta = Meta::default();
    let locked =
        seeds().filter(|&seed| generate_enemy(seed, 4, &defaults()).traits.iter().any(|&id| !meta.has_relic(id)));
    assert!(locked.count() > 0);
}

#[test]
fn buy_relic_takes_crowns_and_refuses_a_relic_that_crowns_cannot_buy() {
    let mut meta = Meta { crowns: 10, ..Meta::default() };
    let code = |result: Result<(), chrogue_game::protocol::Fail>| result.map_err(|fail| fail.code);
    assert_eq!(code(meta.buy_relic(relic("gallop"))), Ok(()));
    assert_eq!((meta.crowns, meta.has_relic(relic("gallop"))), (2, true));
    let before = meta.clone();
    // Unlocked already, a starter, a relic of a feat, and a relic that costs 3 crowns.
    assert_eq!(code(meta.buy_relic(relic("gallop"))), Err(Code::Blocked));
    assert_eq!(code(meta.buy_relic(relic("bounty"))), Err(Code::Blocked));
    assert_eq!(code(meta.buy_relic(relic("longLeap"))), Err(Code::Blocked));
    assert_eq!(code(meta.buy_relic(relic("vault"))), Err(Code::NotAffordable));
    assert_eq!(meta, before);
}

#[test]
fn set_relic_unlocks_and_locks_a_relic_at_no_cost_and_keeps_a_starter() {
    let mut meta = Meta::default();
    for key in ["gallop", "longLeap", "bounty"] {
        meta.set_relic(relic(key), true);
        assert!(meta.has_relic(relic(key)), "{key}");
    }
    assert_eq!(meta.relics, BTreeSet::from([relic("gallop")]));
    assert_eq!(meta.feats, BTreeSet::from([Feat::KnightMate]));
    for key in ["gallop", "longLeap", "bounty"] {
        meta.set_relic(relic(key), false);
    }
    assert_eq!(meta, Meta::default());
    assert!(meta.has_relic(relic("bounty")));
}

#[test]
fn each_feat_has_a_battle_that_does_it_and_a_battle_that_does_not() {
    let cases = [
        (Feat::KnightMate, BattleFacts { mate_by: Some(Kind::Knight), ..PLAIN_WIN }, PLAIN_WIN),
        (
            Feat::KnightMate,
            BattleFacts { mate_by: Some(Kind::Knight), ..PLAIN_WIN },
            BattleFacts { mate_by: Some(Kind::Rook), ..PLAIN_WIN },
        ),
        (
            Feat::RookMate,
            BattleFacts { mate_by: Some(Kind::Rook), ..PLAIN_WIN },
            BattleFacts { mate_by: Some(Kind::Queen), ..PLAIN_WIN },
        ),
        (Feat::CleanWin, PLAIN_WIN, BattleFacts { pieces_lost: 1, ..PLAIN_WIN }),
        (Feat::CleanWin, PLAIN_WIN, BattleFacts { won: false, ..PLAIN_WIN }),
        (Feat::TwoPromotions, BattleFacts { promotions: 2, ..PLAIN_WIN }, BattleFacts { promotions: 1, ..PLAIN_WIN }),
        (
            Feat::TwoPromotions,
            BattleFacts { promotions: 3, ..PLAIN_WIN },
            BattleFacts { promotions: 2, won: false, ..PLAIN_WIN },
        ),
        (Feat::CostlyWin, BattleFacts { pieces_lost: 3, ..PLAIN_WIN }, BattleFacts { pieces_lost: 2, ..PLAIN_WIN }),
        (
            Feat::CostlyWin,
            BattleFacts { pieces_lost: 4, ..PLAIN_WIN },
            BattleFacts { pieces_lost: 4, won: false, ..PLAIN_WIN },
        ),
        (Feat::WinRun, BattleFacts { last_floor: true, ..PLAIN_WIN }, PLAIN_WIN),
        (
            Feat::WinRun,
            BattleFacts { last_floor: true, ..PLAIN_WIN },
            BattleFacts { last_floor: true, won: false, ..PLAIN_WIN },
        ),
    ];
    for (feat, does, does_not) in cases {
        assert!(feat.met(&does), "{}: {does:?}", feat.name());
        assert!(!feat.met(&does_not), "{}: {does_not:?}", feat.name());
    }
}

#[test]
fn earn_records_the_feats_of_a_battle_and_gives_each_relic_one_time() {
    let mut meta = Meta::default();
    // A win of the run with a checkmate by a knight and two promotions.
    let facts = BattleFacts { last_floor: true, won: true, mate_by: Some(Kind::Knight), pieces_lost: 0, promotions: 2 };
    let earned = meta.earn(&facts);
    assert_eq!(earned, [relic("longLeap"), relic("blessing"), relic("apprenticeship"), relic("kingKnight")]);
    let order = board_order();
    assert!(earned.is_sorted_by_key(|id| order.iter().position(|other| other == id)));
    assert_eq!(meta.feats, BTreeSet::from([Feat::KnightMate, Feat::CleanWin, Feat::TwoPromotions, Feat::WinRun]));
    assert!(earned.iter().all(|&id| offer_pool(&meta, &defaults()).contains(&id)));
    assert_eq!(meta.earn(&facts), []);
    // A relic that a debug command unlocked comes with no second unlock.
    let mut meta = Meta::default();
    meta.set_relic(relic("gambit"), true);
    assert_eq!(meta.earn(&BattleFacts { pieces_lost: 3, ..PLAIN_WIN }), []);
    assert_eq!(meta.earn(&BattleFacts { won: false, ..PLAIN_WIN }), []);
}

#[test]
fn a_battle_gives_its_facts_when_it_has_a_result() {
    // The enemy king on a5 has its pieces on each square around it. The knight gives checkmate from c4.
    let cage = [
        (Kind::King, "a5"),
        (Kind::Pawn, "a4"),
        (Kind::Pawn, "b4"),
        (Kind::Rook, "b5"),
        (Kind::Pawn, "b6"),
        (Kind::Pawn, "a6"),
    ];
    let mut run = run_with(&[(Kind::King, "e1"), (Kind::Knight, "b2")], &cage);
    run.floor = FLOORS.len();
    let mut battle = Battle::new(&run).unwrap();
    assert_eq!(battle.facts(&run), None);
    play(&mut battle, &run, "b2", "c4");
    let facts = BattleFacts { last_floor: true, won: true, mate_by: Some(Kind::Knight), pieces_lost: 0, promotions: 0 };
    assert_eq!(battle.facts(&run), Some(facts));

    // The rook captures the last enemy piece: a win with no checkmate.
    let run = run_with(&[(Kind::King, "e1"), (Kind::Rook, "a1")], &[(Kind::King, "e8"), (Kind::Pawn, "a2")]);
    let mut battle = Battle::new(&run).unwrap();
    play(&mut battle, &run, "a1", "a2");
    assert_eq!(battle.facts(&run), Some(PLAIN_WIN));
}

#[test]
fn a_battle_counts_the_promotions_of_the_player_and_the_pieces_that_the_enemy_captured() {
    let enemy = [(Kind::King, "h5"), (Kind::Rook, "c8"), (Kind::Pawn, "g2")];
    let run = run_with(&[(Kind::King, "e1"), (Kind::Pawn, "a2"), (Kind::Knight, "h1")], &enemy);
    // The pawn of a2 starts on a7.
    let mut pieces = Battle::placements(&run).unwrap();
    pieces.iter_mut().find(|p| p.square == sq("a2")).unwrap().square = sq("a7");
    let mut battle = Battle::from_placements(&run, &pieces).unwrap();
    let promote = battle.find_move(sq("a7"), sq("a8"), Some(Kind::Rook)).unwrap();
    battle.play(&run, promote);
    // The enemy pawn captures the knight and promotes: this is not a promotion of the player.
    let capture = battle.find_move(sq("g2"), sq("h1"), Some(Kind::Queen)).unwrap();
    battle.play(&run, capture);
    assert_eq!((battle.promotions, battle.taken[1].len(), battle.result.is_none()), (1, 1, true));
}

/// The lost pieces of the feats are the lost units of the battle (`Battle::lost`), not each
/// capture of the enemy.
#[test]
fn a_unit_that_second_wind_returns_and_the_pawn_of_conscription_are_not_lost_pieces() {
    // The enemy rook captures the pawns of a2, b2, and c2 in this order, and then the king
    // captures the rook. The pawn of Conscription is on a2.
    let facts = |king: &str, pawns: &[&str], relics: &[&str], captures: usize| {
        let mut army = vec![(Kind::King, king), (Kind::Pawn, "h2")];
        army.extend(pawns.iter().map(|&home| (Kind::Pawn, home)));
        let mut run = run_with(&army, &[(Kind::King, "h8"), (Kind::Rook, "a8")]);
        run.relics = relics.iter().map(|key| relic(key)).collect();
        let mut battle = Battle::new(&run).unwrap();
        let path = ["a8", "a2", "b2", "c2"];
        for i in 0..captures {
            play(&mut battle, &run, &format!("h{}", i + 2), &format!("h{}", i + 3));
            play(&mut battle, &run, path[i], path[i + 1]);
        }
        play(&mut battle, &run, king, path[captures]);
        assert_eq!(battle.taken[1].len(), captures);
        let facts = battle.facts(&run).unwrap();
        assert_eq!(facts, BattleFacts { pieces_lost: battle.lost.len() as u32, ..PLAIN_WIN });
        (facts.pieces_lost, Feat::CleanWin.met(&facts), Feat::CostlyWin.met(&facts))
    };
    assert_eq!(facts("b1", &["a2"], &[], 1), (1, false, false));
    assert_eq!(facts("b1", &["a2"], &["secondWind"], 1), (0, true, false));
    assert_eq!(facts("b1", &[], &["conscription"], 1), (0, true, false));
    assert_eq!(facts("d1", &["a2", "b2", "c2"], &[], 3), (3, false, true));
    assert_eq!(facts("d1", &["a2", "b2", "c2"], &["secondWind"], 3), (2, false, false));
    assert_eq!(facts("d1", &["b2", "c2"], &["conscription"], 3), (2, false, false));
}

/// The piece of a checkmate is the piece on the target square: a pawn that promotes to a knight
/// gives checkmate as a knight.
#[test]
fn a_pawn_that_promotes_to_a_knight_and_gives_checkmate_does_the_feat_of_the_knight() {
    let cage = [
        (Kind::King, "h7"),
        (Kind::Bishop, "g8"),
        (Kind::Rook, "h8"),
        (Kind::Pawn, "g7"),
        (Kind::Pawn, "g6"),
        (Kind::Pawn, "h6"),
    ];
    let run = run_with(&[(Kind::King, "e1"), (Kind::Pawn, "a2")], &cage);
    let mut pieces = Battle::placements(&run).unwrap();
    pieces.iter_mut().find(|p| p.square == sq("a2")).unwrap().square = sq("f7");
    let mut battle = Battle::from_placements(&run, &pieces).unwrap();
    let promote = battle.find_move(sq("f7"), sq("f8"), Some(Kind::Knight)).unwrap();
    battle.play(&run, promote);
    let facts = battle.facts(&run).unwrap();
    assert_eq!(facts, BattleFacts { mate_by: Some(Kind::Knight), promotions: 1, ..PLAIN_WIN });
    assert_eq!(Meta::default().earn(&facts), [relic("longLeap"), relic("blessing")]);
}

#[test]
fn a_meta_with_no_unlock_fields_keeps_its_data_and_has_the_starter_relics() {
    let meta = parse_meta(&json!({ "crowns": 9, "best": 3, "runs": 4, "upgrades": { "pawn": 2 } }));
    let mut expected = Meta { crowns: 9, best: 3, runs: 4, ..Meta::default() };
    expected.upgrades.insert(UpgradeId::parse("pawn").unwrap(), 2);
    assert_eq!(meta, expected);
    assert!(RelicId::all().all(|id| meta.has_relic(id) == is_starter(id)));
}

#[test]
fn saved_unlocks_survive_a_round_trip_and_unknown_and_starter_ids_are_dropped() {
    let mut meta = Meta { crowns: 7, ..Meta::default() };
    meta.relics.extend([relic("gallop"), relic("vault")]);
    meta.feats.extend([Feat::RookMate, Feat::WinRun]);
    let saved = meta_json(&meta);
    assert_eq!(saved["relics"], json!(["vault", "gallop"]));
    assert_eq!(saved["feats"], json!(["rookMate", "winRun"]));
    assert_eq!(parse_meta(&saved), meta);
    assert_eq!(parse_meta(&meta_json(&Meta::complete())), Meta::complete());

    let loaded = parse_meta(&json!({
        "crowns": 7,
        "relics": ["gallop", "bounty", "removedRelic", "gallop", 5, "vault"],
        "feats": ["winRun", "removedFeat", 3, "rookMate", "winRun"],
    }));
    assert_eq!(loaded, meta);
    for bad in [json!("gallop"), json!({ "gallop": true }), json!(null)] {
        let loaded = parse_meta(&json!({ "crowns": 7, "relics": bad.clone(), "feats": bad }));
        assert_eq!(loaded, Meta { crowns: 7, ..Meta::default() });
    }
}

// ---- The protocol ----

/// A storage that two sessions share, as two starts of the program share a directory.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<MemoryStorage>>);

impl Storage for Shared {
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String> {
        self.0.borrow_mut().load(doc)
    }
    fn save(&mut self, doc: Doc, text: &str) -> Result<(), String> {
        self.0.borrow_mut().save(doc, text)
    }
    fn remove(&mut self, doc: Doc) -> Result<(), String> {
        self.0.borrow_mut().remove(doc)
    }
    fn set_aside(&mut self, doc: Doc) -> Result<Option<String>, String> {
        self.0.borrow_mut().set_aside(doc)
    }
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

/// The slots of the relic board in a new save: the first relic that crowns buy (Tactical Retreat,
/// 3 crowns), the most expensive one (Gallop, 8 crowns), the first relic of a feat (Long Leap),
/// and the first slot with no relic.
const CROWN_SLOT: usize = 8;
const GALLOP_SLOT: usize = 20;
const FEAT_SLOT: usize = 21;
const EMPTY_SLOT: usize = 27;

/// No relic: the list of `unlocked_by` for a response with no `relic_unlocked` event.
const NONE: [&str; 0] = [];

/// The ids of the relics that the events of a response unlocked.
fn unlocked_by(reply: &Value) -> Vec<&str> {
    let events = reply["events"].as_array().unwrap().iter();
    events.filter(|e| e["type"] == "relic_unlocked").map(|e| e["id"].as_str().unwrap()).collect()
}

/// Starts a new run on a floor with these units (kind, home) and these enemy pieces (kind, square).
fn battle(session: &mut Session, floor: usize, units: &[(&str, &str)], enemy: &[(&str, &str)]) {
    if !matches!(session.view()["screen"].as_str(), Some("title" | "over")) {
        send(session, json!({ "cmd": "to_title" }));
    }
    send(session, json!({ "cmd": "new_run" }));
    send(session, json!({ "cmd": "debug_set_floor", "floor": floor }));
    let units: Vec<Value> = units.iter().map(|&(kind, home)| json!({ "kind": kind, "home": sq(home) })).collect();
    send(session, json!({ "cmd": "debug_set_army", "units": units }));
    let pieces: Vec<Value> = enemy.iter().map(|&(kind, s)| json!({ "kind": kind, "square": sq(s) })).collect();
    send(session, json!({ "cmd": "debug_set_enemy", "pieces": pieces, "traits": [] }));
}

fn player(session: &mut Session, from: &str, to: &str) -> Value {
    send(session, json!({ "cmd": "move", "from": sq(from), "to": sq(to) }))
}

fn enemy(session: &mut Session, from: &str, to: &str) -> Value {
    send(session, json!({ "cmd": "debug_enemy_move", "from": sq(from), "to": sq(to) }))
}

/// The reply of the move that wins the battle.
fn won(reply: Value) -> Value {
    assert_eq!(reply["view"]["result"]["winner"], json!("w"), "{}", reply["view"]["result"]);
    reply
}

/// A win on a floor: the rook captures the last enemy piece. The player loses no piece.
fn rook_win(session: &mut Session, floor: usize) -> Value {
    battle(session, floor, &[("k", "e1"), ("r", "a1")], &[("k", "e8"), ("p", "a2")]);
    won(player(session, "a1", "a2"))
}

/// A win in which the enemy rook captures `pawns` pawns of the player (1 to 3), and then the
/// king captures the rook. The run has these relics.
fn win_with_losses(session: &mut Session, pawns: usize, relics: &[&str]) -> Value {
    let units = [("k", "d1"), ("p", "h2"), ("p", "c2"), ("p", "b2"), ("p", "a2")];
    battle(session, 1, &units[..2 + pawns], &[("k", "h8"), ("r", ["c8", "b8", "a8"][pawns - 1])]);
    for relic in relics {
        send(session, json!({ "cmd": "debug_set_relic", "relic": relic, "on": true }));
    }
    let files = ["a", "b", "c"];
    let path = &files[3 - pawns..];
    let mut rook = format!("{}8", path[0]);
    for (i, file) in path.iter().enumerate() {
        player(session, &format!("h{}", i + 2), &format!("h{}", i + 3));
        let target = format!("{file}2");
        enemy(session, &rook, &target);
        rook = target;
    }
    let reply = won(player(session, "d1", "c2"));
    let view = &reply["view"];
    assert_eq!(view["lost"].as_array().unwrap().len() + view["rescued"].as_array().unwrap().len(), pawns);
    reply
}

/// A win in which `pawns` pawns of the player (1 or 2) promote to a rook, and then a rook
/// captures the last enemy piece. The enemy king goes between h5 and h4.
fn win_with_promotions(session: &mut Session, pawns: usize) -> Value {
    let units = [("k", "e1"), ("p", "a2"), ("p", "b2")];
    battle(session, 1, &units[..1 + pawns], &[("k", "h5"), ("n", "h8")]);
    let mut king = ["h5", "h4"].into_iter().cycle();
    let mut reply = |session: &mut Session| {
        let (from, to) = (king.next().unwrap(), king.clone().next().unwrap());
        enemy(session, from, to);
    };
    for file in &["a", "b"][..pawns] {
        for (from, to) in [(2, 4), (4, 5), (5, 6), (6, 7)] {
            player(session, &format!("{file}{from}"), &format!("{file}{to}"));
            reply(session);
        }
        let promote =
            json!({ "cmd": "move", "from": sq(&format!("{file}7")), "to": sq(&format!("{file}8")), "promo": "r" });
        send(session, promote);
        reply(session);
    }
    won(player(session, ["a8", "b8"][pawns - 1], "h8"))
}

#[test]
fn the_relics_screen_opens_from_the_title_and_from_the_end_screen_and_keeps_the_saved_run() {
    let mut session = debug_session();
    let reply = send(&mut session, json!({ "cmd": "open_relics" }));
    assert_eq!(reply["events"], json!([{ "type": "screen", "name": "relics" }]));
    let view = &reply["view"];
    assert_eq!(view["meta"], json!({ "crowns": 0, "best": 0, "runs": 0 }));
    assert_eq!((&view["screen"], &view["unlocked"], &view["total"]), (&json!("relics"), &json!(8), &json!(27)));
    let slots = view["slots"].as_array().unwrap();
    assert_eq!((slots.len(), slots.iter().filter(|slot| !slot.is_null()).count()), (36, 27));
    assert!(slots[..8].iter().all(|slot| slot["unlocked"] == true) && slots[EMPTY_SLOT..].iter().all(Value::is_null));
    assert_eq!(ask(&mut session, json!({ "cmd": "open_relics" }))["error"]["code"], json!("wrong_screen"));
    let reply = send(&mut session, json!({ "cmd": "back" }));
    assert_eq!(reply["events"], json!([{ "type": "screen", "name": "title" }]));

    // A battle has no relics screen. The title keeps the run in the camp.
    rook_win(&mut session, 1);
    assert_eq!(ask(&mut session, json!({ "cmd": "open_relics" }))["error"]["code"], json!("wrong_screen"));
    send(&mut session, json!({ "cmd": "to_title" }));
    send(&mut session, json!({ "cmd": "open_relics" }));
    let title = send(&mut session, json!({ "cmd": "back" }))["view"].clone();
    assert_eq!(title["run"], json!({ "floor": 2, "phase": "camp" }));

    send(&mut session, json!({ "cmd": "new_run" }));
    send(&mut session, json!({ "cmd": "give_up" }));
    assert_eq!(send(&mut session, json!({ "cmd": "open_relics" }))["view"]["screen"], json!("relics"));
    assert_eq!(send(&mut session, json!({ "cmd": "back" }))["view"]["screen"], json!("title"));
}

#[test]
fn buy_relic_takes_crowns_and_the_game_can_then_offer_the_relic() {
    let storage = Shared::default();
    let mut session = Session::with_debug(Box::new(storage.clone()), 1, true);
    send(&mut session, json!({ "cmd": "open_relics" }));
    let view = send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 5 }))["view"].clone();
    let locked = |cost: Value, feat: Value, affordable: bool| json!({ "unlocked": false, "id": null, "name": null, "text": null, "cost": cost, "feat": feat, "affordable": affordable });
    assert_eq!(view["slots"][CROWN_SLOT], locked(json!(3), Value::Null, true));
    assert_eq!(view["slots"][GALLOP_SLOT], locked(json!(8), Value::Null, false));
    let feat = json!("Give checkmate with a move of a knight.");
    assert_eq!(view["slots"][FEAT_SLOT], locked(Value::Null, feat, false));
    assert!(!offer_pool(session.meta(), &defaults()).contains(&relic("backpedal")));

    let reply = send(&mut session, json!({ "cmd": "buy_relic", "slot": CROWN_SLOT }));
    let text = "Your pawns can move one square backward to an empty square.";
    let event = json!({
        "type": "relic_unlocked", "id": "backpedal", "name": "Tactical Retreat", "text": text,
        "feat": null, "crowns_before": 5, "crowns": 2,
    });
    assert_eq!(reply["events"], json!([event]));
    let view = reply["view"].clone();
    let slot = json!({
        "unlocked": true, "id": "backpedal", "name": "Tactical Retreat", "text": text,
        "cost": null, "feat": null, "affordable": false,
    });
    assert_eq!((&view["slots"][CROWN_SLOT], &view["unlocked"], &view["meta"]["crowns"]), (&slot, &json!(9), &json!(2)));
    assert!(offer_pool(session.meta(), &defaults()).contains(&relic("backpedal")));

    // A second start of the program has the relic.
    let mut second = Session::new(Box::new(storage), 9);
    assert_eq!(send(&mut second, json!({ "cmd": "open_relics" }))["view"], view);
    assert_eq!(second.meta(), session.meta());
}

#[test]
fn a_refused_buy_relic_changes_nothing() {
    let mut session = debug_session();
    let title = send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 5 }))["view"].clone();
    let reply = ask(&mut session, json!({ "cmd": "buy_relic", "slot": CROWN_SLOT }));
    assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!("wrong_screen"), &title));
    send(&mut session, json!({ "cmd": "open_relics" }));
    send(&mut session, json!({ "cmd": "buy_relic", "slot": CROWN_SLOT }));
    let before = send(&mut session, json!({ "cmd": "view" }))["view"].clone();
    assert_eq!(before["meta"]["crowns"], json!(2));
    for (request, code) in [
        (json!({ "cmd": "buy_relic" }), "bad_args"),
        (json!({ "cmd": "buy_relic", "slot": "backpedal" }), "bad_args"),
        (json!({ "cmd": "buy_relic", "slot": -1 }), "bad_args"),
        (json!({ "cmd": "buy_relic", "slot": 2.5 }), "bad_args"),
        (json!({ "cmd": "buy_relic", "slot": EMPTY_SLOT }), "bad_index"),
        (json!({ "cmd": "buy_relic", "slot": 35 }), "bad_index"),
        (json!({ "cmd": "buy_relic", "slot": 36 }), "bad_index"),
        // A starter, a relic that the player bought, and a relic of a feat.
        (json!({ "cmd": "buy_relic", "slot": 0 }), "blocked"),
        (json!({ "cmd": "buy_relic", "slot": CROWN_SLOT }), "blocked"),
        (json!({ "cmd": "buy_relic", "slot": FEAT_SLOT }), "blocked"),
        (json!({ "cmd": "buy_relic", "slot": CROWN_SLOT + 1 }), "not_affordable"),
        (json!({ "cmd": "buy_upgrade", "upgrade": "pawn" }), "wrong_screen"),
    ] {
        let reply = ask(&mut session, request.clone());
        assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!(code), &before), "{request}");
    }
    assert_eq!(session.meta().relics, BTreeSet::from([relic("backpedal")]));
}

/// The name, the text, and the id of a locked relic are not in the view, also not in a message
/// of a refused purchase.
#[test]
fn the_relics_view_has_no_trace_of_a_locked_relic() {
    let mut session = debug_session();
    send(&mut session, json!({ "cmd": "open_relics" }));
    send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 4 }));
    let check = |session: &Session, reply: &Value, locked: usize| {
        let text = json!([reply["view"], reply["error"], reply["events"]]).to_string();
        let hidden: Vec<RelicId> = RelicId::all().filter(|&id| !session.meta().has_relic(id)).collect();
        assert_eq!(hidden.len(), locked);
        for id in hidden {
            let def = id.def();
            for trace in [def.key, def.name, def.text, def.foe_text.unwrap_or(def.text)] {
                assert!(!text.contains(trace), "the response has \"{trace}\" of the locked relic {}", def.key);
            }
        }
    };
    let reply = send(&mut session, json!({ "cmd": "view" }));
    check(&session, &reply, 19);
    for slot in [CROWN_SLOT, GALLOP_SLOT, FEAT_SLOT, CROWN_SLOT, EMPTY_SLOT] {
        let reply = ask(&mut session, json!({ "cmd": "buy_relic", "slot": slot }));
        check(&session, &reply, 18);
    }
}

#[test]
fn hello_has_the_unlock_rule_of_each_relic() {
    let content = send(&mut debug_session(), json!({ "cmd": "hello" }))["data"]["content"].clone();
    assert_eq!(content["relic_board_slots"], json!(36));
    let rule = |key: &str| {
        let relics = content["relics"].as_array().unwrap();
        let relic = relics.iter().find(|relic| relic["id"] == key).unwrap();
        (relic["unlock"].clone(), relic["cost"].clone())
    };
    assert_eq!(rule("bounty"), (json!("start"), Value::Null));
    assert_eq!(rule("gallop"), (json!("crowns"), json!(8)));
    assert_eq!(rule("longLeap"), (json!("feat"), Value::Null));
}

#[test]
fn debug_set_unlock_unlocks_and_locks_a_relic_and_does_not_start_the_battle_again() {
    let storage = Shared::default();
    let mut session = Session::with_debug(Box::new(storage.clone()), 1, true);
    let reply = send(&mut session, json!({ "cmd": "debug_set_unlock", "relic": "gallop", "unlocked": true }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "unlock" }]));
    assert_eq!(reply["data"]["debug"]["meta"]["relics"], json!(["gallop"]));
    send(&mut session, json!({ "cmd": "new_run" }));
    player(&mut session, "e2", "e4");
    let before = send(&mut session, json!({ "cmd": "view" }))["view"].clone();
    let reply = send(&mut session, json!({ "cmd": "debug_set_unlock", "relic": "longLeap", "unlocked": true }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "unlock" }]));
    assert_eq!(reply["view"], before);
    let meta = &reply["data"]["debug"]["meta"];
    assert_eq!((&meta["relics"], &meta["feats"]), (&json!(["gallop"]), &json!(["knightMate"])));
    assert_eq!(Session::new(Box::new(storage.clone()), 2).meta(), session.meta());

    // A starter stays unlocked.
    for key in ["gallop", "longLeap", "bounty"] {
        send(&mut session, json!({ "cmd": "debug_set_unlock", "relic": key, "unlocked": false }));
    }
    assert_eq!(session.meta(), &Meta::default());
    assert_eq!(Session::new(Box::new(storage), 2).meta(), &Meta::default());
    for request in [
        json!({ "cmd": "debug_set_unlock", "relic": "removedRelic", "unlocked": true }),
        json!({ "cmd": "debug_set_unlock", "relic": "gallop" }),
        json!({ "cmd": "debug_set_unlock", "unlocked": true }),
    ] {
        assert_eq!(ask(&mut session, request.clone())["error"]["code"], json!("bad_args"), "{request}");
    }
    let mut plain = Session::new(Box::new(MemoryStorage::default()), 1);
    let reply = ask(&mut plain, json!({ "cmd": "debug_set_unlock", "relic": "gallop", "unlocked": true }));
    assert_eq!(reply["error"]["code"], json!("debug_disabled"));
}

#[test]
fn a_checkmate_by_a_knight_unlocks_long_leap_one_time() {
    let mut session = debug_session();
    assert_eq!(unlocked_by(&rook_win(&mut session, 1)), ["blessing"]);
    // The enemy king on a5 has its pieces on each square around it. The knight gives checkmate from c4.
    let cage = [("k", "a5"), ("p", "a4"), ("p", "b4"), ("r", "b5"), ("p", "b6"), ("p", "a6")];
    battle(&mut session, 1, &[("k", "e1"), ("n", "b2")], &cage);
    let reply = won(player(&mut session, "b2", "c4"));
    let event = json!({
        "type": "relic_unlocked", "id": "longLeap", "name": "Long Leap",
        "text": "Your knights can also jump three squares in one direction and one square to the side.",
        "feat": "Give checkmate with a move of a knight.", "crowns_before": 0, "crowns": 0,
    });
    let events = reply["events"].as_array().unwrap();
    assert_eq!((&reply["view"]["result"]["reason"], events.last()), (&json!("checkmate"), Some(&event)));
    assert_eq!(unlocked_by(&reply), ["longLeap"]);
    assert!(offer_pool(session.meta(), &defaults()).contains(&relic("longLeap")));

    battle(&mut session, 1, &[("k", "e1"), ("n", "b2")], &cage);
    let reply = won(player(&mut session, "b2", "c4"));
    assert_eq!(unlocked_by(&reply), NONE);
    assert_eq!(session.meta().feats, BTreeSet::from([Feat::KnightMate, Feat::CleanWin]));
}

#[test]
fn a_checkmate_by_a_rook_unlocks_enfilade() {
    let mut session = debug_session();
    // A win by a capture of the rook is not a checkmate.
    assert_eq!(unlocked_by(&rook_win(&mut session, 1)), ["blessing"]);
    battle(&mut session, 1, &[("k", "e1"), ("r", "a1")], &[("k", "h8"), ("p", "g7"), ("p", "h7")]);
    let reply = won(player(&mut session, "a1", "a8"));
    assert_eq!((&reply["view"]["result"]["reason"], unlocked_by(&reply)), (&json!("checkmate"), vec!["enfilade"]));
}

#[test]
fn a_win_with_no_lost_piece_unlocks_blessing_and_a_win_with_three_lost_pieces_unlocks_gambit() {
    let mut session = debug_session();
    for pawns in [1, 2] {
        assert_eq!(unlocked_by(&win_with_losses(&mut session, pawns, &[])), NONE, "{pawns}");
    }
    // Second Wind returns one of the three pawns: the player lost two units.
    assert_eq!(unlocked_by(&win_with_losses(&mut session, 3, &["secondWind"])), NONE);
    assert_eq!(unlocked_by(&win_with_losses(&mut session, 3, &[])), ["gambit"]);
    assert_eq!(unlocked_by(&win_with_losses(&mut session, 3, &[])), NONE);
    assert_eq!(unlocked_by(&rook_win(&mut session, 1)), ["blessing"]);
    assert_eq!(unlocked_by(&rook_win(&mut session, 1)), NONE);
}

#[test]
fn two_promotions_in_a_battle_that_the_player_wins_unlock_apprenticeship() {
    let mut session = debug_session();
    assert_eq!(unlocked_by(&win_with_promotions(&mut session, 1)), ["blessing"]);
    assert_eq!(unlocked_by(&win_with_promotions(&mut session, 2)), ["apprenticeship"]);
}

#[test]
fn a_win_of_the_run_unlocks_royal_steed() {
    let mut session = debug_session();
    assert_eq!(unlocked_by(&rook_win(&mut session, FLOORS.len() - 1)), ["blessing"]);
    let reply = rook_win(&mut session, FLOORS.len());
    assert_eq!((&reply["view"]["result"]["next"], unlocked_by(&reply)), (&json!("won"), vec!["kingKnight"]));
    // The crowns of the run come after the event.
    let event = reply["events"].as_array().unwrap().last().unwrap();
    assert_eq!((&event["crowns_before"], &event["crowns"]), (&json!(0), &json!(0)));
    assert_eq!(send(&mut session, json!({ "cmd": "continue" }))["view"]["meta"]["crowns"], json!(13));
}

#[test]
fn a_battle_that_the_player_gives_up_unlocks_nothing() {
    let mut session = debug_session();
    battle(&mut session, FLOORS.len(), &[("k", "e1"), ("r", "a1")], &[("k", "e8"), ("p", "a2")]);
    let reply = send(&mut session, json!({ "cmd": "give_up" }));
    assert_eq!(unlocked_by(&reply), NONE);
    assert_eq!(session.meta().feats, BTreeSet::new());
}

/// The feat comes before the camp, thus the reward and the shop of that camp can have the relic.
#[test]
fn the_camp_after_the_battle_of_a_feat_can_offer_its_relic() {
    let offers = |seed: u64| -> bool {
        let mut session = debug_session();
        send(&mut session, json!({ "cmd": "debug_set_seed", "seed": seed }));
        assert_eq!(unlocked_by(&rook_win(&mut session, 1)), ["blessing"]);
        let camp = send(&mut session, json!({ "cmd": "continue" }))["view"].clone();
        let cards = camp["reward"]["offers"].as_array().unwrap().iter();
        cards.chain(camp["shop"]["offers"].as_array().unwrap()).any(|offer| offer["id"] == "blessing")
    };
    assert!((0..60).any(offers));
}

#[test]
fn a_unit_that_second_wind_returns_and_the_pawn_of_conscription_do_not_stop_the_feat_of_blessing() {
    let mut session = debug_session();
    let reply = win_with_losses(&mut session, 1, &["secondWind"]);
    assert_eq!((&reply["view"]["lost"], unlocked_by(&reply)), (&json!([]), vec!["blessing"]));

    // The enemy rook captures only the pawn of Conscription on a2.
    let mut session = debug_session();
    battle(&mut session, 1, &[("k", "b1"), ("p", "h2")], &[("k", "h8"), ("r", "a8")]);
    send(&mut session, json!({ "cmd": "debug_set_relic", "relic": "conscription", "on": true }));
    player(&mut session, "h2", "h3");
    let capture = enemy(&mut session, "a8", "a2");
    assert!(capture["events"].as_array().unwrap().iter().any(|e| e["type"] == "capture" && e["id"] == 20000));
    let reply = won(player(&mut session, "b1", "a2"));
    assert_eq!((&reply["view"]["taken"]["b"], unlocked_by(&reply)), (&json!(["p"]), vec!["blessing"]));
}

/// The feats are feats of the player.
#[test]
fn a_checkmate_by_an_enemy_knight_does_no_feat() {
    let mut session = debug_session();
    let units = [("k", "a1"), ("r", "b1"), ("p", "a2"), ("p", "b2"), ("p", "h2")];
    battle(&mut session, 1, &units, &[("k", "h8"), ("n", "d4")]);
    player(&mut session, "h2", "h3");
    let reply = enemy(&mut session, "d4", "c2");
    let result = &reply["view"]["result"];
    assert_eq!((&result["winner"], &result["reason"]), (&json!("b"), &json!("checkmate")));
    assert_eq!((unlocked_by(&reply), &session.meta().feats), (NONE.to_vec(), &BTreeSet::new()));
}

#[test]
fn a_draw_does_no_feat() {
    // The king captures the last enemy piece: only the kings remain.
    let mut session = debug_session();
    battle(&mut session, FLOORS.len(), &[("k", "e1")], &[("k", "e8"), ("p", "e2")]);
    let reply = player(&mut session, "e1", "e2");
    let result = &reply["view"]["result"];
    assert_eq!((&result["winner"], &result["reason"]), (&Value::Null, &json!("bare")));
    assert_eq!((unlocked_by(&reply), &session.meta().feats), (NONE.to_vec(), &BTreeSet::new()));
}

/// A feat with no relic to unlock gives no event. It is in the saved data.
#[test]
fn a_feat_that_unlocks_no_relic_is_saved() {
    let storage = Shared::default();
    let mut meta = Meta::default();
    meta.relics.extend([relic("longLeap"), relic("blessing")]);
    storage.0.borrow_mut().meta = Some(chrogue_game::save::meta_document(&meta));
    let mut session = Session::with_debug(Box::new(storage.clone()), 1, true);
    assert_eq!(session.meta(), &meta);
    let cage = [("k", "a5"), ("p", "a4"), ("p", "b4"), ("r", "b5"), ("p", "b6"), ("p", "a6")];
    battle(&mut session, 1, &[("k", "e1"), ("n", "b2")], &cage);
    let reply = won(player(&mut session, "b2", "c4"));
    assert_eq!(unlocked_by(&reply), NONE);
    meta.feats.extend([Feat::KnightMate, Feat::CleanWin]);
    assert_eq!(session.meta(), &meta);
    assert_eq!(Session::new(Box::new(storage), 2).meta(), &meta);
}

/// The offers of a session, not only of `roll_draft` and `roll_shop`: the reward and the shop of
/// a camp, and the shop after each `reroll`.
#[test]
fn a_camp_of_a_new_save_offers_only_starter_relics_also_after_a_reroll() {
    let mut relics = 0;
    for seed in 0..30 {
        let mut session = debug_session();
        send(&mut session, json!({ "cmd": "debug_set_seed", "seed": seed }));
        // A win with a lost unit does no feat, thus the save has only the starter relics.
        assert_eq!(unlocked_by(&win_with_losses(&mut session, 1, &[])), NONE);
        send(&mut session, json!({ "cmd": "continue" }));
        let mut view = send(&mut session, json!({ "cmd": "debug_set_gold", "gold": 100 }))["view"].clone();
        for _ in 0..6 {
            let cards = view["reward"]["offers"].as_array().unwrap().iter();
            for offer in cards.chain(view["shop"]["offers"].as_array().unwrap()).filter(|o| o["kind"] == "relic") {
                relics += 1;
                assert!(is_starter(relic(offer["id"].as_str().unwrap())), "seed {seed}: {}", offer["id"]);
            }
            view = send(&mut session, json!({ "cmd": "reroll" }))["view"].clone();
        }
    }
    assert!(relics > 300, "{relics} relic offers");
}
