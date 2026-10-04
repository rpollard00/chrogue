//! Saved data: a run in camp reloads as it was, a run in the battle phase starts its battle
//! again, unknown ids are dropped, a file that the core cannot use is set aside and never
//! written over, boards that are not valid do not load, and the lock of a save directory.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use chrogue_game::save::{LOCK_FILE, OpenError};
use chrogue_game::{Doc, FileStorage, MemoryStorage, Session, Storage};
use serde_json::{Value, json};

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

fn send(session: &mut Session, request: Value) -> Value {
    let reply: Value = serde_json::from_str(&session.command(&request.to_string())).unwrap();
    assert_eq!(reply["ok"], json!(true), "{request} -> {reply}");
    reply
}

/// Starts a run and wins its first battle at once. The battle stays on the screen with its result.
fn win_first_move(session: &mut Session) -> Value {
    send(session, json!({ "cmd": "new_run" }));
    send(
        session,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 0 }, { "kind": "p", "home": 11 }] }),
    );
    send(
        session,
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "n", "square": 8 }] }),
    );
    send(session, json!({ "cmd": "move", "from": 0, "to": 8 }))
}

/// Wins the first battle at once and goes to the camp.
fn win_first_battle(session: &mut Session) -> Value {
    win_first_move(session);
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
    // reloaded screen: the reward shelf is data of the screen, not of the saved run.
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

#[test]
fn a_won_battle_is_saved_when_it_ends() {
    let storage = Shared::default();
    let mut first = Session::with_debug(Box::new(storage.clone()), 11, true);
    let end = win_first_move(&mut first)["view"].clone();
    assert_eq!((&end["screen"], &end["result"]["next"]), (&json!("battle"), &json!("camp")));

    // A client that starts again continues in the camp. It cannot play the battle a second time.
    let mut second = Session::new(Box::new(storage.clone()), 99);
    assert_eq!(second.view()["run"], json!({ "floor": 2, "phase": "camp" }));
    let reloaded = send(&mut second, json!({ "cmd": "continue_run" }))["view"].clone();
    let camp = send(&mut first, json!({ "cmd": "continue" }))["view"].clone();
    assert_eq!(reloaded, camp);

    // The title after the result has the run in the camp too.
    let mut third = Session::with_debug(Box::new(Shared::default()), 11, true);
    win_first_move(&mut third);
    let title = send(&mut third, json!({ "cmd": "to_title" }))["view"].clone();
    assert_eq!(title["run"], json!({ "floor": 2, "phase": "camp" }));
    assert_eq!(send(&mut third, json!({ "cmd": "continue_run" }))["view"], camp);
}

#[test]
fn a_lost_battle_ends_the_run_when_it_ends() {
    let storage = Shared::default();
    let mut first = Session::with_debug(Box::new(storage.clone()), 11, true);
    send(&mut first, json!({ "cmd": "new_run" }));
    send(
        &mut first,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "p", "home": 8 }] }),
    );
    send(
        &mut first,
        json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 60 }, { "kind": "r", "square": 56 }] }),
    );
    send(&mut first, json!({ "cmd": "move", "from": 4, "to": 12 }));
    // The rook captures the pawn, thus the king of the player is alone.
    let end = send(&mut first, json!({ "cmd": "debug_enemy_move", "from": 56, "to": 8 }))["view"].clone();
    assert_eq!((&end["screen"], &end["result"]["next"]), (&json!("battle"), &json!("lost")));

    // A client that starts again has no run to continue, and the permanent data has the run.
    let second = Session::new(Box::new(storage.clone()), 99);
    assert_eq!(second.view()["run"], Value::Null);
    assert_eq!(second.view()["meta"]["runs"], json!(1));

    // `continue` shows the summary. The run counts one time.
    let over = send(&mut first, json!({ "cmd": "continue" }));
    assert_eq!(over["view"]["screen"], json!("over"));
    assert!(over["events"].as_array().unwrap().iter().any(|e| e["type"] == "run_end"));
    assert_eq!(over["view"]["meta"]["runs"], json!(1));
    assert_eq!(Session::new(Box::new(storage), 5).view()["meta"]["runs"], json!(1));
}

#[test]
fn a_debug_change_after_the_result_keeps_the_result_on_the_screen() {
    let mut session = Session::with_debug(Box::new(Shared::default()), 11, true);
    win_first_move(&mut session);
    let changed = send(&mut session, json!({ "cmd": "debug_set_gold", "gold": 50 }))["view"].clone();
    assert_eq!(changed["screen"], json!("battle"));
    assert_ne!(changed["result"], Value::Null);
    assert_eq!(send(&mut session, json!({ "cmd": "continue" }))["view"]["gold"], json!(50));
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chrogue-game-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(dir: &Path) -> Box<FileStorage> {
    Box::new(FileStorage::open(dir).unwrap())
}

/// The names of the files in a directory that start with `prefix`, sorted.
fn files(dir: &Path, prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(prefix))
        .collect();
    names.sort();
    names
}

