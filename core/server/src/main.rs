//! `chrogue-core`: the command server of Chrogue. It reads newline-delimited JSON requests and
//! writes one response line for each, from stdio or from one TCP client on the loopback address.
//! The game is in `chrogue-game`; this file only moves lines. See `core/PROTOCOL.md`.

use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrogue_game::protocol::MAX_REQUEST_BYTES;
use chrogue_game::{FileStorage, MemoryStorage, Session, Storage};

const USAGE: &str = "\
Usage: chrogue-core (--stdio | --listen 127.0.0.1:PORT) (--save-dir PATH | --no-save)
                    [--seed N] [--debug] [--keep-alive] [--idle-ms N]

  --stdio          Read requests from stdin and write responses to stdout.
  --listen ADDR    Serve one TCP client at a time on a loopback address. PORT 0 picks a free port.
                   The first line on stdout is {\"listening\":\"127.0.0.1:N\"}.
  --save-dir PATH  Keep the saved data in PATH (meta.json and run.json).
  --no-save        Keep the saved data in memory only.
  --seed N         The seed of the random numbers. The default comes from the clock.
  --debug          Accept the debug commands.
  --keep-alive     With --listen, do not exit when no client is connected.
  --idle-ms N      With --listen, exit when no client connects for N ms after a client leaves
                   (default 5000). Before the first client, the limit is 6 times longer.";

enum Mode {
    Stdio,
    Listen(SocketAddr),
}

struct Options {
    mode: Mode,
    save_dir: Option<String>,
    seed: u64,
    debug: bool,
    keep_alive: bool,
    idle: Duration,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut mode = None;
    let mut save_dir = None;
    let mut no_save = false;
    let mut seed = None;
    let (mut debug, mut keep_alive) = (false, false);
    let mut idle = Duration::from_millis(5000);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| rest.next().cloned().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--stdio" => mode = Some(Mode::Stdio),
            "--listen" => {
                let text = value("--listen")?;
                let addr: SocketAddr = text.parse().map_err(|_| format!("\"{text}\" is not an address with a port"))?;
                if !addr.ip().is_loopback() {
                    return Err(format!("{addr} is not a loopback address"));
                }
                mode = Some(Mode::Listen(addr));
            }
            "--save-dir" => save_dir = Some(value("--save-dir")?),
            "--no-save" => no_save = true,
            "--seed" => seed = Some(value("--seed")?.parse().map_err(|_| "--seed needs a whole number".to_string())?),
            "--debug" => debug = true,
            "--keep-alive" => keep_alive = true,
            "--idle-ms" => {
                let ms: u64 = value("--idle-ms")?.parse().map_err(|_| "--idle-ms needs a whole number".to_string())?;
                idle = Duration::from_millis(ms);
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
    Ok(Options { mode, save_dir, seed, debug, keep_alive, idle })
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

/// Serves requests until the input ends or a client sends `quit`. Returns true after `quit`.
fn serve(session: &mut Session, reader: &mut impl BufRead, writer: &mut impl Write) -> io::Result<bool> {
    let mut buf = Vec::new();
    loop {
        let response = match read_line(reader, &mut buf)? {
            Line::End => return Ok(false),
            Line::TooLong => session.too_long(),
            Line::Text(text) if text.trim().is_empty() => continue,
            Line::Text(text) => session.command(&text),
        };
        let mut bytes = response.into_bytes();
        bytes.push(b'\n');
        writer.write_all(&bytes)?;
        writer.flush()?;
        if session.wants_quit() {
            return Ok(true);
        }
    }
}

/// Waits for a client. Returns None when the limit passes with no client.
fn accept(listener: &TcpListener, limit: Option<Duration>) -> io::Result<Option<TcpStream>> {
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(Some(stream)),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => {
                if limit.is_some_and(|limit| start.elapsed() >= limit) {
                    return Ok(None);
                }
                sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e),
        }
    }
}

fn listen(session: &mut Session, addr: SocketAddr, options: &Options) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let mut out = io::stdout().lock();
    writeln!(out, "{{\"listening\":\"{}\"}}", listener.local_addr()?)?;
    out.flush()?;
    let mut limit = (!options.keep_alive).then(|| options.idle * 6);
    while let Some(stream) = accept(&listener, limit)? {
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = stream;
        // A client that breaks the connection ends only its connection, not the session.
        if let Ok(true) = serve(session, &mut reader, &mut writer) {
            return Ok(());
        }
        limit = (!options.keep_alive).then_some(options.idle);
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_options(&args) {
        Ok(options) => options,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("chrogue-core: {message}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let storage: Box<dyn Storage> = match &options.save_dir {
        Some(dir) => Box::new(FileStorage::new(dir)),
        None => Box::new(MemoryStorage::default()),
    };
    let mut session = Session::with_debug(storage, options.seed, options.debug);
    let result = match options.mode {
        Mode::Stdio => serve(&mut session, &mut io::stdin().lock(), &mut io::stdout().lock()).map(|_| ()),
        Mode::Listen(addr) => listen(&mut session, addr, &options),
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
