//! The tuning of a session: its defaults and limits, the enemy armies that it gives, the AI
//! level of a floor, the fixed seed of new runs, and the debug commands that change it.

use std::cell::RefCell;
use std::rc::Rc;

use chrogue_game::battle::Battle;
use chrogue_game::chess::{self, Kind, Square};
use chrogue_game::content::{ENEMY_KINDS, FLOORS, RECRUIT_KINDS, RELIC_SLOTS, TRAITS_MAX, gold_value};
use chrogue_game::random::{Dice, Stream};
use chrogue_game::run::{Enemy, Meta, Run, SEED_MAX, generate_enemy};
use chrogue_game::tuning::{BUDGET_MAX, Field, Target, Tuning, WEIGHT_MAX};
use chrogue_game::{Doc, MemoryStorage, Session, Storage};
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

fn debug_session(seed: u64) -> Session {
    Session::with_debug(Box::new(MemoryStorage::default()), seed, true)
}

fn ask(session: &mut Session, request: Value) -> Value {
    serde_json::from_str(&session.command(&request.to_string())).unwrap()
}

fn send(session: &mut Session, request: Value) -> Value {
    let reply = ask(session, request.clone());
    assert_eq!(reply["ok"], json!(true), "{request} -> {reply}");
    reply
}

fn debug_state(session: &mut Session) -> Value {
    send(session, json!({ "cmd": "debug_state" }))["data"]["debug"].clone()
}

fn event_types(reply: &Value) -> Vec<&str> {
    reply["events"].as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).collect()
}

/// A tuning with these fields of one floor or of one kind.
fn tuned(target: Target, values: &[(Field, f64)]) -> Tuning {
    let mut tuning = Tuning::default();
    tuning.tune(target, values).unwrap();
    tuning
}

