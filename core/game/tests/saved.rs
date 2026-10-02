//! Saved data: a run in camp reloads as it was, a run in the battle phase starts its battle
//! again, bad files load as fresh data, and unknown ids are dropped.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use chrogue_game::{FileStorage, MemoryStorage, Session, Storage};
use serde_json::{Value, json};

/// A storage that two sessions share, as two starts of the program share a directory.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<MemoryStorage>>);

impl Storage for Shared {
    fn load_meta(&mut self) -> Option<String> {
        self.0.borrow_mut().load_meta()
    }
    fn save_meta(&mut self, text: &str) -> Result<(), String> {
        self.0.borrow_mut().save_meta(text)
    }
    fn load_run(&mut self) -> Option<String> {
        self.0.borrow_mut().load_run()
    }
    fn save_run(&mut self, text: &str) -> Result<(), String> {
        self.0.borrow_mut().save_run(text)
    }
    fn clear_run(&mut self) -> Result<(), String> {
        self.0.borrow_mut().clear_run()
    }
}

fn send(session: &mut Session, request: Value) -> Value {
    let reply: Value = serde_json::from_str(&session.command(&request.to_string())).unwrap();
    assert_eq!(reply["ok"], json!(true), "{request} -> {reply}");
    reply
}

/// Wins the first battle at once and goes to the camp.
fn win_first_battle(session: &mut Session) -> Value {
    send(session, json!({ "cmd": "new_run" }));
    send(
        session,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 0 }, { "kind": "p", "home": 11 }] }),
    );
    send(
        session,
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "n", "square": 8 }] }),
    );
    send(session, json!({ "cmd": "move", "from": 0, "to": 8 }));
    let reply = send(session, json!({ "cmd": "continue" }));
    assert_eq!(reply["view"]["screen"], json!("camp"));
    reply["view"].clone()
}

#[test]
fn a_run_saved_in_camp_reloads_identically() {
    let storage = Shared::default();
    let mut first = Session::with_debug(Box::new(storage.clone()), 11, true);
    let camp = win_first_battle(&mut first);
    assert_eq!(camp["reward"]["state"], json!("open"));

    let mut second = Session::new(Box::new(storage.clone()), 99);
    assert_eq!(second.view()["run"], json!({ "floor": 2, "phase": "camp" }));
    let reloaded = send(&mut second, json!({ "cmd": "continue_run" }))["view"].clone();
    assert_eq!(reloaded, camp);

    // After a camp action the run is saved again. A reward that the player took is not on the
    // reloaded screen, as in the TypeScript game, where the reward shelf is data of the screen.
    let taken = send(&mut first, json!({ "cmd": "take_reward", "index": 0 }))["view"].clone();
    let mut third = Session::new(Box::new(storage), 5);
    let mut reloaded = send(&mut third, json!({ "cmd": "continue_run" }))["view"].clone();
    assert_eq!(reloaded["reward"], Value::Null);
    reloaded["reward"] = taken["reward"].clone();
    assert_eq!(reloaded, taken);
}

