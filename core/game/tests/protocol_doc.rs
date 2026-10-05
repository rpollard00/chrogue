//! `PROTOCOL.md` documents each command, event, and error code of the code, and each field
//! name of the responses of a scripted session that visits each screen and makes each event.

use std::collections::BTreeSet;

use chrogue_game::protocol::{Code, Command, EventKind};
use chrogue_game::{Doc, MemoryStorage, Session, Storage};
use serde_json::{Value, json};

fn doc() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../PROTOCOL.md")).unwrap()
}

/// A storage that cannot save, with a meta that a crash cut.
struct Broken;

impl Storage for Broken {
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String> {
        Ok((doc == Doc::Meta).then(|| "{\"format\":\"chrogue.meta\",\"vers".to_string()))
    }
    fn save(&mut self, _: Doc, _: &str) -> Result<(), String> {
        Err("read-only".into())
    }
    fn remove(&mut self, _: Doc) -> Result<(), String> {
        Err("read-only".into())
    }
    fn set_aside(&mut self, _: Doc) -> Result<Option<String>, String> {
        Ok(Some("meta.json.bad-1".into()))
    }
}

#[derive(Default)]
struct Seen {
    keys: BTreeSet<String>,
    events: BTreeSet<String>,
}

impl Seen {
    fn collect(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(kind)) = map.get("type") {
                    self.events.insert(kind.clone());
                }
                for (key, item) in map {
                    self.keys.insert(key.clone());
                    self.collect(item);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.collect(item)),
            _ => {}
        }
    }
}

struct Script {
    session: Session,
    seen: Seen,
}

impl Script {
    fn send(&mut self, request: Value) -> Value {
        let reply: Value = serde_json::from_str(&self.session.command(&request.to_string())).unwrap();
        assert_eq!(reply["ok"], json!(true), "{request} -> {}", reply["error"]);
        self.seen.collect(&reply);
        reply
    }

    fn army(&mut self, units: Value, enemy: Value) -> Value {
        self.send(json!({ "cmd": "debug_set_army", "units": units }));
        self.send(json!({ "cmd": "debug_set_enemy", "pieces": enemy, "traits": [] }))
    }
}

fn units(list: &[(&str, u8)]) -> Value {
    list.iter().map(|&(kind, home)| json!({ "kind": kind, "home": home })).collect()
}

fn pieces(list: &[(&str, u8)]) -> Value {
    list.iter().map(|&(kind, square)| json!({ "kind": kind, "square": square })).collect()
}

fn scripted_session() -> Seen {
    let mut s =
        Script { session: Session::with_debug(Box::new(MemoryStorage::default()), 21, true), seen: Seen::default() };
    s.send(json!({ "cmd": "hello" }));
    s.send(json!({ "cmd": "view", "run": true }));
    s.send(json!({ "cmd": "open_upgrades" }));
    s.send(json!({ "cmd": "debug_set_crowns", "crowns": 50 }));
    s.send(json!({ "cmd": "buy_upgrade", "upgrade": "scout" }));
    s.send(json!({ "cmd": "back" }));
    s.send(json!({ "cmd": "new_run" }));
    s.send(json!({ "cmd": "view", "run": true }));
    for relic in ["bounty", "interest", "secondWind", "conscription"] {
        s.send(json!({ "cmd": "debug_set_relic", "relic": relic, "on": true }));
    }
    s.send(json!({ "cmd": "debug_set_trait", "relic": "sidestep", "on": true }));
    s.send(json!({ "cmd": "debug_bar_relic", "relic": "longLeap", "barred": true }));
    s.send(json!({ "cmd": "debug_set_seed", "seed": 7 }));
    s.send(json!({ "cmd": "debug_set_relic_slots", "slots": 5 }));
    s.send(json!({ "cmd": "debug_tune", "floor": 5, "level": 5, "budget": 9 }));
    s.send(json!({ "cmd": "debug_tune", "kind": "q", "min_floor": 4 }));
    s.send(json!({ "cmd": "debug_state" }));

    // A castle, an en passant capture of a unit (Second Wind returns it), a promotion that
    // captures a second unit, and a capture with check.
    s.army(
        units(&[("k", 4), ("r", 7), ("r", 0), ("p", 12), ("n", 15)]),
        pieces(&[("k", 60), ("p", 27), ("p", 10), ("b", 58)]),
    );
    s.send(json!({ "cmd": "move", "from": 4, "to": 6 }));
    s.send(json!({ "cmd": "debug_enemy_move", "from": 58, "to": 51 }));
    s.send(json!({ "cmd": "move", "from": 12, "to": 28 }));
    s.send(json!({ "cmd": "debug_enemy_move", "from": 27, "to": 20 }));
    s.send(json!({ "cmd": "move", "from": 0, "to": 1 }));
    s.send(json!({ "cmd": "debug_enemy_move", "from": 10, "to": 1, "promo": "q" }));
    s.send(json!({ "cmd": "move", "from": 15, "to": 30 }));
    s.send(json!({ "cmd": "debug_enemy_move", "from": 1, "to": 5 }));
    s.send(json!({ "cmd": "debug_ai_move", "level": 2 }));
    s.send(json!({ "cmd": "enemy_move" }));

    // A won battle with Bounty and Interest, and the camp.
    s.send(json!({ "cmd": "debug_set_gold", "gold": 20 }));
    s.army(units(&[("k", 4), ("r", 0)]), pieces(&[("k", 60), ("r", 8)]));
    s.send(json!({ "cmd": "move", "from": 0, "to": 8 }));
    s.send(json!({ "cmd": "continue" }));
    s.send(json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "piece", "type": "n" }, { "kind": "gold", "amount": 12 }] }));
    s.send(json!({ "cmd": "take_reward", "index": 0 }));
    s.send(json!({ "cmd": "debug_set_shop", "offers": [{ "kind": "piece", "type": "p" }, { "kind": "relic", "id": "earlyPromo" }] }));
    s.send(json!({ "cmd": "buy", "index": 0 }));
    s.send(json!({ "cmd": "reroll" }));
    s.send(json!({ "cmd": "discard_relic", "relic": "conscription" }));
    s.send(json!({ "cmd": "place", "unit": 1, "square": 0 }));
    s.send(json!({ "cmd": "debug_add_unit", "kind": "q" }));
    s.send(json!({ "cmd": "debug_remove_unit", "unit": 2 }));
    s.send(json!({ "cmd": "debug_set_floor", "floor": 3 }));
    s.send(json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "gold", "amount": 3 }] }));
    s.send(json!({ "cmd": "skip_reward" }));
    s.send(json!({ "cmd": "to_title" }));
    s.send(json!({ "cmd": "continue_run" }));
    s.send(json!({ "cmd": "start_battle" }));
    s.send(json!({ "cmd": "debug_ai_move" }));
    s.send(json!({ "cmd": "to_title" }));
    s.send(json!({ "cmd": "continue_run" }));
    s.send(json!({ "cmd": "debug_set_upgrade", "upgrade": "pawn", "level": 1 }));

    // A lost run, a won run, and the end screen.
    s.send(json!({ "cmd": "give_up" }));
    s.send(json!({ "cmd": "open_upgrades" }));
    s.send(json!({ "cmd": "back" }));
    s.send(json!({ "cmd": "new_run" }));
    s.send(json!({ "cmd": "debug_set_floor", "floor": 8 }));
    s.army(units(&[("k", 4), ("r", 0)]), pieces(&[("k", 60), ("p", 8)]));
    s.send(json!({ "cmd": "move", "from": 0, "to": 8 }));
    s.send(json!({ "cmd": "continue" }));
    s.send(json!({ "cmd": "to_title" }));
    s.send(json!({ "cmd": "quit" }));

    let mut broken = Script { session: Session::new(Box::new(Broken), 1), seen: s.seen };
    broken.send(json!({ "cmd": "new_run" }));
    broken.seen
}