fn count(enemy: &Enemy, kind: Kind) -> usize {
    enemy.pieces.kinds().iter().filter(|&&k| k == kind).count()
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

#[test]
fn the_default_tuning_is_the_content() {
    let tuning = Tuning::default();
    assert_eq!((tuning.seed, tuning.barred.len(), tuning.tuned()), (None, 0, false));
    assert_eq!(tuning.relic_slots, RELIC_SLOTS);
    assert_eq!(tuning.floors.len(), FLOORS.len());
    for (i, (floor, def)) in tuning.floors.iter().zip(&FLOORS).enumerate() {
        assert_eq!((floor.level, floor.budget, floor.traits), (i + 1, def.budget, def.traits));
    }
    for ((kind, tuned), def) in RECRUIT_KINDS.iter().zip(&tuning.kinds).zip(&ENEMY_KINDS) {
        assert_eq!(*kind, def.kind);
        assert_eq!((tuned.cap, tuned.weight, tuned.min_floor), (def.cap, def.weight, def.min_floor));
    }

    // The limits contain the content.
    assert_eq!(BUDGET_MAX, ENEMY_KINDS.iter().map(|k| k.cap * gold_value(k.kind)).sum::<u32>());
    assert!(FLOORS.iter().all(|f| f.traits <= TRAITS_MAX && f.budget <= BUDGET_MAX));
    assert!(ENEMY_KINDS.iter().all(|k| k.weight <= WEIGHT_MAX && (1..=FLOORS.len()).contains(&k.min_floor)));
    assert_eq!(chess::LEVELS, FLOORS.len());

    let state = debug_state(&mut debug_session(1));
    let floors: Vec<Value> = FLOORS
        .iter()
        .enumerate()
        .map(|(i, f)| {
            json!({ "number": i + 1, "name": f.name, "level": i + 1, "level_name": chess::level_name(i + 1),
                    "budget": f.budget, "traits": f.traits })
        })
        .collect();
    let kinds: Vec<Value> = ENEMY_KINDS
        .iter()
        .map(|k| {
            json!({ "kind": chess::kind_letter(k.kind).to_string(), "cap": k.cap, "cap_max": k.cap,
                    "weight": k.weight, "min_floor": k.min_floor })
        })
        .collect();
    assert_eq!(state["floors"], json!(floors));
    assert_eq!(state["kinds"], json!(kinds));
    assert_eq!(
        state["limits"],
        json!({ "seed": SEED_MAX, "level": 8, "budget": 39, "traits": 2, "weight": 9.0, "relics": 10 })
    );
    assert_eq!((&state["tuned"], &state["seed"], &state["barred"]), (&json!(false), &Value::Null, &json!([])));
    assert_eq!(state["relic_slots"], json!(RELIC_SLOTS));
}

#[test]
fn a_budget_of_0_gives_an_enemy_of_only_a_king() {
    for floor in 1..=FLOORS.len() {
        let tuning = tuned(Target::Floor(floor), &[(Field::Budget, 0.0), (Field::Traits, 0.0)]);
        for seed in 0..20 {
            let enemy = generate_enemy(seed, floor, &tuning);
            assert_eq!(enemy.pieces.kinds(), vec![Kind::King]);
            assert_eq!(enemy.traits, vec![]);
        }
    }
}

#[test]
fn the_budget_and_the_traits_of_a_floor_come_from_the_tuning() {
    let tuning = tuned(Target::Floor(1), &[(Field::Budget, BUDGET_MAX as f64), (Field::Traits, 2.0)]);
    for seed in 0..50 {
        let enemy = generate_enemy(seed, 1, &tuning);
        // Floor 1 has no queen, thus the pieces have a value of 30.
        assert_eq!(enemy.pieces.kinds().into_iter().map(gold_value).sum::<u32>(), 30);
        assert_eq!(enemy.traits.len(), 2);
        assert_eq!(generate_enemy(seed, 2, &tuning), generate_enemy(seed, 2, &Tuning::default()));
    }
}

#[test]
fn a_cap_of_0_or_a_weight_of_0_removes_the_kind() {
    for field in [Field::Cap, Field::Weight] {
        let tuning = tuned(Target::Kind(Kind::Pawn), &[(field, 0.0)]);
        for seed in 0..50 {
            for floor in 1..=FLOORS.len() {
                assert_eq!(count(&generate_enemy(seed, floor, &tuning), Kind::Pawn), 0);
            }
        }
    }
    assert!((0..50).all(|seed| count(&generate_enemy(seed, 8, &Tuning::default()), Kind::Pawn) > 0));
    let one = tuned(Target::Kind(Kind::Pawn), &[(Field::Cap, 1.0)]);
    assert!((0..50).all(|seed| count(&generate_enemy(seed, 8, &one), Kind::Pawn) == 1));
}

#[test]
fn min_floor_moves_the_first_floor_of_the_queen() {
    let queens = |tuning: &Tuning, floor: usize| -> usize {
        (0..200).map(|seed| count(&generate_enemy(seed, floor, tuning), Kind::Queen)).sum()
    };
    let first = |tuning: &Tuning| (1..=FLOORS.len()).find(|&floor| queens(tuning, floor) > 0);
    assert_eq!(first(&Tuning::default()), Some(5));
    assert_eq!(first(&tuned(Target::Kind(Kind::Queen), &[(Field::MinFloor, 3.0)])), Some(3));
    assert_eq!(first(&tuned(Target::Kind(Kind::Queen), &[(Field::MinFloor, 8.0)])), Some(8));
}

#[test]
fn debug_tune_refuses_a_value_above_its_limit_and_changes_nothing() {
    let mut session = debug_session(1);
    let view = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    let state = debug_state(&mut session);
    let limit = |name: &str| state["limits"][name].as_f64().unwrap();

    // Each field with its largest value: the target (a floor or a kind), the field, the value.
    let mut largest = vec![
        ("floor", json!(8), "level", limit("level")),
        ("floor", json!(8), "budget", limit("budget")),
        ("floor", json!(8), "traits", limit("traits")),
    ];
    for kind in state["kinds"].as_array().unwrap() {
        largest.push(("kind", kind["kind"].clone(), "cap", kind["cap_max"].as_f64().unwrap()));
        largest.push(("kind", kind["kind"].clone(), "weight", limit("weight")));
        largest.push(("kind", kind["kind"].clone(), "min_floor", FLOORS.len() as f64));
    }
    let request = |target: &str, id: &Value, field: &str, value: f64| {
        let mut request = json!({ "cmd": "debug_tune" });
        request[target] = id.clone();
        request[field] = json!(value);
        request
    };
    let mut refused: Vec<Value> = largest
        .iter()
        .map(|(target, id, field, max)| request(target, id, field, max + if *field == "weight" { 0.5 } else { 1.0 }))
        .collect();
    refused.extend([
        json!({ "cmd": "debug_tune" }),
        json!({ "cmd": "debug_tune", "floor": 1 }),
        json!({ "cmd": "debug_tune", "kind": "p" }),
        json!({ "cmd": "debug_tune", "floor": 1, "kind": "p", "budget": 3, "cap": 1 }),
        json!({ "cmd": "debug_tune", "reset": true, "floor": 1, "budget": 3 }),
        json!({ "cmd": "debug_tune", "reset": false }),
        json!({ "cmd": "debug_tune", "reset": 1 }),
        json!({ "cmd": "debug_tune", "floor": 0, "budget": 3 }),
        json!({ "cmd": "debug_tune", "floor": 9, "budget": 3 }),
        json!({ "cmd": "debug_tune", "kind": "k", "cap": 1 }),
        json!({ "cmd": "debug_tune", "floor": 1, "cap": 1 }),
        json!({ "cmd": "debug_tune", "kind": "p", "budget": 3 }),
        json!({ "cmd": "debug_tune", "floor": 1, "level": 0 }),
        json!({ "cmd": "debug_tune", "kind": "q", "min_floor": 0 }),
        json!({ "cmd": "debug_tune", "floor": 1, "budget": -1 }),
        json!({ "cmd": "debug_tune", "floor": 1, "budget": 2.5 }),
        json!({ "cmd": "debug_tune", "floor": 1, "budget": "5" }),
        json!({ "cmd": "debug_tune", "kind": "p", "weight": -0.5 }),
        // One value in its range and one value above its limit.
        json!({ "cmd": "debug_tune", "floor": 1, "budget": 10, "traits": 3 }),
        json!({ "cmd": "debug_tune", "kind": "n", "weight": 3, "cap": 3 }),
        json!({ "cmd": "debug_set_seed", "seed": SEED_MAX + 1 }),
        json!({ "cmd": "debug_set_seed", "seed": -1 }),
        json!({ "cmd": "debug_set_seed", "seed": "7" }),
        json!({ "cmd": "debug_set_seed" }),
    ]);
    for request in &refused {
        let reply = ask(&mut session, request.clone());
        assert_eq!(reply["error"]["code"], json!("bad_args"), "{request} -> {reply}");
        assert_eq!(reply["view"], view, "{request}");
        assert_eq!(reply.get("data"), None, "{request}");
        assert_eq!(debug_state(&mut session), state, "{request}");
    }

    // The largest value of each field is in its range.
    for (target, id, field, max) in &largest {
        let reply = send(&mut session, request(target, id, field, *max));
        let (list, key) = if *target == "floor" { ("floors", "number") } else { ("kinds", "kind") };
        let item = reply["data"]["debug"][list].as_array().unwrap().iter().find(|item| item[key] == *id);
        assert_eq!(item.unwrap()[*field].as_f64(), Some(*max), "{target} {id} {field}");
    }
}

#[test]
fn a_tune_of_the_budget_in_the_camp_gives_a_new_enemy_and_saves_the_run() {
    let storage = Shared::default();
    let mut session = Session::with_debug(Box::new(storage.clone()), 11, true);
    win_first_move(&mut session);
    let camp = send(&mut session, json!({ "cmd": "continue" }))["view"].clone();
    assert!(camp["enemy"]["kinds"].as_array().unwrap().len() > 1);

    let reply = send(&mut session, json!({ "cmd": "debug_tune", "floor": 2, "budget": 0 }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "tuning" }]));
    assert_eq!(reply["view"]["enemy"]["kinds"], json!(["k"]));
    assert_eq!(reply["data"]["debug"]["tuned"], json!(true));
    assert_eq!(reply["data"]["debug"]["floors"][1]["budget"], json!(0));
    let mut expected = camp.clone();
    expected["enemy"]["kinds"] = json!(["k"]);
    assert_eq!(reply["view"], expected);

    // A second core on the same storage has the run with the new enemy.
    let mut second = Session::new(Box::new(storage), 99);
    assert_eq!(send(&mut second, json!({ "cmd": "continue_run" }))["view"]["enemy"], expected["enemy"]);

    // The tuning stays for the next run of the session.
    send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "budget": 0 }));
    send(&mut session, json!({ "cmd": "to_title" }));
    let battle = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    let black: Vec<&Value> = battle["pieces"].as_array().unwrap().iter().filter(|p| p["color"] == "b").collect();
    assert_eq!(black.len(), 1);
}