fn run_doc(data: Value) -> String {
    json!({ "format": "chrogue.run", "version": 1, "data": data }).to_string()
}

/// A valid run in camp with these army units and enemy pieces: (kind, square).
fn run_data(army: &[(&str, u64)], enemy: &[(&str, u64)], phase: &str) -> Value {
    let army: Vec<Value> =
        army.iter().enumerate().map(|(i, &(kind, home))| json!({ "id": i + 1, "type": kind, "home": home })).collect();
    let pieces: Vec<Value> = enemy.iter().map(|&(kind, square)| json!({ "type": kind, "square": square })).collect();
    json!({ "floor": 2, "gold": 3, "nextId": 20, "phase": phase, "army": army, "relics": [],
            "enemy": { "pieces": pieces, "traits": [] }, "draft": null, "shop": [] })
}

const ARMY: [(&str, u64); 3] = [("k", 4), ("r", 0), ("p", 12)];
const ENEMY: [(&str, u64); 2] = [("k", 60), ("p", 52)];

/// The bad files of the old test and more. Each one is set aside with its bytes, the first
/// response tells it, and the saves of the session never touch it.
#[test]
fn a_file_that_the_core_cannot_use_is_kept_aside_and_never_written_over() {
    // `F` is the format of the document under test.
    let deep = format!("{{\"format\":\"F\",\"version\":1,\"data\":{}{}", "[".repeat(300), "]".repeat(300));
    let bad: Vec<(&[u8], &str)> = vec![
        (b"", "unreadable"),
        (b"not json", "unreadable"),
        (b"{\"format\":\"F\",\"version\":1,\"data\":{\"crowns\":5", "unreadable"),
        (deep.as_bytes(), "unreadable"),
        (b"{\"format\":\"F\",\"version\":2,\"data\":{\"crowns\":5,\"new_field\":true}}", "newer_version"),
        (b"{\"format\":\"F\",\"version\":99,\"data\":null}", "newer_version"),
        (b"[1,2,3]", "invalid"),
        (b"null", "invalid"),
        (b"{\"format\":\"something.else\",\"version\":1,\"data\":{\"crowns\":5}}", "invalid"),
        (b"{\"format\":\"F\",\"version\":0,\"data\":{}}", "invalid"),
        (b"{\"format\":\"F\",\"version\":\"1\",\"data\":{}}", "invalid"),
        (b"{\"format\":\"F\",\"version\":1,\"data\":[]}", "invalid"),
        (b"{\"crowns\":5,\"best\":2}", "invalid"),
        (&[0xff, 0xfe, 0x00, 0x80, 0x7b], "unreadable"),
    ];
    for (i, &(template, reason)) in bad.iter().enumerate() {
        for doc in [Doc::Meta, Doc::Run] {
            let format = if doc == Doc::Meta { "chrogue.meta" } else { "chrogue.run" };
            let bytes = match std::str::from_utf8(template) {
                Ok(text) => text.replace("\"F\"", &format!("\"{format}\"")).into_bytes(),
                Err(_) => template.to_vec(),
            };
            let dir = temp_dir(&format!("bad{i}-{}", doc.name()));
            fs::write(dir.join(doc.file_name()), &bytes).unwrap();
            let mut session = Session::with_debug(open(&dir), 1, true);
            let view = session.view();
            assert_eq!(view["meta"], json!({ "crowns": 0, "best": 0, "runs": 0 }), "file {i}");
            assert_eq!(view["can_continue"], json!(false), "file {i}");
            let kept = files(&dir, &format!("{}.bad-", doc.file_name()));
            assert_eq!(kept.len(), 1, "file {i} {}: {kept:?}", doc.name());
            assert!(!dir.join(doc.file_name()).exists(), "file {i}");

            // The first response tells it.
            let first = send(&mut session, json!({ "cmd": "view" }));
            let problem = &first["events"][0];
            assert_eq!(problem["type"], json!("save_problem"), "file {i}");
            assert_eq!(problem["what"], json!(doc.name()));
            assert_eq!(problem["reason"], json!(reason), "file {i} {}: {problem}", doc.name());
            assert_eq!(problem["kept"], json!(kept[0]));
            assert!(problem["message"].as_str().is_some_and(|m| !m.is_empty()));
            assert_eq!(send(&mut session, json!({ "cmd": "view" }))["events"], json!([]), "only the first response");

            // The session saves both documents; the kept file keeps its bytes.
            send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 3 }));
            send(&mut session, json!({ "cmd": "new_run" }));
            send(&mut session, json!({ "cmd": "give_up" }));
            send(&mut session, json!({ "cmd": "new_run" }));
            assert_eq!(fs::read(dir.join(&kept[0])).unwrap(), bytes, "file {i}");
            assert_eq!(
                problem["kept"].as_str().map(|k| k.starts_with(&format!("{}.bad-", doc.file_name()))),
                Some(true)
            );
            assert_eq!(files(&dir, &format!("{}.bad-", doc.file_name())), kept);
            for doc in [Doc::Meta, Doc::Run] {
                let saved: Value =
                    serde_json::from_str(&fs::read_to_string(dir.join(doc.file_name())).unwrap()).unwrap();
                assert_eq!(saved["version"], json!(1));
            }
            assert_eq!(files(&dir, "").iter().filter(|n| n.ends_with(".tmp")).count(), 0);
            drop(session);
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}

