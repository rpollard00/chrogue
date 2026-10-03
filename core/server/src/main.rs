//! `chrogue-core`: the command server of Chrogue. It reads newline-delimited JSON requests and
//! writes one response line for each, from stdio or from one authenticated TCP client on the
//! loopback address. The game is in `chrogue-game`; this file only moves lines. See
//! `core/PROTOCOL.md`.
//!
//! With `--listen`, the main thread owns the session. An accept thread takes each connection and
//! gives it to a short thread that checks the auth line. A connection that passes goes to the
//! main thread, which makes it the current client (a previous client is closed) and starts a
//! thread that reads its lines. The main thread runs each line of the current client and writes
//! the response; it also keeps the idle limit.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrogue_game::protocol::MAX_REQUEST_BYTES;
use chrogue_game::save::OpenError;
use chrogue_game::{FileStorage, MemoryStorage, Session, Storage};

const USAGE: &str = "\
Usage: chrogue-core (--stdio | --listen 127.0.0.1:PORT) (--save-dir PATH | --no-save)
                    [--seed N] [--debug] [--keep-alive] [--idle-ms N] [--client-idle-ms N]

  --stdio             Read requests from stdin and write responses to stdout.
  --listen ADDR       Serve one TCP client at a time on a loopback address. PORT 0 picks a free
                      port. The first line on stdout is {\"listening\":\"127.0.0.1:N\"}. The
                      environment variable CHROGUE_TOKEN must have the token of the clients
                      (32 characters or more); the first line of a client is {\"auth\":\"TOKEN\"}.
  --save-dir PATH     Keep the saved data in PATH (meta.json and run.json). The core locks PATH.
  --no-save           Keep the saved data in memory only.
  --seed N            The seed of the random numbers (digits only). The default comes from the clock.
  --debug             Accept the debug commands.
  --keep-alive        With --listen, never exit for idle time.
  --idle-ms N         With --listen and no client, exit N ms after the last client leaves
                      (default 5000, more than 0). Before the first client, the limit is 6 times longer.
  --client-idle-ms N  With --listen, exit when the connected client sends no line for N ms
                      (default 1800000, 30 minutes; more than 0).

Exit codes: 0 after quit or the idle limit, 1 for an I/O error, 2 for a usage error (also a
missing or short CHROGUE_TOKEN), 3 if another chrogue-core holds the lock of the save directory.";

/// The exit code of a usage error.
const EXIT_USAGE: u8 = 2;
/// The exit code when another live core holds the lock of the save directory.
const EXIT_LOCKED: u8 = 3;

/// The environment variable of the token. It is not a command-line flag, thus it does not show
/// in a list of the processes.
const TOKEN_VAR: &str = "CHROGUE_TOKEN";
const TOKEN_MIN: usize = 32;
const TOKEN_MAX: usize = 1024;
/// A connection that does not send its auth line in this time is closed.
const AUTH_TIMEOUT: Duration = Duration::from_secs(3);
/// A write to a client that takes longer closes that connection.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// The most connections that wait for their auth line at one time. The core closes more.
const PENDING_MAX: usize = 8;
/// The start of an HTTP request. Such a first line closes the connection (a web page can send
/// requests to a loopback port).
const HTTP_METHODS: [&str; 9] =
    ["GET ", "POST ", "PUT ", "OPTIONS ", "HEAD ", "DELETE ", "PATCH ", "CONNECT ", "TRACE "];
const AUTH_REPLY: &[u8] = b"{\"ok\":true,\"auth\":true}\n";

enum Mode {
    Stdio,
    Listen(SocketAddr),
}

struct Options {
    mode: Mode,
    save_dir: Option<PathBuf>,
    seed: u64,
    debug: bool,
    keep_alive: bool,
    idle: Duration,
    client_idle: Duration,
}

/// A whole number with digits only: no sign, no space, no fraction.
fn whole(name: &str, text: &str) -> Result<u64, String> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    match text.parse::<u64>() {
        Ok(n) if digits => Ok(n),
        _ => Err(format!("{name} needs a whole number from 0 to {} with digits only, not \"{text}\"", u64::MAX)),
    }
}

fn millis(name: &str, text: &str) -> Result<Duration, String> {
    match whole(name, text)? {
        0 => Err(format!("{name} must be more than 0")),
        ms => Ok(Duration::from_millis(ms)),
    }
}

fn parse_options(args: Vec<OsString>) -> Result<Options, String> {
    let mut mode = None;
    let mut save_dir = None;
    let mut no_save = false;
    let mut seed = None;
    let (mut debug, mut keep_alive) = (false, false);
    let mut idle = Duration::from_millis(5000);
    let mut client_idle = Duration::from_millis(1_800_000);
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        let arg = arg.into_string().map_err(|arg| format!("The argument {arg:?} is not UTF-8 text"))?;
        let mut raw = |name: &str| rest.next().ok_or(format!("{name} needs a value"));
        let text = |value: OsString, name: &str| {
            value.into_string().map_err(|value| format!("The value {value:?} of {name} is not UTF-8 text"))
        };
        match arg.as_str() {
            "--stdio" => mode = Some(Mode::Stdio),
            "--listen" => {
                let value = text(raw("--listen")?, "--listen")?;
                let addr: SocketAddr =
                    value.parse().map_err(|_| format!("\"{value}\" is not an address with a port"))?;
                if !addr.ip().is_loopback() {
                    return Err(format!("{addr} is not a loopback address"));
                }
                mode = Some(Mode::Listen(addr));
            }
            // A path can be any text of the system, thus it need not be UTF-8.
            "--save-dir" => save_dir = Some(PathBuf::from(raw("--save-dir")?)),
            "--no-save" => no_save = true,
            "--seed" => seed = Some(whole("--seed", &text(raw("--seed")?, "--seed")?)?),
            "--debug" => debug = true,
            "--keep-alive" => keep_alive = true,
            "--idle-ms" => idle = millis("--idle-ms", &text(raw("--idle-ms")?, "--idle-ms")?)?,
            "--client-idle-ms" => {
                client_idle = millis("--client-idle-ms", &text(raw("--client-idle-ms")?, "--client-idle-ms")?)?
            }
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("Unknown argument \"{other}\"")),
        }
    }
    let mode = mode.ok_or("Give --stdio or --listen")?;
    if save_dir.is_some() == no_save {
        return Err("Give one of --save-dir or --no-save".into());
    }
    let seed =
        seed.unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
    Ok(Options { mode, save_dir, seed, debug, keep_alive, idle, client_idle })
}