#[test]
fn a_tune_of_the_budget_in_a_battle_with_no_result_starts_the_battle_again() {
    let mut session = debug_session(4);
    let start = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    let m = start["moves"][0].clone();
    send(&mut session, json!({ "cmd": "move", "from": m["from"], "to": m["to"] }));
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "budget": 39 }));
    assert_eq!(event_types(&reply), ["debug_changed", "screen", "battle_start"]);
    let view = &reply["view"];
    assert_eq!((&view["phase"], &view["last"]), (&json!("player"), &Value::Null));
    // Floor 1 has no queen, thus the full enemy army has 15 pieces.
    let enemy = |view: &Value| view["pieces"].as_array().unwrap().iter().filter(|p| p["color"] == "b").count();
    assert!(enemy(&start) <= 6);
    assert_eq!(enemy(view), 15);

    // After the result, the change goes to the run after the battle, and the screen stays.
    let mut session = debug_session(4);
    let end = win_first_move(&mut session)["view"].clone();
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "floor": 2, "budget": 0 }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!(reply["view"], end);
    let camp = send(&mut session, json!({ "cmd": "continue" }))["view"].clone();
    assert_eq!(camp["enemy"]["kinds"], json!(["k"]));

    // With no run, only the tuning changes.
    let mut session = debug_session(4);
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "kind": "q", "min_floor": 1, "weight": 9 }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!(reply["view"]["screen"], json!("title"));
    assert_eq!(reply["data"]["debug"]["run"], Value::Null);
    assert_eq!(
        reply["data"]["debug"]["kinds"][4],
        json!({ "kind": "q", "cap": 1, "cap_max": 1, "weight": 9.0, "min_floor": 1 })
    );
}