/// A second bad file in the same second gets its own name.
#[test]
fn two_bad_files_get_two_names() {
    let dir = temp_dir("twice");
    for _ in 0..2 {
        fs::write(dir.join("meta.json"), b"{").unwrap();
        let mut session = Session::new(open(&dir), 1);
        send(&mut session, json!({ "cmd": "view" }));
    }
    let kept = files(&dir, "meta.json.bad-");
    assert_eq!(kept.len(), 2, "{kept:?}");
    fs::remove_dir_all(&dir).unwrap();
}

/// A shared storage whose bad files cannot move: the session must not write or remove them.
struct Stuck(Shared);

impl Storage for Stuck {
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String> {
        self.0.load(doc)
    }
    fn save(&mut self, doc: Doc, text: &str) -> Result<(), String> {
        self.0.save(doc, text)
    }
    fn remove(&mut self, doc: Doc) -> Result<(), String> {
        self.0.remove(doc)
    }
    fn set_aside(&mut self, _: Doc) -> Result<Option<String>, String> {
        Err("read-only directory".into())
    }
}

#[test]
fn a_bad_file_that_cannot_move_is_never_written() {
    let newer = r#"{"format":"chrogue.meta","version":7,"data":{"crowns":5}}"#.to_string();
    let run = r#"{"format":"chrogue.run","version":1,"data":{"floor":"#.to_string();
    let shared = Shared(Rc::new(RefCell::new(MemoryStorage { meta: Some(newer.clone()), run: Some(run.clone()) })));
    let mut session = Session::with_debug(Box::new(Stuck(shared.clone())), 1, true);
    let reply = send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 9 }));
    let events = reply["events"].as_array().unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["save_problem", "save_problem", "debug_changed", "save_failed"]);
    assert_eq!(events[0]["kept"], Value::Null);
    assert_eq!(events[0]["reason"], json!("newer_version"));
    assert_eq!(events[1]["reason"], json!("unreadable"));
    assert_eq!(events[3]["what"], json!("meta"));
    send(&mut session, json!({ "cmd": "new_run" }));
    send(&mut session, json!({ "cmd": "give_up" }));
    let stored = shared.0.borrow();
    assert_eq!(stored.meta, Some(newer));
    assert_eq!(stored.run, Some(run));
}