/// The token of `CHROGUE_TOKEN`: 32 to 1024 printable ASCII characters with no space, `"`, or `\`,
/// thus the auth line is the token in a JSON string with no escapes.
fn read_token() -> Result<String, String> {
    let value = std::env::var_os(TOKEN_VAR).ok_or(format!("--listen needs the environment variable {TOKEN_VAR}"))?;
    let token = value.into_string().map_err(|_| format!("{TOKEN_VAR} is not UTF-8 text"))?;
    if token.len() < TOKEN_MIN || token.len() > TOKEN_MAX {
        return Err(format!("{TOKEN_VAR} must have {TOKEN_MIN} to {TOKEN_MAX} characters, not {}", token.len()));
    }
    if token.chars().any(|c| !c.is_ascii_graphic() || c == '"' || c == '\\') {
        return Err(format!("{TOKEN_VAR} may have only printable ASCII characters, with no space, '\"', or '\\'"));
    }
    Ok(token)
}

/// Compares two byte strings in a time that depends only on their lengths.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        diff |= std::hint::black_box((x ^ y) as usize);
    }
    diff == 0
}

enum Line {
    Text(String),
    TooLong,
    End,
}

/// Reads one line. A line longer than `MAX_REQUEST_BYTES` is read to its end and dropped.
fn read_line(reader: &mut impl BufRead, buf: &mut Vec<u8>) -> io::Result<Line> {
    buf.clear();
    let mut too_long = false;
    loop {
        let available = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(if too_long {
                Line::TooLong
            } else if buf.is_empty() {
                Line::End
            } else {
                Line::Text(String::from_utf8_lossy(buf).into_owned())
            });
        }
        let (part, used, done) = match available.iter().position(|&b| b == b'\n') {
            Some(i) => (&available[..i], i + 1, true),
            None => (available, available.len(), false),
        };
        if !too_long {
            buf.extend_from_slice(part);
            if buf.len() > MAX_REQUEST_BYTES {
                too_long = true;
                buf.clear();
            }
        }
        reader.consume(used);
        if done {
            if too_long {
                return Ok(Line::TooLong);
            }
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(Line::Text(String::from_utf8_lossy(buf).into_owned()));
        }
    }
}