#[test]
fn a_tune_of_only_the_level_does_not_start_the_battle_again() {
    let mut session = debug_session(4);
    let start = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    let m = start["moves"][0].clone();
    send(&mut session, json!({ "cmd": "move", "from": m["from"], "to": m["to"] }));
    let before = send(&mut session, json!({ "cmd": "enemy_move" }))["view"].clone();
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "level": 4 }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "tuning" }]));
    assert_eq!(reply["view"], before);
    let floor = &reply["data"]["debug"]["floors"][0];
    assert_eq!((&floor["level"], &floor["level_name"]), (&json!(4), &json!("The Warden")));
    assert_eq!(reply["data"]["debug"]["tuned"], json!(true));

    // A value that is the value of the tuning changes nothing, thus the battle continues too.
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "budget": FLOORS[0].budget }));
    assert_eq!(reply["view"], before);

    // A reset of a tuning that has the default armies does not start the battle again.
    let reply = send(&mut session, json!({ "cmd": "debug_tune", "reset": true }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!(reply["view"], before);
    assert_eq!(reply["data"]["debug"]["tuned"], json!(false));
}

#[test]
fn a_reset_gives_the_default_armies_and_keeps_the_seed_the_barred_relics_and_the_relic_slots() {
    let mut session = debug_session(2);
    let fresh = debug_state(&mut session);
    send(&mut session, json!({ "cmd": "debug_set_seed", "seed": 31 }));
    send(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": 6 }));
    send(&mut session, json!({ "cmd": "debug_bar_relic", "relic": "interest", "barred": true }));
    send(&mut session, json!({ "cmd": "debug_bar_relic", "relic": "bounty", "barred": true }));
    let start = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "budget": 20, "traits": 1, "level": 3 }));
    let tuned = send(&mut session, json!({ "cmd": "debug_tune", "kind": "r", "cap": 0 }));
    assert_ne!(tuned["view"]["pieces"], start["pieces"]);
    assert_eq!(tuned["data"]["debug"]["tuned"], json!(true));

    let reply = send(&mut session, json!({ "cmd": "debug_tune", "reset": true }));
    assert_eq!(event_types(&reply), ["debug_changed", "screen", "battle_start"]);
    assert_eq!(reply["view"], start);
    let state = &reply["data"]["debug"];
    assert_eq!(
        (&state["floors"], &state["kinds"], &state["tuned"]),
        (&fresh["floors"], &fresh["kinds"], &json!(false))
    );
    assert_eq!((&state["seed"], &state["barred"]), (&json!(31), &json!(["bounty", "interest"])));
    assert_eq!((&state["relic_slots"], &state["run"]["relic_slots"]), (&json!(6), &json!(6)));
}

