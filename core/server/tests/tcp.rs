//! The server binary over TCP and stdio: the auth line, the port line, a battle that outlives a
//! connection, a newer client that takes over, `quit`, the idle exits, `--keep-alive`, long
//! lines, the lock of the save directory, and the checks of the command line. The test with
//! `--nocapture` prints the round-trip times.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_chrogue-core");
const TOKEN: &str = "test-token-0123456789abcdef0123456789";

/// A server process. Drop kills it, thus a failed test leaves no process.
struct Server {
    child: Child,
    addr: String,
    _stdout: BufReader<ChildStdout>,
}

impl Server {
    /// A server with no saved data.
    fn start(args: &[&str]) -> Server {
        Server::spawn(&["--no-save"], args)
    }

    fn spawn(storage: &[&str], args: &[&str]) -> Server {
        let mut child = Command::new(BIN)
            .args(["--listen", "127.0.0.1:0", "--seed", "5"])
            .args(storage)
            .args(args)
            .env("CHROGUE_TOKEN", TOKEN)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let first: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line:?}"));
        let addr = first["listening"].as_str().unwrap().to_string();
        assert!(addr.starts_with("127.0.0.1:") && !addr.ends_with(":0"), "{addr}");
        Server { child, addr, _stdout: stdout }
    }

    /// A connection that sends no auth line.
    fn raw(&self) -> Client {
        let stream = TcpStream::connect(&self.addr).unwrap();
        stream.set_nodelay(true).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        Client { reader: BufReader::new(stream.try_clone().unwrap()), stream }
    }

    /// An authenticated connection.
    fn connect(&self) -> Client {
        let mut client = self.raw();
        assert_eq!(client.line(&format!("{{\"auth\":\"{TOKEN}\"}}")), json!({ "ok": true, "auth": true }));
        client
    }

    /// Waits for the process to exit. Returns false if it still runs after `limit`.
    fn exits_within(&mut self, limit: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if self.child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl Client {
    fn line(&mut self, text: &str) -> Value {
        self.stream.write_all(format!("{text}\n").as_bytes()).unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }

    fn send(&mut self, request: Value) -> Value {
        let reply = self.line(&request.to_string());
        assert_eq!(reply["ok"], json!(true), "{request} -> {}", reply["error"]);
        reply
    }

    /// True if the server closes the connection with no byte within `limit`.
    fn closed_within(&mut self, limit: Duration) -> bool {
        self.stream.set_read_timeout(Some(limit)).unwrap();
        let mut rest = Vec::new();
        match self.reader.read_to_end(&mut rest) {
            Ok(_) => rest.is_empty(),
            Err(e) => {
                matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted)
                    && rest.is_empty()
            }
        }
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chrogue-tcp-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

#[test]
fn a_battle_outlives_a_connection_and_quit_ends_the_process() {
    let mut server = Server::start(&["--debug", "--idle-ms", "3000"]);
    let mut client = server.connect();
    assert_eq!(client.send(json!({ "id": 1, "cmd": "hello" }))["data"]["protocol"], json!(1));
    let start = client.send(json!({ "id": 2, "cmd": "new_run" }))["view"].clone();
    let m = &start["moves"][0];
    client.send(json!({ "cmd": "move", "from": m["from"], "to": m["to"] }));
    let before = client.send(json!({ "cmd": "enemy_move" }))["view"].clone();
    drop(client);

    // A new connection finds the same battle.
    let mut client = server.connect();
    let after = client.send(json!({ "cmd": "view" }))["view"].clone();
    assert_eq!(after, before);
    assert_eq!(after["screen"], json!("battle"));

    // The round trip of `view` and of `move`.
    let mut views = Vec::new();
    for _ in 0..300 {
        let t = Instant::now();
        client.send(json!({ "cmd": "view" }));
        views.push(t.elapsed());
    }
    let mut moves = Vec::new();
    for _ in 0..60 {
        let view = client.send(json!({ "cmd": "view" }))["view"].clone();
        if view["phase"] != json!("player") {
            client.send(json!({ "cmd": "to_title" }));
            client.send(json!({ "cmd": "new_run" }));
            continue;
        }
        let m = view["moves"][0].clone();
        let t = Instant::now();
        let reply = client.send(json!({ "cmd": "move", "from": m["from"], "to": m["to"], "promo": m.get("promo") }));
        moves.push(t.elapsed());
        if reply["view"]["phase"] == json!("enemy") {
            client.send(json!({ "cmd": "debug_ai_move", "level": 1 }));
        }
    }
    // The enemy move of the strongest level of the AI on floor 8, from the start of each battle.
    let strongest = client.send(json!({ "cmd": "debug_state" }))["data"]["debug"]["limits"]["level"].clone();
    client.send(json!({ "cmd": "debug_tune", "floor": 8, "level": strongest }));
    let mut enemy = Vec::new();
    for _ in 0..12 {
        client.send(json!({ "cmd": "to_title" }));
        client.send(json!({ "cmd": "new_run" }));
        client.send(json!({ "cmd": "debug_set_floor", "floor": 8 }));
        for _ in 0..6 {
            let reply = client.send(json!({ "cmd": "debug_ai_move", "level": 1 }));
            if reply["view"]["phase"] != json!("enemy") {
                break;
            }
            let t = Instant::now();
            let reply = client.send(json!({ "cmd": "enemy_move" }));
            enemy.push(t.elapsed());
            if reply["view"]["phase"] != json!("player") {
                break;
            }
        }
    }
    println!(
        "TCP round trip: view median {:?} (max {:?}, {} bytes); move median {:?}; enemy_move of the strongest level median {:?} max {:?} ({} moves)",
        median(views.clone()),
        views.iter().max().unwrap(),
        after.to_string().len(),
        median(moves),
        median(enemy.clone()),
        enemy.iter().max().unwrap(),
        enemy.len(),
    );

    let reply = client.send(json!({ "cmd": "quit" }));
    assert_eq!(reply["events"][0]["type"], json!("quit"));
    assert!(server.exits_within(Duration::from_secs(3)));
    assert!(server.child.try_wait().unwrap().unwrap().success());
}

#[test]
fn the_server_exits_when_no_client_comes_back() {
    let mut server = Server::start(&["--idle-ms", "200"]);
    let mut client = server.connect();
    client.send(json!({ "cmd": "new_run" }));
    drop(client);
    assert!(server.exits_within(Duration::from_secs(3)));

    // Before the first client the limit is 6 times longer: 1.2 s here.
    let mut server = Server::start(&["--idle-ms", "200"]);
    assert!(!server.exits_within(Duration::from_millis(600)));
    assert!(server.exits_within(Duration::from_secs(3)));
}

#[test]
fn keep_alive_waits_for_the_next_client() {
    let mut server = Server::start(&["--keep-alive", "--idle-ms", "100"]);
    let mut client = server.connect();
    client.send(json!({ "cmd": "new_run" }));
    drop(client);
    assert!(!server.exits_within(Duration::from_millis(1500)));
    let mut client = server.connect();
    assert_eq!(client.send(json!({ "cmd": "view" }))["view"]["screen"], json!("battle"));
    client.send(json!({ "cmd": "quit" }));
    assert!(server.exits_within(Duration::from_secs(3)));
}

#[test]
fn a_long_line_gets_an_error_and_the_connection_stays() {
    let server = Server::start(&[]);
    let mut client = server.connect();
    let long = format!("{{\"cmd\":\"view\",\"pad\":\"{}\"}}", "x".repeat(200_000));
    let reply = client.line(&long);
    assert_eq!(reply["error"]["code"], json!("too_long"));
    assert_eq!(client.line("{\"cmd\":\"nope\"}")["error"]["code"], json!("unknown_command"));
    assert_eq!(client.line("not json")["error"]["code"], json!("bad_json"));
    assert_eq!(client.send(json!({ "cmd": "view" }))["view"]["screen"], json!("title"));
}

#[test]
fn the_server_accepts_only_a_loopback_address() {
    let status = Command::new(BIN).args(["--listen", "0.0.0.0:0", "--no-save"]).stderr(Stdio::null()).status().unwrap();
    assert_eq!(status.code(), Some(2));
    let status = Command::new(BIN).args(["--stdio"]).stderr(Stdio::null()).status().unwrap();
    assert_eq!(status.code(), Some(2), "a server needs --save-dir or --no-save");
}

#[test]
fn stdio_answers_each_line_and_exits_at_the_end_of_the_input() {
    let mut child = Command::new(BIN)
        .args(["--stdio", "--no-save", "--seed", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let long = "y".repeat(100_000);
    input
        .write_all(
            format!("{{\"id\":1,\"cmd\":\"view\"}}\n\n{long}\nbad\r\n{{\"id\":\"x\",\"cmd\":\"new_run\"}}\n")
                .as_bytes(),
        )
        .unwrap();
    drop(input);
    let mut output = String::new();
    child.stdout.take().unwrap().read_to_string(&mut output).unwrap();
    assert!(child.wait().unwrap().success());
    let replies: Vec<Value> = output.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let codes: Vec<Value> = replies.iter().map(|r| r["error"]["code"].clone()).collect();
    assert_eq!(codes, vec![Value::Null, json!("too_long"), json!("bad_json"), Value::Null]);
    assert_eq!(replies[3]["id"], json!("x"));
    assert_eq!(replies[3]["view"]["screen"], json!("battle"));
}

/// An HTTP request (a web page can send one to a loopback port) and a wrong first line run no
/// command and write no file.
#[test]
fn a_connection_with_no_valid_auth_line_runs_nothing() {
    let dir = temp_dir("auth");
    let server = Server::spawn(&["--save-dir", dir.to_str().unwrap()], &["--debug"]);
    let new_run = "{\"cmd\":\"new_run\"}\n";
    let firsts = [
        format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\n\r\n{new_run}"),
        format!("GET /?{new_run}"),
        format!("OPTIONS * HTTP/1.1\r\n\r\n{new_run}"),
        format!("{{\"auth\":\"{}x\"}}\n{new_run}", TOKEN),
        format!("{{\"auth\":\"{}\"}}\n{new_run}", &TOKEN[..TOKEN.len() - 1]),
        format!("{{\"auth\": \"{TOKEN}\"}}\n{new_run}"),
        format!("{{\"auth\":\"{TOKEN}\",\"cmd\":\"new_run\"}}\n{new_run}"),
        new_run.to_string(),
        format!("{{\"cmd\":\"quit\"}}\n{{\"auth\":\"{TOKEN}\"}}\n{new_run}"),
        format!("\n{{\"auth\":\"{TOKEN}\"}}\n"),
    ];
    for first in &firsts {
        let mut client = server.raw();
        client.stream.write_all(first.as_bytes()).unwrap();
        assert!(client.closed_within(Duration::from_secs(5)), "{first:?} got an answer");
    }
    assert_eq!(names(&dir), vec!["chrogue-core.lock"], "a file changed");
    let mut client = server.connect();
    let view = client.send(json!({ "cmd": "view" }))["view"].clone();
    assert_eq!((view["screen"].clone(), view["can_continue"].clone()), (json!("title"), json!(false)));
    // The auth line and a command can come in one packet.
    let mut client = server.raw();
    client.stream.write_all(format!("{{\"auth\":\"{TOKEN}\"}}\r\n{{\"id\":4,\"cmd\":\"view\"}}\n").as_bytes()).unwrap();
    let mut lines = Vec::new();
    for _ in 0..2 {
        let mut line = String::new();
        client.reader.read_line(&mut line).unwrap();
        lines.push(serde_json::from_str::<Value>(&line).unwrap());
    }
    assert_eq!(lines[0], json!({ "ok": true, "auth": true }));
    assert_eq!(lines[1]["id"], json!(4));
    drop(server);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_client_that_does_not_authenticate_is_dropped_after_three_seconds() {
    let server = Server::start(&["--keep-alive"]);
    let mut silent = server.raw();
    let start = Instant::now();
    assert!(silent.closed_within(Duration::from_secs(6)));
    let waited = start.elapsed();
    assert!(waited >= Duration::from_millis(2500) && waited < Duration::from_secs(5), "{waited:?}");
    // A client that sends a part of the line slowly gets the same limit.
    let mut slow = server.raw();
    let start = Instant::now();
    for byte in "{\"auth\":\"".bytes() {
        if slow.stream.write_all(&[byte]).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    assert!(slow.closed_within(Duration::from_secs(6)));
    assert!(start.elapsed() < Duration::from_secs(6), "{:?}", start.elapsed());
}

/// A client that connects and says nothing, or that authenticates and then says nothing, does
/// not stop a newer client. The newer client gets the same session, and the older connection
/// closes.
#[test]
fn a_silent_client_does_not_block_a_newer_client() {
    let server = Server::start(&["--debug", "--keep-alive"]);
    let _unauthenticated: Vec<Client> = (0..3).map(|_| server.raw()).collect();
    let start = Instant::now();
    let mut first = server.connect();
    assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
    let battle = first.send(json!({ "cmd": "new_run" }))["view"].clone();

    let start = Instant::now();
    let mut second = server.connect();
    assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
    assert_eq!(second.send(json!({ "cmd": "view" }))["view"], battle);
    assert!(first.closed_within(Duration::from_secs(5)), "the older connection stays open");
    second.send(json!({ "cmd": "quit" }));
}

#[test]
fn the_server_exits_when_a_connected_client_is_silent() {
    let mut server = Server::start(&["--idle-ms", "200", "--client-idle-ms", "600"]);
    let mut client = server.connect();
    client.send(json!({ "cmd": "view" }));
    let start = Instant::now();
    assert!(!server.exits_within(Duration::from_millis(400)));
    assert!(server.exits_within(Duration::from_secs(3)));
    assert!(start.elapsed() >= Duration::from_millis(550), "{:?}", start.elapsed());

    // `--keep-alive` keeps the server for a silent client.
    let mut server = Server::start(&["--keep-alive", "--client-idle-ms", "100"]);
    let mut client = server.connect();
    assert!(!server.exits_within(Duration::from_millis(800)));
    client.send(json!({ "cmd": "quit" }));
    assert!(server.exits_within(Duration::from_secs(3)));
}

#[test]
fn a_second_core_on_a_save_directory_does_not_start() {
    let dir = temp_dir("lock");
    let path = dir.to_str().unwrap();
    let server = Server::spawn(&["--save-dir", path], &[]);
    let mut client = server.connect();
    client.send(json!({ "cmd": "new_run" }));

    for mode in [&["--stdio"][..], &["--listen", "127.0.0.1:0"][..]] {
        let output = Command::new(BIN)
            .args(mode)
            .args(["--save-dir", path])
            .env("CHROGUE_TOKEN", TOKEN)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3), "{mode:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("another chrogue-core"), "{message}");
        assert!(message.contains(&server.child.id().to_string()), "the message has the PID: {message}");
        assert!(output.stdout.is_empty());
    }
    // The first core keeps working and saving.
    client.send(json!({ "cmd": "give_up" }));
    let meta: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["data"]["runs"], json!(1));
    client.send(json!({ "cmd": "quit" }));
    drop(server);

    // A core that was killed leaves its PID in the lock file. The next core takes the lock over.
    let mut killed = Server::spawn(&["--save-dir", path], &[]);
    killed.child.kill().unwrap();
    killed.child.wait().unwrap();
    assert!(!std::fs::read_to_string(dir.join("chrogue-core.lock")).unwrap().trim().is_empty());
    let mut child = Command::new(BIN)
        .args(["--stdio", "--save-dir", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"cmd\":\"view\"}\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["view"]["meta"]["runs"], json!(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("stale lock"));
    // A clean exit empties the lock file.
    assert_eq!(std::fs::read_to_string(dir.join("chrogue-core.lock")).unwrap(), "");
    drop(killed);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Each bad command line exits with code 2 and a message, with no panic.
#[test]
fn a_bad_command_line_gets_a_clear_error() {
    let run = |args: &[&std::ffi::OsStr], token: Option<&str>| {
        let mut command = Command::new(BIN);
        command.args(args).env_remove("CHROGUE_TOKEN").stdin(Stdio::null());
        if let Some(token) = token {
            command.env("CHROGUE_TOKEN", token);
        }
        let output = command.output().unwrap();
        (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let os = |args: &[&str]| args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let cases: Vec<(Vec<std::ffi::OsString>, Option<&str>, &str)> = vec![
        (os(&["--listen", "127.0.0.1:0", "--no-save"]), None, "CHROGUE_TOKEN"),
        (os(&["--listen", "127.0.0.1:0", "--no-save"]), Some("short"), "32"),
        (os(&["--listen", "127.0.0.1:0", "--no-save"]), Some("a token with spaces in it, which is long"), "printable"),
        (os(&["--listen", "127.0.0.1:0", "--no-save", "--token", TOKEN]), Some(TOKEN), "Unknown argument"),
        (os(&["--stdio", "--no-save", "--seed", "+5"]), None, "--seed"),
        (os(&["--stdio", "--no-save", "--seed", "-1"]), None, "--seed"),
        (os(&["--stdio", "--no-save", "--seed", "1.0"]), None, "--seed"),
        (os(&["--stdio", "--no-save", "--seed", " 7"]), None, "--seed"),
        (os(&["--stdio", "--no-save", "--seed", "18446744073709551616"]), None, "--seed"),
        (os(&["--stdio", "--no-save", "--seed"]), None, "needs a value"),
        (os(&["--stdio", "--no-save", "--idle-ms", "0"]), None, "more than 0"),
        (os(&["--stdio", "--no-save", "--client-idle-ms", "0"]), None, "more than 0"),
    ];
    #[cfg(unix)]
    let cases = {
        use std::os::unix::ffi::OsStringExt;
        let mut cases = cases;
        let bad = std::ffi::OsString::from_vec(vec![b'-', b'-', 0xff, 0xfe]);
        cases.push((vec!["--stdio".into(), "--no-save".into(), bad.clone()], None, "UTF-8"));
        cases.push((vec!["--stdio".into(), "--no-save".into(), "--seed".into(), bad], None, "UTF-8"));
        cases
    };
    for (args, token, needle) in &cases {
        let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
        let (code, message) = run(&refs, *token);
        assert_eq!(code, Some(2), "{args:?}: {message}");
        assert!(message.contains(needle), "{args:?}: {message}");
        assert!(!message.contains("panicked"), "{args:?}: {message}");
    }
    // The largest seed works.
    let (code, _) = run(
        &[std::ffi::OsStr::new("--stdio"), "--no-save".as_ref(), "--seed".as_ref(), "18446744073709551615".as_ref()],
        None,
    );
    assert_eq!(code, Some(0));
}
