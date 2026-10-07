//! The relic unlocks: the starter relics, the relics that crowns buy, and the relics of the feats.

use std::collections::BTreeSet;
use std::ops::Range;

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
use serde_json::json;

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