#[test]
fn the_ai_of_a_floor_plays_at_the_tuned_level() {
    const SEED: u64 = 5;
    const LEVEL: usize = 5;
    let mut session = debug_session(1);
    send(&mut session, json!({ "cmd": "debug_set_seed", "seed": SEED }));
    send(&mut session, json!({ "cmd": "debug_tune", "floor": 1, "level": LEVEL }));
    let mut view = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();

    // The same battle with no session. The seed of each AI move comes from the seed of the run,
    // the floor, and the number of moves of the battle.
    let run = Run::new(&Meta::default(), SEED, &Tuning::default());
    let mut battle = Battle::new(&run).unwrap();
    let mut differs = false;
    for _ in 0..6 {
        let m = view["moves"][0].clone();
        let (from, to) = (m["from"].as_u64().unwrap() as Square, m["to"].as_u64().unwrap() as Square);
        let reply = send(&mut session, json!({ "cmd": "move", "from": from, "to": to }));
        let mv = battle.find_move(from, to, None).expect("The two battles differ");
        battle.play(&run, mv);
        if reply["view"]["phase"] != json!("enemy") {
            break;
        }
        let seed = Dice::stream(SEED, Stream::Ai, 1, u64::from(battle.plies)).seed();
        let tuned = chess::ai_move(&mut battle.state, LEVEL, seed).expect("The enemy has no move");
        differs |= chess::ai_move(&mut battle.state, 1, seed) != Some(tuned);
        let reply = send(&mut session, json!({ "cmd": "enemy_move" }));
        let played = &reply["events"][0];
        assert_eq!((&played["from"], &played["to"]), (&json!(tuned.from), &json!(tuned.to)));
        // `debug_ai_move` with no level plays at the level of the floor too.
        assert_eq!(battle.ai_move(&run, LEVEL), Some(tuned));
        battle.play(&run, tuned);
        view = reply["view"].clone();
        if view["phase"] != json!("player") {
            break;
        }
    }
    assert!(battle.plies >= 6, "the battle ended after {} moves", battle.plies);
    assert!(differs, "level 1 and level {LEVEL} played the same moves");
}