/// The response to a line, or None for an empty line.
fn respond(session: &mut Session, line: Line) -> Option<Vec<u8>> {
    let response = match line {
        Line::End => return None,
        Line::TooLong => session.too_long(),
        Line::Text(text) if text.trim().is_empty() => return None,
        Line::Text(text) => session.command(&text),
    };
    let mut bytes = response.into_bytes();
    bytes.push(b'\n');
    Some(bytes)
}

/// Serves stdio until the input ends or a client sends `quit`.
fn serve_stdio(session: &mut Session) -> io::Result<()> {
    let (mut reader, mut writer) = (io::stdin().lock(), io::stdout().lock());
    let mut buf = Vec::new();
    loop {
        let line = read_line(&mut reader, &mut buf)?;
        if matches!(line, Line::End) {
            return Ok(());
        }
        if let Some(bytes) = respond(session, line) {
            writer.write_all(&bytes)?;
            writer.flush()?;
        }
        if session.wants_quit() {
            return Ok(());
        }
    }
}

/// Reads the first line of a connection within `AUTH_TIMEOUT`. Returns the reader (with the
/// bytes after the line) if the line is exactly the auth line, else None.
fn authenticate(stream: &TcpStream, expected: &[u8]) -> io::Result<Option<BufReader<TcpStream>>> {
    let deadline = Instant::now() + AUTH_TIMEOUT;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        // The clone shares the socket, thus the timeout applies to the reads of `reader`.
        stream.set_read_timeout(Some(left))?;
        let available = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Ok(None),
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(None);
        }
        let (part, used, done) = match available.iter().position(|&b| b == b'\n') {
            Some(i) => (&available[..i], i + 1, true),
            None => (available, available.len(), false),
        };
        line.extend_from_slice(part);
        reader.consume(used);
        let http = HTTP_METHODS.iter().any(|method| line.starts_with(method.as_bytes()));
        if http || line.len() > expected.len() + 1 {
            return Ok(None);
        }
        if done {
            break;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if !same_bytes(&line, expected) {
        return Ok(None);
    }
    stream.set_read_timeout(None)?;
    Ok(Some(reader))
}

enum Msg {
    /// An authenticated connection and its reader.
    Client(TcpStream, BufReader<TcpStream>),
    Line(u64, Line),
    Closed(u64),
}