#[test]
fn the_document_has_each_name_of_the_protocol() {
    let doc = doc();
    let has = |name: &str| doc.contains(&format!("`{name}`"));
    let missing: Vec<&str> = Command::ALL
        .iter()
        .map(|c| c.name())
        .chain(EventKind::ALL.iter().map(|e| e.name()))
        .chain(Code::ALL.iter().map(|c| c.name()))
        .filter(|name| !has(name))
        .collect();
    assert!(missing.is_empty(), "PROTOCOL.md does not document: {missing:?}");
}

#[test]
fn the_document_has_each_field_of_the_responses() {
    let seen = scripted_session();
    let events: BTreeSet<String> = EventKind::ALL.iter().map(|e| e.name().to_string()).collect();
    let missed: Vec<_> = events.difference(&seen.events).collect();
    assert!(missed.is_empty(), "the scripted session makes no event of these kinds: {missed:?}");
    let doc = doc();
    let missing: Vec<&String> = seen.keys.iter().filter(|key| !doc.contains(&format!("`{key}`"))).collect();
    assert!(missing.is_empty(), "PROTOCOL.md does not document the fields {missing:?}");
}

/// The camp example of `PROTOCOL.md` comes from this session.
#[test]
fn the_camp_example_is_real() {
    let mut s =
        Script { session: Session::with_debug(Box::new(MemoryStorage::default()), 21, true), seen: Seen::default() };
    s.send(json!({ "cmd": "new_run" }));
    s.send(json!({ "cmd": "debug_set_gold", "gold": 20 }));
    for relic in ["bounty", "interest"] {
        s.send(json!({ "cmd": "debug_set_relic", "relic": relic, "on": true }));
    }
    s.army(units(&[("k", 4), ("r", 0)]), pieces(&[("k", 60), ("r", 8)]));
    let reply = s.send(json!({ "cmd": "move", "from": 0, "to": 8 }));
    let doc = doc();
    let text = reply["events"][1].to_string();
    assert!(doc.contains(r#""type":"capture","color":"b","id":30001,"kind":"r","square":8,"gold":7.5"#), "{text}");
    assert_eq!(reply["view"]["result"]["total"], json!(18));
    s.send(json!({ "cmd": "continue" }));
    s.send(json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "gold", "amount": 14 }, { "kind": "piece", "type": "n" }] }));
    let reply = s.send(json!({ "cmd": "take_reward", "index": 1 }));
    let cue = &reply["events"][0];
    assert_eq!(cue["gold_before"], json!(38));
    assert_eq!(cue["units"], json!([{ "id": 3, "kind": "n", "home": 3 }]));
}
