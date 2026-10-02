//! The server binary over TCP and stdio: the port line, a battle that outlives a connection,
//! `quit`, the exit when no client comes back, `--keep-alive`, and long lines. The test with
//! `--nocapture` prints the round-trip times.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_chrogue-core");

/// A server process. Drop kills it, thus a failed test leaves no process.
struct Server {
    child: Child,
    addr: String,
    _stdout: BufReader<ChildStdout>,
}

impl Server {
    fn start(args: &[&str]) -> Server {
        let mut child = Command::new(BIN)
            .args(["--listen", "127.0.0.1:0", "--no-save", "--seed", "5"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let first: Value = serde_json::from_str(&line).unwrap();
        let addr = first["listening"].as_str().unwrap().to_string();
        assert!(addr.starts_with("127.0.0.1:") && !addr.ends_with(":0"), "{addr}");
        Server { child, addr, _stdout: stdout }
    }

    fn connect(&self) -> Client {
        let stream = TcpStream::connect(&self.addr).unwrap();
        stream.set_nodelay(true).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        Client { reader: BufReader::new(stream.try_clone().unwrap()), stream }
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
    // The enemy move on floor 8, from the start of each battle.
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
        "TCP round trip: view median {:?} (max {:?}, {} bytes); move median {:?}; enemy_move floor 8 median {:?} max {:?} ({} moves)",
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