#[test]
fn a_run_saved_in_the_battle_phase_starts_that_battle_again() {
    let storage = Shared::default();
    let mut first = Session::with_debug(Box::new(storage.clone()), 3, true);
    let start = send(&mut first, json!({ "cmd": "new_run" }))["view"].clone();
    let m = start["moves"][0].clone();
    send(&mut first, json!({ "cmd": "move", "from": m["from"], "to": m["to"] }));
    send(&mut first, json!({ "cmd": "enemy_move" }));

    let mut second = Session::new(Box::new(storage), 4);
    assert_eq!(second.view()["run"], json!({ "floor": 1, "phase": "battle" }));
    let again = send(&mut second, json!({ "cmd": "continue_run" }))["view"].clone();
    assert_eq!(again, start);
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chrogue-game-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn corrupt_or_foreign_files_load_as_fresh_data() {
    let bad: [&[u8]; 12] = [
        b"",
        b"not json",
        b"{\"format\":\"chrogue.meta\",\"version\":1,\"data\"",
        b"[1,2,3]",
        b"null",
        b"{\"format\":\"something.else\",\"version\":1,\"data\":{\"crowns\":5}}",
        b"{\"format\":\"chrogue.meta\",\"version\":99,\"data\":{\"crowns\":5}}",
        b"{\"format\":\"chrogue.run\",\"version\":1,\"data\":{\"floor\":\"x\",\"gold\":-1}}",
        b"{\"format\":\"chrogue.run\",\"version\":1,\"data\":{\"floor\":1e999}}",
        b"{\"crowns\":5,\"best\":2}",
        &[0xff, 0xfe, 0x00, 0x80, 0x7b],
        b"{\"format\":\"chrogue.run\",\"version\":1,\"data\":{\"floor\":3,\"gold\":0,\"nextId\":9,\"army\":[{\"id\":1,\"type\":\"k\",\"home\":4},{\"id\":1,\"type\":\"q\",\"home\":5}],\"enemy\":{\"pieces\":[{\"type\":\"k\",\"square\":60}]}}}",
    ];
    for (i, bytes) in bad.iter().enumerate() {
        let dir = temp_dir(&format!("bad{i}"));
        fs::write(dir.join("meta.json"), bytes).unwrap();
        fs::write(dir.join("run.json"), bytes).unwrap();
        let mut session = Session::new(Box::new(FileStorage::new(&dir)), 1);
        let view = session.view();
        assert_eq!(view["meta"], json!({ "crowns": 0, "best": 0, "runs": 0 }), "file {i}");
        assert_eq!(view["can_continue"], json!(false), "file {i}");
        // The session works, and the next save replaces the bad file.
        send(&mut session, json!({ "cmd": "new_run" }));
        let saved: Value = serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(saved["format"], json!("chrogue.run"));
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn unknown_relic_and_upgrade_ids_are_dropped() {
    let dir = temp_dir("unknown");
    let meta = json!({ "format": "chrogue.meta", "version": 1,
        "data": { "crowns": 7, "best": 3, "runs": 2, "upgrades": { "pawn": 2, "removedUpgrade": 1, "scout": 1 } } });
    let run = json!({ "format": "chrogue.run", "version": 1, "data": {
        "floor": 3, "gold": 12, "nextId": 8, "phase": "camp",
        "army": [{ "id": 1, "type": "k", "home": 4 }, { "id": 2, "type": "r", "home": 0 }],
        "relics": ["bounty", "removedRelic", "interest"],
        "enemy": { "pieces": [{ "type": "k", "square": 60 }, { "type": "p", "square": 52 }], "traits": ["removedRelic", "sidestep"] },
        "draft": [{ "kind": "relic", "id": "removedRelic" }, { "kind": "gold", "amount": 16 }],
        "shop": [{ "kind": "piece", "type": "k" }, { "kind": "piece", "type": "n" }, { "kind": "unknownKind" }],
    } });
    fs::write(dir.join("meta.json"), meta.to_string()).unwrap();
    fs::write(dir.join("run.json"), run.to_string()).unwrap();
    let mut session = Session::with_debug(Box::new(FileStorage::new(&dir)), 1, true);
    assert_eq!(session.view()["meta"], json!({ "crowns": 7, "best": 3, "runs": 2 }));
    let camp = send(&mut session, json!({ "cmd": "continue_run" }))["view"].clone();
    let ids = |list: &Value| list.as_array().unwrap().iter().map(|r| r["id"].clone()).collect::<Vec<_>>();
    assert_eq!(ids(&camp["relics"]), vec![json!("bounty"), json!("interest")]);
    assert_eq!(ids(&camp["enemy"]["traits"]), vec![json!("sidestep")]);
    assert_eq!(camp["reward"]["offers"].as_array().unwrap().len(), 1);
    assert_eq!(camp["shop"]["offers"].as_array().unwrap().len(), 1);
    send(&mut session, json!({ "cmd": "to_title" }));
    let upgrades = send(&mut session, json!({ "cmd": "open_upgrades" }))["view"]["slots"].clone();
    let levels: Vec<_> = upgrades
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| !s.is_null())
        .map(|s| (s["id"].clone(), s["level"].clone()))
        .collect();
    assert_eq!(levels[0], (json!("pawn"), json!(2)));
    assert_eq!(levels[4], (json!("scout"), json!(1)));

    // A save writes the clean data.
    send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 8 }));
    let saved: Value = serde_json::from_str(&fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(saved["data"]["upgrades"], json!({ "pawn": 2, "scout": 1 }));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_game_saves_at_the_points_of_the_typescript_game() {
    let dir = temp_dir("points");
    let mut session = Session::with_debug(Box::new(FileStorage::new(&dir)), 2, true);
    let run_file = dir.join("run.json");
    let phase = || -> Option<Value> {
        let text = fs::read_to_string(&run_file).ok()?;
        Some(serde_json::from_str::<Value>(&text).unwrap()["data"]["phase"].clone())
    };
    assert_eq!(phase(), None);
    win_first_battle(&mut session);
    assert_eq!(phase(), Some(json!("camp")));
    send(&mut session, json!({ "cmd": "skip_reward" }));
    send(&mut session, json!({ "cmd": "start_battle" }));
    assert_eq!(phase(), Some(json!("battle")));
    let reply = send(&mut session, json!({ "cmd": "give_up" }));
    assert_eq!(reply["view"]["screen"], json!("over"));
    assert_eq!(phase(), None);
    let meta: Value = serde_json::from_str(&fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["data"]["runs"], json!(1));
    assert_eq!(meta["data"]["crowns"], json!(1));
    fs::remove_dir_all(&dir).unwrap();
}