#[test]
fn a_fixed_seed_gives_the_same_run_each_time() {
    let mut first = debug_session(1);
    let reply = send(&mut first, json!({ "cmd": "debug_set_seed", "seed": 424_242 }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "seed" }]));
    assert_eq!(reply["data"]["debug"]["seed"], json!(424_242));
    let battle = send(&mut first, json!({ "cmd": "new_run" }))["view"].clone();
    assert_eq!(debug_state(&mut first)["run"]["seed"], json!(424_242));
    send(&mut first, json!({ "cmd": "give_up" }));
    assert_eq!(send(&mut first, json!({ "cmd": "new_run" }))["view"], battle);

    // A session with a different session seed gets the same run. The camp does not depend on
    // the number of moves of the battle.
    let camp = |session: &mut Session, wait: bool| -> Value {
        send(
            session,
            json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "r", "home": 0 }, { "kind": "p", "home": 12 }] }),
        );
        // The enemy pawn on a2 cannot move, thus the AI moves the king.
        send(
            session,
            json!({ "cmd": "debug_set_enemy", "pieces": [{ "kind": "k", "square": 63 }, { "kind": "p", "square": 8 }] }),
        );
        if wait {
            send(session, json!({ "cmd": "move", "from": 12, "to": 20 }));
            send(session, json!({ "cmd": "enemy_move" }));
        }
        send(session, json!({ "cmd": "move", "from": 0, "to": 8 }));
        send(session, json!({ "cmd": "continue" }))["view"].clone()
    };
    let mut second = debug_session(2);
    send(&mut second, json!({ "cmd": "debug_set_seed", "seed": 424_242 }));
    assert_eq!(send(&mut second, json!({ "cmd": "new_run" }))["view"], battle);
    assert_eq!(camp(&mut first, false), camp(&mut second, true));

    // The seed of the run in progress stays when the fixed seed changes. `null` clears the
    // fixed seed, thus the dice of the session make the seed of each new run.
    let reply = send(&mut first, json!({ "cmd": "debug_set_seed", "seed": null }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!(reply["view"]["screen"], json!("camp"));
    assert_eq!(
        (&reply["data"]["debug"]["seed"], &reply["data"]["debug"]["run"]["seed"]),
        (&Value::Null, &json!(424_242))
    );
    let mut seeds = Vec::new();
    for _ in 0..3 {
        send(&mut first, json!({ "cmd": "to_title" }));
        send(&mut first, json!({ "cmd": "new_run" }));
        seeds.push(debug_state(&mut first)["run"]["seed"].as_u64().unwrap());
    }
    assert!(seeds.iter().all(|&seed| seed <= SEED_MAX && seed != 424_242), "{seeds:?}");
    assert!(seeds[0] != seeds[1] && seeds[1] != seeds[2], "{seeds:?}");
}

#[test]
fn debug_set_relic_slots_sets_the_slots_of_the_tuning_and_of_each_new_run() {
    let mut session = debug_session(1);
    // With no run, only the tuning changes.
    let reply = send(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": 6 }));
    assert_eq!(reply["events"], json!([{ "type": "debug_changed", "what": "relic_slots" }]));
    assert_eq!(reply["view"]["screen"], json!("title"));
    let state = &reply["data"]["debug"];
    assert_eq!((&state["relic_slots"], &state["run"], &state["tuned"]), (&json!(6), &Value::Null, &json!(false)));

    let battle = send(&mut session, json!({ "cmd": "new_run" }))["view"].clone();
    assert_eq!(battle["relic_slots"], json!(6));
    assert_eq!(debug_state(&mut session)["run"]["relic_slots"], json!(6));

    for slots in [json!(11), json!(-1), json!(2.5), json!("4"), Value::Null] {
        let reply = ask(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": slots }));
        assert_eq!((&reply["error"]["code"], &reply["view"]), (&json!("bad_args"), &battle), "{slots}");
    }
    assert_eq!(debug_state(&mut session)["relic_slots"], json!(6));
    for slots in [0, 10] {
        let reply = send(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": slots }));
        assert_eq!(reply["data"]["debug"]["relic_slots"], json!(slots));
    }
}

#[test]
fn debug_set_relic_slots_changes_the_run_in_progress_and_does_not_start_the_battle_again() {
    let mut session = debug_session(1);
    send(&mut session, json!({ "cmd": "new_run" }));
    send(
        &mut session,
        json!({ "cmd": "debug_set_army", "units": [{ "kind": "k", "home": 4 }, { "kind": "p", "home": 12 }] }),
    );
    let moved = send(&mut session, json!({ "cmd": "move", "from": 12, "to": 20 }))["view"].clone();
    let reply = send(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": 2 }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!((&reply["view"]["relic_slots"], &reply["view"]["pieces"]), (&json!(2), &moved["pieces"]));
    assert_eq!((&reply["view"]["last"], &reply["view"]["phase"]), (&moved["last"], &json!("enemy")));
    let state = &reply["data"]["debug"];
    assert_eq!((&state["relic_slots"], &state["run"]["relic_slots"]), (&json!(2), &json!(2)));
    // The core saves the run with its slots.
    assert_eq!(send(&mut session, json!({ "cmd": "view", "run": true }))["data"]["run"]["slots"], json!(2));
}

#[test]
fn fewer_slots_than_relics_remove_no_relic_and_block_each_relic_offer() {
    let mut session = debug_session(1);
    win_first_move(&mut session);
    send(&mut session, json!({ "cmd": "continue" }));
    for relic in ["bounty", "interest", "vault"] {
        send(&mut session, json!({ "cmd": "debug_set_relic", "relic": relic, "on": true }));
    }
    send(&mut session, json!({ "cmd": "debug_set_draft", "offers": [{ "kind": "relic", "id": "coup" }] }));
    let blocked = |reply: &Value| reply["view"]["reward"]["offers"][0]["blocked"].clone();
    let relics = |reply: &Value| reply["view"]["relics"].as_array().unwrap().len();

    let reply = send(&mut session, json!({ "cmd": "debug_set_relic_slots", "slots": 1 }));
    assert_eq!(event_types(&reply), ["debug_changed"]);
    assert_eq!((relics(&reply), &reply["view"]["relic_slots"], blocked(&reply)), (3, &json!(1), json!("relics_full")));
    assert_eq!(ask(&mut session, json!({ "cmd": "take_reward", "index": 0 }))["error"]["code"], json!("blocked"));
    // The offer is free when the run has fewer relics than slots.
    for (relic, left, state) in
        [("bounty", 2, json!("relics_full")), ("interest", 1, json!("relics_full")), ("vault", 0, Value::Null)]
    {
        let reply = send(&mut session, json!({ "cmd": "discard_relic", "relic": relic }));
        assert_eq!((relics(&reply), blocked(&reply)), (left, state), "{relic}");
    }
    send(&mut session, json!({ "cmd": "take_reward", "index": 0 }));
}

#[test]
fn debug_state_has_the_run_the_meta_and_the_tuning() {
    let mut session = debug_session(1);
    let reply = send(&mut session, json!({ "cmd": "debug_state" }));
    assert_eq!(reply["events"], json!([]));
    assert_eq!(reply["view"]["screen"], json!("title"));
    let state = &reply["data"]["debug"];
    assert_eq!(state["run"], Value::Null);
    assert_eq!(state["meta"], json!({ "crowns": 0, "upgrades": {} }));
    assert_eq!((state["floors"].as_array().unwrap().len(), state["kinds"].as_array().unwrap().len()), (8, 5));

    send(&mut session, json!({ "cmd": "debug_set_crowns", "crowns": 4 }));
    send(&mut session, json!({ "cmd": "debug_set_upgrade", "upgrade": "pawn", "level": 2 }));
    send(&mut session, json!({ "cmd": "debug_set_seed", "seed": 123_456_789 }));
    send(&mut session, json!({ "cmd": "new_run" }));
    send(&mut session, json!({ "cmd": "debug_set_floor", "floor": 4 }));
    send(&mut session, json!({ "cmd": "debug_set_gold", "gold": 12 }));
    send(&mut session, json!({ "cmd": "debug_bar_relic", "relic": "interest", "barred": true }));
    let reply = send(&mut session, json!({ "cmd": "debug_set_relic", "relic": "bounty", "on": true }));
    let state = &reply["data"]["debug"];
    let traits = generate_enemy(123_456_789, 4, &Tuning::default()).traits;
    assert_eq!(
        state["run"],
        json!({ "seed": 123_456_789, "floor": 4, "gold": 12, "relics": ["bounty"], "relic_slots": 4,
                "traits": traits.iter().map(|id| id.key()).collect::<Vec<_>>() })
    );
    assert_eq!(state["meta"], json!({ "crowns": 4, "upgrades": { "pawn": 2 } }));
    assert_eq!((&state["seed"], &state["barred"]), (&json!(123_456_789), &json!(["interest"])));
    assert_eq!(debug_state(&mut session), *state);

    // After the run ends, the state has no run.
    send(&mut session, json!({ "cmd": "give_up" }));
    assert_eq!(debug_state(&mut session)["run"], Value::Null);
}

#[test]
fn a_session_with_no_debug_refuses_the_commands_of_the_tuning() {
    let mut session = Session::new(Box::new(MemoryStorage::default()), 1);
    let view = session.view();
    for request in [
        json!({ "cmd": "debug_state" }),
        json!({ "cmd": "debug_set_seed", "seed": 7 }),
        json!({ "cmd": "debug_tune", "floor": 1, "budget": 0 }),
        json!({ "cmd": "debug_bar_relic", "relic": "bounty", "barred": true }),
        json!({ "cmd": "debug_set_relic_slots", "slots": 2 }),
    ] {
        let reply = ask(&mut session, request.clone());
        assert_eq!(reply["error"]["code"], json!("debug_disabled"), "{request} -> {reply}");
        assert_eq!(reply["view"], view);
        assert_eq!(reply.get("data"), None);
    }
    // A command of the game has no debug state.
    assert_eq!(send(&mut session, json!({ "cmd": "new_run" })).get("data"), None);
}
