//! The C interface, as a client uses it: text in, text out, through the functions of `chrogue_core.h`.

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use chrogue_core::{ChrogueCore, Options, chrogue_close, chrogue_command, chrogue_open, chrogue_open_error};
use serde_json::{Value, json};

fn open(options: Value) -> *mut ChrogueCore {
    let options = CString::new(options.to_string()).unwrap();
    unsafe { chrogue_open(options.as_ptr()) }
}

fn text(pointer: *const c_char) -> String {
    unsafe { CStr::from_ptr(pointer) }.to_str().unwrap().to_string()
}

fn command(core: *mut ChrogueCore, request: Value) -> Value {
    let request = CString::new(request.to_string()).unwrap();
    serde_json::from_str(&text(unsafe { chrogue_command(core, request.as_ptr()) })).unwrap()
}

fn save_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("chrogue-embed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_core_plays_the_protocol() {
    let core = open(json!({ "seed": 7 }));
    assert!(!core.is_null());
    let view = command(core, json!({ "id": 1, "cmd": "view" }));
    assert_eq!(view["id"], 1);
    assert_eq!(view["view"]["screen"], "title");
    let battle = command(core, json!({ "cmd": "new_run" }));
    assert_eq!(battle["view"]["screen"], "battle");
    let first = &battle["view"]["moves"][0];
    let moved = command(core, json!({ "cmd": "move", "from": first["from"], "to": first["to"] }));
    assert_eq!(moved["view"]["phase"], "enemy");
    let answer = command(core, json!({ "cmd": "enemy_move" }));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["view"]["phase"], "player");
    unsafe { chrogue_close(core) };
}

#[test]
fn the_same_seed_gives_the_same_game() {
    let play = |seed: Value| {
        let core = open(json!({ "seed": seed }));
        let battle = command(core, json!({ "cmd": "new_run" }));
        unsafe { chrogue_close(core) };
        battle["view"]["pieces"].clone()
    };
    assert_eq!(play(json!(7)), play(json!("7")));
    assert_ne!(play(json!(7)), play(json!(8)));
}

#[test]
fn a_bad_request_gets_an_error_response() {
    let core = open(json!({}));
    let not_json = CString::new("move e2").unwrap();
    let response: Value = serde_json::from_str(&text(unsafe { chrogue_command(core, not_json.as_ptr()) })).unwrap();
    assert_eq!(response["error"]["code"], "bad_json");
    assert_eq!(command(core, json!({ "cmd": "debug_set_gold", "gold": 5 }))["error"]["code"], "debug_disabled");
    let long = json!({ "cmd": "view", "pad": "x".repeat(70_000) });
    assert_eq!(command(core, long)["error"]["code"], "too_long");
    unsafe { chrogue_close(core) };
}

#[test]
fn the_debug_option_permits_the_debug_commands() {
    let core = open(json!({ "debug": true }));
    command(core, json!({ "cmd": "new_run" }));
    assert_eq!(command(core, json!({ "cmd": "debug_set_gold", "gold": 5 }))["ok"], true);
    unsafe { chrogue_close(core) };
}

#[test]
fn the_save_folder_keeps_the_run_and_has_one_core() {
    let dir = save_dir("keeps");
    let options = json!({ "save_dir": dir.to_str().unwrap(), "seed": 7 });
    let core = open(options.clone());
    command(core, json!({ "cmd": "new_run" }));

    // A second core on the same folder does not open, and the reason names the lock.
    assert!(open(options.clone()).is_null());
    assert!(text(chrogue_open_error()).contains("uses this save directory"), "{}", text(chrogue_open_error()));

    // The close releases the lock, and the next core finds the saved run.
    unsafe { chrogue_close(core) };
    let again = open(options);
    assert!(!again.is_null());
    assert_eq!(command(again, json!({ "cmd": "view" }))["view"]["can_continue"], true);
    unsafe { chrogue_close(again) };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_options_do_not_open_a_core() {
    for (options, reason) in [
        ("[]", "must be a JSON object"),
        ("{", "not JSON"),
        (r#"{"seed":-1}"#, "\"seed\" must be a whole number"),
        (r#"{"seed":"7 "}"#, "\"seed\" must be a whole number"),
        (r#"{"seed":"99999999999999999999"}"#, "too large"),
        (r#"{"save_dir":7}"#, "\"save_dir\" must be a path"),
        (r#"{"debug":"yes"}"#, "\"debug\" must be true or false"),
    ] {
        let text_in = CString::new(options).unwrap();
        assert!(unsafe { chrogue_open(text_in.as_ptr()) }.is_null(), "{options}");
        assert!(text(chrogue_open_error()).contains(reason), "{options}: {}", text(chrogue_open_error()));
    }
}

#[test]
fn null_options_open_a_core_in_memory_and_null_closes() {
    let core = unsafe { chrogue_open(ptr::null()) };
    assert!(!core.is_null());
    assert_eq!(command(core, json!({ "cmd": "view" }))["view"]["screen"], "title");
    unsafe { chrogue_close(core) };
    unsafe { chrogue_close(ptr::null_mut()) };
}

#[test]
fn a_call_with_no_core_gets_an_error_response() {
    let request = CString::new(r#"{"cmd":"view"}"#).unwrap();
    let response: Value =
        serde_json::from_str(&text(unsafe { chrogue_command(ptr::null_mut(), request.as_ptr()) })).unwrap();
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "internal");
    assert!(response.get("view").is_none());
}

#[test]
fn options_ignore_unknown_fields() {
    let options = Options::parse(r#"{"seed":"18446744073709551615","later":1,"save_dir":null}"#).unwrap();
    assert_eq!(options, Options { save_dir: None, seed: Some(u64::MAX), debug: false });
}
