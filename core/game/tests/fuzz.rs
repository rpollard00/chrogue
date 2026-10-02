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

fn piece_kind(g: &mut Gen) -> &'static str {
    g.pick(&["k", "q", "r", "b", "n", "p"])
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
            // The AI search on a high floor takes time, thus most enemy moves come from the list of
            // a Scout view or from the AI of floor 1.
            "enemy" => match g.below(3) {
                0 => json!({ "cmd": "debug_ai_move", "level": 1 }),
                1 if view["floor"]["number"].as_u64().unwrap_or(9) <= 2 => json!({ "cmd": "enemy_move" }),
                _ => json!({ "cmd": "debug_ai_move", "level": 1 + g.below(2) }),
            },
            "over" => json!({ "cmd": "continue" }),
            _ => json!({ "cmd": "give_up" }),
        },
        "camp" => {
            let shop = view["shop"]["offers"].as_array().map_or(0, Vec::len);
            match g.below(9) {
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
        "from", "to", "promo", "index", "unit", "square", "upgrade", "relic", "on", "barred", "level", "floor", "gold",
        "crowns", "kind", "units", "pieces", "traits", "offers", "run",
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
                "upgrade" => json!(*g.pick(&["pawn", "gold", "bishop", "haggle", "scout", "x"])),
                "relic" => json!(*g.pick(&["bounty", "secondWind", "kingKnight", "earlyPromo", "y"])),
                "on" | "barred" | "run" => json!(g.chance(0.5)),
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