/// Takes each connection and checks its auth line on a thread of its own, thus a client that
/// does not authenticate does not delay another client.
fn accept_loop(listener: TcpListener, expected: Arc<Vec<u8>>, to_main: SyncSender<Msg>) {
    let pending = Arc::new(AtomicUsize::new(0));
    loop {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                // For example ECONNABORTED (a client that left before the accept) or EMFILE.
                eprintln!("chrogue-core: accept: {e}");
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if pending.fetch_add(1, Ordering::SeqCst) >= PENDING_MAX {
            pending.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let (expected, to_main, done) = (expected.clone(), to_main.clone(), pending.clone());
        let spawned = thread::Builder::new().name("auth".into()).spawn(move || {
            let result = authenticate(&stream, &expected);
            done.fetch_sub(1, Ordering::SeqCst);
            let ready = stream.set_nodelay(true).and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)));
            match (result, ready) {
                (Ok(Some(reader)), Ok(())) => {
                    let _ = to_main.send(Msg::Client(stream, reader));
                }
                _ => {
                    let _ = stream.shutdown(Shutdown::Both);
                }
            }
        });
        if spawned.is_err() {
            pending.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Sends the lines of one connection to the main thread, then `Closed`.
fn read_loop(id: u64, mut reader: BufReader<TcpStream>, to_main: SyncSender<Msg>) {
    let mut buf = Vec::new();
    loop {
        match read_line(&mut reader, &mut buf) {
            Ok(Line::End) | Err(_) => {
                let _ = to_main.send(Msg::Closed(id));
                return;
            }
            Ok(line) => {
                if to_main.send(Msg::Line(id, line)).is_err() {
                    return;
                }
            }
        }
    }
}

fn write_all(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    stream.write_all(bytes)?;
    stream.flush()
}

fn listen(session: &mut Session, addr: SocketAddr, options: &Options, token: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let mut out = io::stdout().lock();
    writeln!(out, "{{\"listening\":\"{}\"}}", listener.local_addr()?)?;
    out.flush()?;
    drop(out);

    let (to_main, inbox) = mpsc::sync_channel::<Msg>(64);
    let expected = Arc::new(format!("{{\"auth\":\"{token}\"}}").into_bytes());
    let sender = to_main.clone();
    thread::Builder::new().name("accept".into()).spawn(move || accept_loop(listener, expected, sender))?;

    // The idle time counts from the last line of the client or the last close, whichever is later.
    let mut last = Instant::now();
    let mut seen_client = false;
    let mut current: Option<(u64, TcpStream)> = None;
    let mut next_id = 0;
    loop {
        let limit = match (&current, seen_client) {
            (Some(_), _) => options.client_idle,
            (None, true) => options.idle,
            (None, false) => options.idle * 6,
        };
        let message = if options.keep_alive {
            inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            inbox.recv_timeout((last + limit).saturating_duration_since(Instant::now()))
        };
        let message = match message {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => return Ok(()),
            Err(RecvTimeoutError::Disconnected) => return Err(io::Error::other("the accept thread stopped")),
        };
        match message {
            Msg::Client(mut stream, reader) => {
                if let Some((_, old)) = current.take() {
                    // The newer client takes over. The session stays.
                    let _ = old.shutdown(Shutdown::Both);
                    eprintln!("chrogue-core: a new client took the place of the connected client");
                }
                seen_client = true;
                last = Instant::now();
                if write_all(&mut stream, AUTH_REPLY).is_err() {
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                next_id += 1;
                let (id, sender) = (next_id, to_main.clone());
                match thread::Builder::new().name("read".into()).spawn(move || read_loop(id, reader, sender)) {
                    Ok(_) => current = Some((id, stream)),
                    Err(_) => {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                }
            }
            Msg::Line(id, line) => {
                let Some((current_id, stream)) = current.as_mut().filter(|(current_id, _)| *current_id == id) else {
                    continue;
                };
                last = Instant::now();
                let Some(bytes) = respond(session, line) else { continue };
                // A client that breaks the connection or does not read ends only its connection.
                if write_all(stream, &bytes).is_err() {
                    let _ = stream.shutdown(Shutdown::Both);
                    eprintln!("chrogue-core: could not write to client {current_id}; closed it");
                    current = None;
                    last = Instant::now();
                }
                if session.wants_quit() {
                    return Ok(());
                }
            }
            Msg::Closed(id) => {
                if current.as_ref().is_some_and(|(current_id, _)| *current_id == id) {
                    current = None;
                    last = Instant::now();
                }
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("chrogue-core: {message}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let token = match options.mode {
        Mode::Listen(_) => match read_token() {
            Ok(token) => token,
            Err(message) => {
                eprintln!("chrogue-core: {message}");
                return ExitCode::from(EXIT_USAGE);
            }
        },
        Mode::Stdio => String::new(),
    };
    let storage: Box<dyn Storage> = match &options.save_dir {
        Some(dir) => match FileStorage::open(dir) {
            Ok(storage) => {
                if let Some(pid) = storage.recovered_lock() {
                    eprintln!(
                        "chrogue-core: took over the stale lock of a core that ended (PID {pid}) in {}",
                        dir.display()
                    );
                }
                Box::new(storage)
            }
            Err(error @ OpenError::Locked { .. }) => {
                eprintln!("chrogue-core: {}: {error}", dir.display());
                return ExitCode::from(EXIT_LOCKED);
            }
            Err(error) => {
                eprintln!("chrogue-core: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => Box::new(MemoryStorage::default()),
    };
    let mut session = Session::with_debug(storage, options.seed, options.debug);
    let result = match options.mode {
        Mode::Stdio => serve_stdio(&mut session),
        Mode::Listen(addr) => listen(&mut session, addr, &options, &token),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("chrogue-core: {e}");
            ExitCode::FAILURE
        }
    }
}