#[test]
fn boards_that_are_not_valid_do_not_load() {
    let cases: Vec<(&str, Value)> = vec![
        ("two enemy pieces on one square", run_data(&ARMY, &[("k", 60), ("q", 60)], "battle")),
        ("two enemy pieces on one square in camp", run_data(&ARMY, &[("k", 60), ("p", 52), ("n", 52)], "camp")),
        ("an enemy piece on a unit", run_data(&ARMY, &[("k", 60), ("q", 4)], "camp")),
        ("two units on one home", run_data(&[("k", 4), ("r", 4)], &ENEMY, "camp")),
        ("no enemy king", run_data(&ARMY, &[("q", 59)], "battle")),
        ("two enemy kings", run_data(&ARMY, &[("k", 60), ("k", 62)], "battle")),
        ("no king in the army", run_data(&[("q", 3), ("r", 0)], &ENEMY, "camp")),
        ("gold above 2^53", {
            let mut data = run_data(&ARMY, &ENEMY, "camp");
            data["gold"] = json!(u64::MAX);
            data
        }),
        ("a floor of 1e300", {
            let mut data = run_data(&ARMY, &ENEMY, "camp");
            data["floor"] = json!(1e300);
            data
        }),
        ("enemy kinds with no king", {
            let mut data = run_data(&ARMY, &ENEMY, "camp");
            data["enemy"] = json!({ "kinds": ["q", "p"], "traits": [] });
            data
        }),
        ("enemy kinds with 9 pawns", {
            let mut data = run_data(&ARMY, &ENEMY, "camp");
            data["enemy"] = json!({ "kinds": ["k", "p", "p", "p", "p", "p", "p", "p", "p", "p"], "traits": [] });
            data
        }),
        ("an enemy kind that is not a piece", {
            let mut data = run_data(&ARMY, &ENEMY, "camp");
            data["enemy"] = json!({ "kinds": ["k", "x"], "traits": [] });
            data
        }),
        ("65 enemy pieces", {
            let pieces: Vec<(&str, u64)> = (0..65).map(|s| (if s == 63 { "k" } else { "p" }, s % 64)).collect();
            run_data(&[("k", 4)], &pieces, "camp")
        }),
    ];
    for (name, data) in cases {
        let storage = MemoryStorage { meta: None, run: Some(run_doc(data)) };
        let mut session = Session::new(Box::new(storage), 1);
        assert_eq!(session.view()["can_continue"], json!(false), "{name}");
        let problem = send(&mut session, json!({ "cmd": "view" }))["events"][0].clone();
        assert_eq!((problem["what"].clone(), problem["reason"].clone()), (json!("run"), json!("invalid")), "{name}");
    }

    // The same boards that are valid load: an enemy with kinds, which gets its squares in the
    // battle, and an enemy with set squares. With set squares, a start with the enemy king in
    // check is valid in camp and in a battle.
    let mut kinds = run_data(&ARMY, &ENEMY, "camp");
    kinds["enemy"] = json!({ "kinds": ["k", "r", "p"], "traits": [] });
    for data in [
        kinds,
        run_data(&ARMY, &ENEMY, "battle"),
        run_data(&[("k", 4), ("p", 13)], &[("k", 20)], "camp"),
        run_data(&[("k", 4), ("p", 13)], &[("k", 20)], "battle"),
        run_data(&[("k", 12)], &[("k", 5)], "battle"),
    ] {
        let storage = MemoryStorage { meta: None, run: Some(run_doc(data.clone())) };
        let mut session = Session::new(Box::new(storage), 1);
        assert_eq!(session.view()["can_continue"], json!(true), "{data}");
        assert_eq!(send(&mut session, json!({ "cmd": "view" }))["events"], json!([]));
    }

    // A meta with counts above 2^53 loads those fields as 0, as a field that is not a count.
    let meta = json!({ "format": "chrogue.meta", "version": 1,
        "data": { "crowns": u64::MAX, "best": 3, "runs": 9_007_199_254_740_992u64, "upgrades": { "pawn": u64::MAX, "scout": 1 } } });
    let mut session = Session::new(Box::new(MemoryStorage { meta: Some(meta.to_string()), run: None }), 1);
    assert_eq!(session.view()["meta"], json!({ "crowns": 0, "best": 3, "runs": 0 }));
    let slots = send(&mut session, json!({ "cmd": "open_upgrades" }))["view"]["slots"].clone();
    assert_eq!(slots[0]["level"], json!(0));
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
    let mut session = Session::with_debug(open(&dir), 1, true);
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
    drop(session);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_game_saves_the_run_and_the_meta_at_each_change_of_the_phase() {
    let dir = temp_dir("points");
    let mut session = Session::with_debug(open(&dir), 2, true);
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
    drop(session);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_save_directory_has_one_core_at_a_time() {
    let dir = temp_dir("lock");
    let first = FileStorage::open(&dir).unwrap();
    assert_eq!(first.recovered_lock(), None);
    let pid = std::process::id().to_string();
    assert_eq!(fs::read_to_string(dir.join(LOCK_FILE)).unwrap().trim(), pid);
    match FileStorage::open(&dir) {
        Err(OpenError::Locked { holder, .. }) => assert_eq!(holder.as_deref(), Some(pid.as_str())),
        other => panic!("a second open got {other:?}"),
    }
    // A clean end empties the lock file; the next open finds no stale lock.
    drop(first);
    assert_eq!(fs::read_to_string(dir.join(LOCK_FILE)).unwrap(), "");
    let second = FileStorage::open(&dir).unwrap();
    assert_eq!(second.recovered_lock(), None);
    drop(second);

    // A lock file with a PID that no process locks: a core that crashed. The next open takes
    // it over. A temporary file of a save that the crash stopped goes away.
    fs::write(dir.join(LOCK_FILE), "4194999\n").unwrap();
    fs::write(dir.join("meta.json.4194999-3.tmp"), "{\"format\":").unwrap();
    let third = FileStorage::open(&dir).unwrap();
    assert_eq!(third.recovered_lock(), Some("4194999"));
    assert_eq!(fs::read_to_string(dir.join(LOCK_FILE)).unwrap().trim(), pid);
    assert!(files(&dir, "").iter().all(|n| !n.ends_with(".tmp")));
    drop(third);
    fs::remove_dir_all(&dir).unwrap();
}
