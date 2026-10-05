//! Robustness: random requests (valid ones, valid ones on the wrong screen, malformed JSON, wrong
//! types, huge numbers, out-of-range squares) to sessions. No request may panic or get the error
//! `internal`, and after each refused request the view is the view from before the request.

use std::collections::BTreeMap;

use chrogue_game::protocol::{Command, MAX_REQUEST_BYTES};
use chrogue_game::random::Dice;
use chrogue_game::{MemoryStorage, Session};
use serde_json::{Value, json};

const SESSIONS: u64 = 24;
const REQUESTS: usize = 2_200;

struct Gen(Dice);

impl Gen {
    fn below(&mut self, n: usize) -> usize {
        self.0.below(n)
    }
    fn chance(&mut self, p: f64) -> bool {
        self.0.unit() < p
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// A value of a random JSON type, often one that a command does not accept.
fn junk(g: &mut Gen) -> Value {
    match g.below(14) {
        0 => Value::Null,
        1 => json!(true),
        2 => json!(-1),
        3 => json!(64),
        4 => json!(1e300),
        5 => json!(18_446_744_073_709_551_615u64),
        6 => json!(-9_223_372_036_854_775_808i64),
        7 => json!(2.5),
        8 => json!("q"),
        9 => json!("bounty"),
        10 => json!([1, 2, 3]),
        11 => json!({ "kind": "k", "square": 60 }),
        12 => json!(g.below(70)),
        _ => json!(""),
    }
}

/// The ids of the upgrades, and an id that no upgrade has.
const UPGRADE_IDS: [&str; 13] = [
    "pawn",
    "gold",
    "bishop",
    "haggle",
    "scout",
    "knight",
    "heirloom",
    "antiquary",
    "fixer",
    "envoy",
    "muster",
    "curator",
    "x",
];

fn piece_kind(g: &mut Gen) -> &'static str {
    g.pick(&["k", "q", "r", "b", "n", "p"])
}

/// A `debug_tune` with numbers that are mostly in their ranges. The level stays, thus the AI of
/// each floor stays fast.
fn tune(g: &mut Gen) -> Value {
    match g.below(4) {
        0 => json!({ "cmd": "debug_tune", "reset": true }),
        1 => json!({ "cmd": "debug_tune", "floor": 1 + g.below(8), "budget": g.below(42), "traits": g.below(4) }),
        _ => {
            let (kind, weight) = (piece_kind(g), g.below(20) as f64 / 2.0);
            json!({ "cmd": "debug_tune", "kind": kind, "cap": g.below(4), "weight": weight, "min_floor": 1 + g.below(8) })
        }
    }
}

/// A command that is valid for the view, when the view has one.
fn valid(g: &mut Gen, view: &Value) -> Value {
    let screen = view["screen"].as_str().unwrap_or("");
    match screen {
        "title" => match g.below(4) {
            0 if view["can_continue"] == json!(true) => json!({ "cmd": "continue_run" }),
            1 => json!({ "cmd": "open_upgrades" }),
            _ => json!({ "cmd": "new_run" }),
        },
        "upgrades" => {
            let slots: Vec<Value> =
                view["slots"].as_array().unwrap().iter().filter(|s| !s.is_null()).cloned().collect();
            if g.chance(0.6) {
                json!({ "cmd": "buy_upgrade", "upgrade": g.pick(&slots)["id"] })
            } else {
                json!({ "cmd": "back" })
            }
        }
        "battle" => match view["phase"].as_str().unwrap() {
            "player" if g.chance(0.05) => json!({ "cmd": "move", "from": g.below(64), "to": g.below(64) }),
            "player" if g.chance(0.97) => {
                let moves = view["moves"].as_array().unwrap();
                let m = g.pick(moves);
                // Some promotions come with no piece, thus the core asks for one.
                let promo = if g.chance(0.8) { m.get("promo").cloned() } else { None };
                json!({ "cmd": "move", "from": m["from"], "to": m["to"], "promo": promo })
            }
            // The AI search at a high level takes time, thus the enemy moves come from the AI of the
            // floor, which is a beginner level, and from levels 4 and 5.
            "enemy" => match g.below(3) {
                0 => json!({ "cmd": "debug_ai_move", "level": 4 }),
                1 => json!({ "cmd": "enemy_move" }),
                _ => json!({ "cmd": "debug_ai_move", "level": 4 + g.below(2) }),
            },
            "over" => json!({ "cmd": "continue" }),
            _ => json!({ "cmd": "give_up" }),
        },
        "camp" => {
            let shop = view["shop"]["offers"].as_array().map_or(0, Vec::len);
            match g.below(12) {
                0 => json!({ "cmd": "take_reward", "index": g.below(3) }),
                1 => json!({ "cmd": "skip_reward" }),
                2 => json!({ "cmd": "buy", "index": g.below(shop.max(1)) }),
                3 => json!({ "cmd": "reroll" }),
                4 => {
                    let army = view["army"].as_array().unwrap();
                    json!({ "cmd": "place", "unit": g.pick(army)["id"], "square": g.below(16) })
                }
                5 => json!({ "cmd": "debug_set_gold", "gold": g.below(100) }),
                6 => json!({ "cmd": "debug_add_unit", "kind": piece_kind(g) }),
                7 => tune(g),
                // The run often does not have the relic.
                8 => {
                    let relic =
                        view["relics"].as_array().and_then(|relics| relics.first()).map(|relic| relic["id"].clone());
                    json!({ "cmd": "discard_relic", "relic": relic.unwrap_or(json!("bounty")) })
                }
                9 => json!({ "cmd": "debug_set_relic_slots", "slots": g.below(12) }),
                _ => json!({ "cmd": "start_battle" }),
            }
        }
        _ => json!({ "cmd": *g.pick(&["new_run", "open_upgrades", "to_title"]) }),
    }
}

/// A command with arguments of the right names and random values, on any screen.
fn any_command(g: &mut Gen) -> Value {
    let command = *g.pick(Command::ALL);
    let mut request = json!({ "cmd": command.name() });
    let names = [
        "from",
        "to",
        "promo",
        "index",
        "unit",
        "square",
        "upgrade",
        "relic",
        "on",
        "barred",
        "level",
        "floor",
        "gold",
        "crowns",
        "kind",
        "units",
        "pieces",
        "traits",
        "offers",
        "run",
        "seed",
        "reset",
        "budget",
        "cap",
        "weight",
        "min_floor",
        "slots",
    ];
    for _ in 0..g.below(4) {
        let name = *g.pick(&names);
        let value = match g.below(4) {
            0 => junk(g),
            1 => json!(g.below(64)),
            2 => json!(piece_kind(g)),
            _ => match name {
                "units" => {
                    json!([{ "kind": "k", "home": g.below(20) }, { "kind": piece_kind(g), "home": g.below(20), "id": g.below(5) }])
                }
                "pieces" => {
                    json!([{ "kind": "k", "square": g.below(70) }, { "kind": piece_kind(g), "square": g.below(70) }])
                }
                "offers" => {
                    json!([{ "kind": "piece", "type": piece_kind(g) }, { "kind": "relic", "id": "bounty" }, { "kind": "gold", "amount": g.below(9) }])
                }
                "traits" => json!([*g.pick(&["sidestep", "bounty", "forcedMarch", "nope"])]),
                "upgrade" => json!(*g.pick(&UPGRADE_IDS)),
                "relic" => json!(*g.pick(&["bounty", "secondWind", "kingKnight", "earlyPromo", "y"])),
                "on" | "barred" | "run" | "reset" => json!(g.chance(0.5)),
                _ => json!(g.below(10)),
            },
        };
        request[name] = value;
    }
    if g.chance(0.3) {
        request["id"] = junk(g);
    }
    request
}

/// A request that is not a valid request.
fn malformed(g: &mut Gen) -> String {
    match g.below(12) {
        0 => String::new(),
        1 => "{".into(),
        2 => "{\"cmd\":\"move\",\"from\":".into(),
        3 => "[\"move\"]".into(),
        4 => "42".into(),
        5 => "{\"cmd\":5}".into(),
        6 => "{\"from\":1,\"to\":2}".into(),
        7 => "{\"cmd\":\"no_such_command\"}".into(),
        8 => "[".repeat(200 + g.below(300)),
        9 => format!("{{\"cmd\":\"view\",\"pad\":\"{}\"}}", "x".repeat(MAX_REQUEST_BYTES)),
        10 => (0..g.below(40)).map(|_| char::from_u32(g.below(0x2fff) as u32).unwrap_or('?')).collect(),
        _ => "{\"cmd\":\"move\",\"from\":1e400,\"to\":-0}".into(),
    }
}

#[test]
fn random_requests_never_break_a_session() {
    let mut total = 0;
    let mut codes: BTreeMap<String, usize> = BTreeMap::new();
    let mut ok = 0;
    for seed in 0..SESSIONS {
        let debug = seed % 4 != 3;
        let mut session = Session::with_debug(Box::new(MemoryStorage::default()), seed, debug);
        let mut g = Gen(Dice::new(seed + 1000));
        for _ in 0..REQUESTS {
            let before = session.view();
            let request = match g.below(10) {
                0..=4 => valid(&mut g, &before).to_string(),
                5..=7 => any_command(&mut g).to_string(),
                _ => malformed(&mut g),
            };
            let text = session.command(&request);
            total += 1;
            let reply: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
            if reply["ok"] == json!(true) {
                ok += 1;
                continue;
            }
            let code = reply["error"]["code"].as_str().unwrap_or("(none)").to_string();
            assert_ne!(code, "internal", "{request} -> {}", reply["error"]);
            assert_eq!(reply["view"], before, "the view changed after the refused request {request}");
            assert_eq!(session.view(), before, "the state changed after the refused request {request}");
            *codes.entry(code).or_default() += 1;
        }
    }
    println!("{total} requests, {ok} accepted, refused: {codes:?}");
    assert!(total >= 50_000);
    for code in [
        "bad_json",
        "bad_request",
        "unknown_command",
        "wrong_screen",
        "bad_args",
        "too_long",
        "debug_disabled",
        "illegal_move",
        "wrong_phase",
        "not_affordable",
    ] {
        assert!(codes.contains_key(code), "no request got {code}");
    }
}

/// The same seed and the same requests give the same responses, byte for byte.
#[test]
fn a_seed_and_the_requests_give_the_same_transcript() {
    let transcript = |seed: u64| -> Vec<String> {
        let mut session = Session::with_debug(Box::new(MemoryStorage::default()), seed, true);
        let mut g = Gen(Dice::new(seed));
        (0..3000).map(|_| session.command(&valid(&mut g, &session.view()).to_string())).collect()
    };
    assert_eq!(transcript(8), transcript(8));
    assert_ne!(transcript(8), transcript(9));
}

// ---- Saved files and debug boards ----

/// The number of ids at the start of `RELIC_IDS` that a boss can have as a trait.
const TRAITS: usize = 15;

const RELIC_IDS: [&str; 22] = [
    "forcedMarch",
    "backpedal",
    "earlyPromo",
    "kingKnight",
    "longLeap",
    "sidestep",
    "vault",
    "crossfire",
    "closeQuarters",
    "pilgrimLeap",
    "queenFlight",
    "gallop",
    "crusade",
    "royalMarch",
    "huntress",
    "bounty",
    "secondWind",
    "conscription",
    "interest",
    "apprenticeship",
    "coup",
    "gambit",
];

/// A huge or odd number for a count of saved data.
fn big(g: &mut Gen) -> Value {
    g.pick(&[
        json!(0),
        json!(5),
        json!(u64::MAX),
        json!(9_007_199_254_740_991u64),
        json!(9_007_199_254_740_992u64),
        json!(1e15),
        json!(1e300),
        json!(-1),
        json!(2.5),
    ])
    .clone()
}

/// A run document with squares that often overlap, a king that is often missing, and huge numbers.
fn run_doc(g: &mut Gen) -> String {
    // The first piece is a king, with some exceptions; another piece is a second king at times.
    let king = |g: &mut Gen, i: usize| {
        if g.chance(if i == 0 { 0.9 } else { 0.03 }) { "k" } else { *g.pick(&["q", "r", "b", "n", "p"]) }
    };
    let army: Vec<Value> = (0..1 + g.below(16))
        .map(|i| json!({ "id": if g.chance(0.05) { big(g) } else { json!(i + 1) }, "type": king(g, i), "home": if g.chance(0.03) { g.below(17) } else { i } }))
        .collect();
    let pieces: Vec<Value> = (0..1 + if g.chance(0.9) { g.below(16) } else { g.below(66) })
        .map(|i| {
            let square = if g.chance(0.97) { 63 - (i % 64) } else { g.below(65) };
            json!({ "type": king(g, i), "square": square })
        })
        .collect();
    let relics: Vec<&str> = (0..g.below(6)).map(|_| *g.pick(&RELIC_IDS)).collect();
    let traits: Vec<&str> = (0..g.below(4)).map(|_| *g.pick(&RELIC_IDS[..TRAITS])).collect();
    let gold = if g.chance(0.8) { json!(g.below(50)) } else { big(g) };
    let floor = if g.chance(0.9) { json!(1 + g.below(8)) } else { big(g) };
    let next_id = if g.chance(0.9) { json!(17 + g.below(20_000)) } else { big(g) };
    let draft = if g.chance(0.5) {
        Value::Null
    } else {
        json!([{ "kind": "piece", "type": "q" }, { "kind": "relic", "id": "bounty" }, { "kind": "gold", "amount": big(g) }])
    };
    let mut doc = json!({ "format": "chrogue.run", "version": if g.chance(0.95) { json!(1) } else { big(g) }, "data": {
        "floor": floor, "gold": gold, "army": army, "nextId": next_id, "relics": relics,
        "enemy": { "pieces": pieces, "traits": traits }, "phase": if g.chance(0.5) { "camp" } else { "battle" },
        "draft": draft, "shop": [{ "kind": "piece", "type": "p" }, { "kind": "relic", "id": *g.pick(&RELIC_IDS) }, { "kind": "gold", "amount": big(g) }],
    } });
    // A file of an older core has no seed and no rolls.
    if g.chance(0.7) {
        doc["data"]["seed"] = if g.chance(0.8) { json!(g.below(1_000_000_000)) } else { big(g) };
        doc["data"]["rolls"] = if g.chance(0.8) { json!(g.below(5)) } else { big(g) };
    }
    // A file of an older core has no slots.
    if g.chance(0.8) {
        doc["data"]["slots"] = if g.chance(0.85) { json!(g.below(11)) } else { big(g) };
    }
    doc.to_string()
}

fn meta_doc(g: &mut Gen) -> String {
    let text = json!({ "format": "chrogue.meta", "version": 1, "data": {
        "crowns": big(g), "best": big(g), "runs": big(g),
        "upgrades": UPGRADE_IDS.iter().map(|&id| (id.to_string(), big(g))).collect::<serde_json::Map<_, _>>(),
    } })
    .to_string();
    // Sometimes a file that a crash cut.
    if g.chance(0.1) { text[..g.below(text.len())].to_string() } else { text }
}

/// Each square has at most one piece, and at the start of a battle each side has one king.
fn check_board(reply: &Value, request: &Value) {
    let view = &reply["view"];
    if view["screen"] != json!("battle") {
        return;
    }
    let pieces = view["pieces"].as_array().unwrap();
    let mut squares: Vec<u64> = pieces.iter().map(|p| p["square"].as_u64().unwrap()).collect();
    squares.sort_unstable();
    squares.dedup();
    assert_eq!(squares.len(), pieces.len(), "two pieces on one square after {request}");
    let started = reply["events"].as_array().is_some_and(|e| e.iter().any(|e| e["type"] == json!("battle_start")));
    if started {
        for color in ["w", "b"] {
            let kings = pieces.iter().filter(|p| p["color"] == json!(color) && p["kind"] == json!("k")).count();
            assert_eq!(kings, 1, "{color} has {kings} kings after {request}");
        }
    }
}

#[test]
fn saved_files_and_debug_boards_that_are_not_valid_are_refused() {
    let (mut loaded, mut boards) = (0, 0);
    for seed in 0..300u64 {
        let mut g = Gen(Dice::new(seed * 7 + 1));
        let storage = MemoryStorage { meta: Some(meta_doc(&mut g)), run: Some(run_doc(&mut g)) };
        let mut session = Session::with_debug(Box::new(storage), seed, true);
        if session.view()["can_continue"] == json!(true) {
            loaded += 1;
            // A run that loads is a run that the game can continue.
            let reply: Value = serde_json::from_str(&session.command("{\"cmd\":\"continue_run\"}")).unwrap();
            assert_eq!(reply["ok"], json!(true), "seed {seed}: {}", reply["error"]);
            check_board(&reply, &json!("continue_run"));
        }
        for step in 0..150 {
            let before = session.view();
            let request = match step % 25 {
                7 | 17 => {
                    let n = 1 + g.below(66);
                    let king = g.chance(0.85);
                    let pieces: Vec<Value> = (0..n)
                        .map(|i| {
                            let kind = if i == 0 && king { "k" } else { piece_kind(&mut g) };
                            json!({ "kind": kind, "square": if g.chance(0.5) { (i * 13 + g.below(3)) % 64 } else { g.below(64) } })
                        })
                        .collect();
                    json!({ "cmd": "debug_set_enemy", "pieces": pieces, "traits": [*g.pick(&RELIC_IDS[..TRAITS])] })
                }
                11 => {
                    let units: Vec<Value> = (0..1 + g.below(16))
                        .map(|i| json!({ "kind": if i == 0 { "k" } else { piece_kind(&mut g) }, "home": g.below(16) }))
                        .collect();
                    json!({ "cmd": "debug_set_army", "units": units })
                }
                3 => json!({ "cmd": "debug_set_relic", "relic": *g.pick(&RELIC_IDS), "on": g.chance(0.7) }),
                13 => json!({ "cmd": "debug_add_unit", "kind": *g.pick(&["p", "n", "b", "r", "q"]) }),
                21 => json!({ "cmd": "debug_set_floor", "floor": 1 + g.below(8) }),
                5 => tune(&mut g),
                // A debug board can start with no legal move for the player (see PROTOCOL.md).
                _ if before["phase"] == json!("player") && before["moves"] == json!([]) => json!({ "cmd": "give_up" }),
                _ => valid(&mut g, &before),
            };
            let reply: Value = serde_json::from_str(&session.command(&request.to_string())).unwrap();
            let code = reply["error"]["code"].clone();
            assert_ne!(code, json!("internal"), "seed {seed}: {request} -> {}", reply["error"]);
            if reply["ok"] == json!(true) {
                check_board(&reply, &request);
            } else {
                assert_eq!(session.view(), before, "the state changed after the refused request {request}");
                let message = reply["error"]["message"].as_str().unwrap_or("");
                if message.starts_with("The board is not valid") {
                    assert_eq!(code, json!("bad_args"));
                    boards += 1;
                }
            }
        }
    }
    println!("saved runs that loaded: {loaded} of 300; debug boards refused: {boards}");
    assert!(loaded > 10 && loaded < 290, "{loaded} saved runs loaded");
    assert!(boards > 100, "{boards} debug boards were refused");
}
