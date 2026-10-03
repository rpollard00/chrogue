//! Saved data. A `Storage` keeps two documents: the meta (crowns and upgrades) and the run in
//! progress. This module writes them as versioned JSON and checks the data that it reads.
//!
//! The checks: unknown relic and upgrade ids are dropped, a field of the meta that is not valid
//! counts as 0, and an offer that is not valid is dropped. These rules also apply:
//!
//! - A relic id counts one time in a list. The core removes the second copy.
//! - A run has at most `RELICS_MAX` relics. The core keeps the first ones.
//! - The enemy has at most `TRAITS_MAX` traits, and each one is a relic that a boss can have.
//!   The core keeps the first ones.
//! - A count (a floor, gold, an id, a level, an amount) is at most 2^53 - 1, the largest whole
//!   number that a JSON reader in a browser keeps exactly.
//! - A unit id is at most `UNIT_ID_MAX` and appears one time, `nextId` is at most
//!   `UNIT_ID_MAX + 1`, and the enemy has at most `ENEMY_PIECES_MAX` pieces.
//! - The `seed` of a run is at most `SEED_MAX`, and its `rolls` is at most `u32::MAX`. A run with
//!   no `seed` or no `rolls` loads with 0 for that field.
//! - The board of a run must be valid (`Battle::new`): no two enemy pieces on one square, no
//!   enemy piece on the home square of a unit, and one king on each side. A start with the enemy king in check
//!   is valid: normal play can make it, and the core saves the run at the start of the battle.
//! - A document that the core cannot use is not dropped in silence. `load` reports it as a
//!   `Problem`, and the session moves the file aside (`Storage::set_aside`) before it writes
//!   anything.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::battle::Battle;
use crate::chess::{self, Kind, Square};
use crate::content::{FLOORS, RELICS_MAX, RelicId, TRAITS_MAX, UpgradeId};
use crate::run::{ENEMY_PIECES_MAX, Enemy, EnemyPiece, Meta, Offer, Phase, Run, SEED_MAX, UNIT_ID_MAX, Unit, UnitId};

/// The version of the saved documents.
pub const SAVE_VERSION: u64 = 1;

/// The largest count that saved data can have: 2^53 - 1 (`Number.MAX_SAFE_INTEGER`).
pub const COUNT_MAX: u64 = (1 << 53) - 1;

/// The name of the lock file in a save directory.
pub const LOCK_FILE: &str = "chrogue-core.lock";

/// The two saved documents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doc {
    Meta,
    Run,
}

impl Doc {
    /// The name in the protocol (`save_failed`, `save_problem`).
    pub const fn name(self) -> &'static str {
        match self {
            Doc::Meta => "meta",
            Doc::Run => "run",
        }
    }

    /// The name of the file in a save directory.
    pub const fn file_name(self) -> &'static str {
        match self {
            Doc::Meta => "meta.json",
            Doc::Run => "run.json",
        }
    }

    const fn format(self) -> &'static str {
        match self {
            Doc::Meta => "chrogue.meta",
            Doc::Run => "chrogue.run",
        }
    }
}

/// The place where the saved documents are. A document is a JSON text.
pub trait Storage {
    /// The text of a document: `Ok(None)` if there is no document, an error if there is one that
    /// the storage cannot read.
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String>;
    fn save(&mut self, doc: Doc, text: &str) -> Result<(), String>;
    /// Removes the document. No document is not an error.
    fn remove(&mut self, doc: Doc) -> Result<(), String>;
    /// Moves a document that the core cannot use to a place where no save writes. Returns the
    /// name of that place, or `Ok(None)` if the storage keeps no copy.
    fn set_aside(&mut self, doc: Doc) -> Result<Option<String>, String>;
}

/// A storage in memory, for tests and for `--no-save`. A clone shares nothing. `set_aside`
/// drops the document.
#[derive(Clone, Debug, Default)]
pub struct MemoryStorage {
    pub meta: Option<String>,
    pub run: Option<String>,
}

impl MemoryStorage {
    fn slot(&mut self, doc: Doc) -> &mut Option<String> {
        match doc {
            Doc::Meta => &mut self.meta,
            Doc::Run => &mut self.run,
        }
    }
}

impl Storage for MemoryStorage {
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String> {
        Ok(self.slot(doc).clone())
    }

    fn save(&mut self, doc: Doc, text: &str) -> Result<(), String> {
        *self.slot(doc) = Some(text.to_string());
        Ok(())
    }

    fn remove(&mut self, doc: Doc) -> Result<(), String> {
        *self.slot(doc) = None;
        Ok(())
    }

    fn set_aside(&mut self, doc: Doc) -> Result<Option<String>, String> {
        *self.slot(doc) = None;
        Ok(None)
    }
}

/// A storage in a directory: `meta.json` and `run.json`, and the lock file `chrogue-core.lock`.
///
/// - The value holds an operating-system lock on the lock file for its life (`File::try_lock`:
///   `flock` on Linux and macOS, `LockFileEx` on Windows). A second `open` of the directory, in
///   this process or in another one, gets `OpenError::Locked`. The system releases the lock when
///   the process ends in any way, thus a lock file of a process that crashed is stale: no process
///   holds its lock, and `open` takes it over. The file has the PID of the core that holds it;
///   a clean exit empties it, and the file stays.
/// - A save writes a temporary file with a new name, syncs it to the disk, and renames it over
///   the document. On Unix it then syncs the directory. Thus a crash leaves the old document or
///   the new one, never a part of one.
#[derive(Debug)]
pub struct FileStorage {
    dir: PathBuf,
    lock: File,
    recovered: Option<String>,
}

/// Why a save directory cannot be used.
#[derive(Debug)]
pub enum OpenError {
    /// A live process holds the lock. `holder` is the text of the lock file (its PID), if the
    /// system lets another process read it.
    Locked {
        path: PathBuf,
        holder: Option<String>,
    },
    Io(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Locked { path, holder } => {
                let holder = holder.as_ref().map_or(String::new(), |pid| format!(" (PID {pid})"));
                write!(f, "another chrogue-core{holder} uses this save directory: it holds the lock {}", path.display())
            }
            OpenError::Io(message) => f.write_str(message),
        }
    }
}

fn io_error(path: &Path, error: io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// Syncs the entries of a directory to the disk. Windows has no such call in `std`.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    return File::open(dir)?.sync_all();
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

impl FileStorage {
    /// Opens a save directory (it makes the directory if necessary) and locks it.
    pub fn open(dir: impl Into<PathBuf>) -> Result<FileStorage, OpenError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|e| OpenError::Io(io_error(&dir, e)))?;
        let path = dir.join(LOCK_FILE);
        let io = |e: io::Error| OpenError::Io(io_error(&path, e));
        let (mut lock, created) = match OpenOptions::new().read(true).write(true).create_new(true).open(&path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                (OpenOptions::new().read(true).write(true).open(&path).map_err(io)?, false)
            }
            Err(e) => return Err(io(e)),
        };
        match lock.try_lock() {
            Ok(()) => {}
            // A page of a browser has a file system of its own, with no file locks and no second process.
            #[cfg(target_os = "emscripten")]
            Err(TryLockError::Error(e)) if e.kind() == ErrorKind::Unsupported => {}
            Err(TryLockError::WouldBlock) => {
                let holder = fs::read_to_string(&path).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                return Err(OpenError::Locked { path, holder });
            }
            Err(TryLockError::Error(e)) => return Err(io(e)),
        }
        // A lock file with a PID and no lock: that core ended with no clean exit.
        let mut previous = String::new();
        if !created {
            let _ = lock.read_to_string(&mut previous);
        }
        let recovered = Some(previous.trim()).filter(|t| !t.is_empty()).map(str::to_string);
        lock.set_len(0).map_err(io)?;
        lock.seek(SeekFrom::Start(0)).map_err(io)?;
        lock.write_all(format!("{}\n", std::process::id()).as_bytes()).map_err(io)?;
        lock.sync_all().map_err(io)?;
        let storage = FileStorage { dir, lock, recovered };
        storage.remove_temporary_files();
        Ok(storage)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The PID in a stale lock file that `open` took over, if it found one.
    pub fn recovered_lock(&self) -> Option<&str> {
        self.recovered.as_deref()
    }

    /// Removes the temporary files of saves that a crash stopped. The lock makes sure that no
    /// other core writes them.
    fn remove_temporary_files(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let ours = [Doc::Meta, Doc::Run].iter().any(|doc| name.starts_with(&format!("{}.", doc.file_name())));
            if ours && name.ends_with(".tmp") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    fn path(&self, doc: Doc) -> PathBuf {
        self.dir.join(doc.file_name())
    }
}

impl Drop for FileStorage {
    /// Empties the lock file: a later `open` then knows that this core ended cleanly. The system
    /// releases the lock when the file closes.
    fn drop(&mut self) {
        let _ = self.lock.set_len(0);
    }
}

impl Storage for FileStorage {
    fn load(&mut self, doc: Doc) -> Result<Option<String>, String> {
        let path = self.path(doc);
        match fs::read(&path) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| format!("{}: not UTF-8 text", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(&path, e)),
        }
    }

    fn save(&mut self, doc: Doc, text: &str) -> Result<(), String> {
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let path = self.path(doc);
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        let temp = self.dir.join(format!("{}.{}-{n}.tmp", doc.file_name(), std::process::id()));
        let write = || -> io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()
        };
        if let Err(e) = write().and_then(|()| fs::rename(&temp, &path)) {
            let _ = fs::remove_file(&temp);
            return Err(io_error(&path, e));
        }
        sync_dir(&self.dir).map_err(|e| io_error(&self.dir, e))
    }

    fn remove(&mut self, doc: Doc) -> Result<(), String> {
        let path = self.path(doc);
        match fs::remove_file(&path) {
            Ok(()) => sync_dir(&self.dir).map_err(|e| io_error(&self.dir, e)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error(&path, e)),
        }
    }

    /// Renames the document to `<name>.bad-<unix time>` (with `-2`, `-3`, ... if that name is
    /// taken). No save writes to such a name.
    fn set_aside(&mut self, doc: Doc) -> Result<Option<String>, String> {
        let path = self.path(doc);
        if path.symlink_metadata().is_err() {
            return Ok(None);
        }
        let time = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut name = format!("{}.bad-{time}", doc.file_name());
        let mut n = 1;
        while self.dir.join(&name).symlink_metadata().is_ok() {
            n += 1;
            name = format!("{}.bad-{time}-{n}", doc.file_name());
        }
        fs::rename(&path, self.dir.join(&name)).map_err(|e| io_error(&path, e))?;
        let _ = sync_dir(&self.dir);
        Ok(Some(name))
    }
}

// ---- Documents ----

fn document(doc: Doc, data: Value) -> String {
    json!({ "format": doc.format(), "version": SAVE_VERSION, "data": data }).to_string()
}

/// Why the core cannot use a saved document. The codes are the `reason` of `save_problem`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// The storage cannot read it, or it is not JSON (for example a file that a crash cut).
    Unreadable,
    /// It is JSON, but not a document of its format, or its data is not valid.
    Invalid,
    /// It has a version that is newer than `SAVE_VERSION`. A newer core wrote it.
    Newer,
}

impl Reason {
    pub const fn code(self) -> &'static str {
        match self {
            Reason::Unreadable => "unreadable",
            Reason::Invalid => "invalid",
            Reason::Newer => "newer_version",
        }
    }
}

/// A saved document that the core cannot use.
#[derive(Clone, PartialEq, Debug)]
pub struct Problem {
    pub doc: Doc,
    pub reason: Reason,
    pub message: String,
}

/// The saved data of a storage, and the documents that the core cannot use. A document with a
/// problem loads as no data.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub meta: Meta,
    pub run: Option<Run>,
    pub problems: Vec<Problem>,
}

/// The data of a document, `Ok(None)` if the storage has no such document.
fn open(storage: &mut dyn Storage, doc: Doc) -> Result<Option<Value>, (Reason, String)> {
    let text = match storage.load(doc) {
        Ok(Some(text)) => text,
        Ok(None) => return Ok(None),
        Err(message) => return Err((Reason::Unreadable, message)),
    };
    let value: Value = serde_json::from_str(&text).map_err(|e| (Reason::Unreadable, format!("Not JSON: {e}")))?;
    let Value::Object(mut map) = value else { return Err((Reason::Invalid, "Not a JSON object".into())) };
    if map.get("format") != Some(&Value::from(doc.format())) {
        return Err((Reason::Invalid, format!("Not a \"{}\" document", doc.format())));
    }
    match map.get("version").and_then(count) {
        Some(SAVE_VERSION) => {}
        Some(version) if version > SAVE_VERSION => {
            let message = format!("The document has version {version}, and this core reads version {SAVE_VERSION}");
            return Err((Reason::Newer, message));
        }
        _ => return Err((Reason::Invalid, "The document has no valid version".into())),
    }
    Ok(Some(map.remove("data").unwrap_or(Value::Null)))
}

/// Reads the meta and the run. See the module comment for the checks.
pub fn load(storage: &mut dyn Storage) -> Loaded {
    let mut problems = Vec::new();
    let mut report = |doc: Doc, (reason, message): (Reason, String)| problems.push(Problem { doc, reason, message });
    let meta = match open(storage, Doc::Meta) {
        Ok(None) => Meta::default(),
        Ok(Some(data @ Value::Object(_))) => parse_meta(&data),
        Ok(Some(_)) => {
            report(Doc::Meta, (Reason::Invalid, "The data of the meta is not an object".into()));
            Meta::default()
        }
        Err(problem) => {
            report(Doc::Meta, problem);
            Meta::default()
        }
    };
    let run = match open(storage, Doc::Run) {
        Ok(None) => None,
        Ok(Some(data)) => parse_run(&data).map_err(|message| report(Doc::Run, (Reason::Invalid, message))).ok(),
        Err(problem) => {
            report(Doc::Run, problem);
            None
        }
    };
    Loaded { meta, run, problems }
}

pub fn meta_document(meta: &Meta) -> String {
    document(Doc::Meta, meta_json(meta))
}

pub fn run_document(run: &Run) -> String {
    document(Doc::Run, run_json(run))
}

// ---- Writing ----

pub fn meta_json(meta: &Meta) -> Value {
    let upgrades: serde_json::Map<String, Value> =
        meta.upgrades.iter().map(|(id, &level)| (id.key().to_string(), Value::from(level))).collect();
    json!({ "crowns": meta.crowns, "best": meta.best, "runs": meta.runs, "upgrades": upgrades })
}

pub fn offer_json(offer: Offer) -> Value {
    match offer {
        Offer::Piece(kind) => json!({ "kind": "piece", "type": chess::kind_letter(kind).to_string() }),
        Offer::Relic(id) => json!({ "kind": "relic", "id": id.key() }),
        Offer::Gold(amount) => json!({ "kind": "gold", "amount": amount }),
    }
}

pub fn run_json(run: &Run) -> Value {
    let letter = |kind: Kind| chess::kind_letter(kind).to_string();
    json!({
        "seed": run.seed,
        "floor": run.floor,
        "gold": run.gold,
        "army": run.army.iter().map(|u| json!({ "id": u.id, "type": letter(u.kind), "home": u.home })).collect::<Vec<_>>(),
        "nextId": run.next_id,
        "relics": run.relics.iter().map(|id| id.key()).collect::<Vec<_>>(),
        "enemy": {
            "pieces": run.enemy.pieces.iter().map(|p| json!({ "type": letter(p.kind), "square": p.square })).collect::<Vec<_>>(),
            "traits": run.enemy.traits.iter().map(|id| id.key()).collect::<Vec<_>>(),
        },
        "phase": match run.phase { Phase::Battle => "battle", Phase::Camp => "camp" },
        "draft": run.draft.as_ref().map(|d| d.iter().map(|&o| offer_json(o)).collect::<Vec<_>>()),
        "shop": run.shop.iter().map(|&o| offer_json(o)).collect::<Vec<_>>(),
        "rolls": run.rolls,
    })
}

// ---- Reading ----

/// A whole number that is 0 or more (`isCount`). A JSON number such as 4.0 counts.
fn count(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| {
                let f = n.as_f64()?;
                (f >= 0.0 && f.fract() == 0.0 && f <= COUNT_MAX as f64).then_some(f as u64)
            })
            .filter(|&n| n <= COUNT_MAX),
        _ => None,
    }
}

fn square(value: &Value, max: Square) -> Option<Square> {
    count(value).filter(|&s| s <= max as u64).map(|s| s as Square)
}

fn kind(value: &Value) -> Option<Kind> {
    let text = value.as_str()?;
    let mut chars = text.chars();
    let letter = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    chess::kind_from_letter(letter)
}

fn list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// The relic ids of a list. Unknown ids are dropped, and an id counts one time.
fn relics(value: &Value) -> Vec<RelicId> {
    let mut ids = Vec::new();
    for id in list(value).iter().filter_map(|v| RelicId::parse(v.as_str()?)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

pub fn parse_meta(raw: &Value) -> Meta {
    let mut meta = Meta::default();
    let Value::Object(data) = raw else { return meta };
    let field = |name: &str| data.get(name).and_then(count);
    meta.crowns = field("crowns").unwrap_or(0);
    meta.best = field("best").unwrap_or(0);
    meta.runs = field("runs").unwrap_or(0);
    if let Some(Value::Object(upgrades)) = data.get("upgrades") {
        for (key, level) in upgrades {
            if let (Some(id), Some(level @ 1..)) = (UpgradeId::parse(key), count(level)) {
                meta.upgrades.insert(id, level);
            }
        }
    }
    meta
}

pub fn parse_offer(raw: &Value) -> Option<Offer> {
    let Value::Object(o) = raw else { return None };
    match o.get("kind")?.as_str()? {
        "piece" => kind(o.get("type")?).filter(|&k| k != Kind::King).map(Offer::Piece),
        "relic" => RelicId::parse(o.get("id")?.as_str()?).map(Offer::Relic),
        "gold" => count(o.get("amount")?).map(Offer::Gold),
        _ => None,
    }
}

fn offers(value: &Value) -> Vec<Offer> {
    list(value).iter().filter_map(parse_offer).collect()
}

/// The run of the data, or why it is not a run that the game can continue. The checks of
/// the fields and of the board (see the module comment).
pub fn parse_run(raw: &Value) -> Result<Run, String> {
    let run = parse_run_shape(raw).ok_or("The data does not have the fields of a run")?;
    Battle::new(&run)?;
    Ok(run)
}

/// The checks of the fields, and the limits of ids and of the enemy army.
fn parse_run_shape(raw: &Value) -> Option<Run> {
    let Value::Object(data) = raw else { return None };
    let Some(Value::Object(enemy)) = data.get("enemy") else { return None };
    let floor = count(data.get("floor")?)?;
    let gold = count(data.get("gold")?)?;
    let next_id = count(data.get("nextId")?)?;
    if floor < 1 || floor > FLOORS.len() as u64 || next_id > UNIT_ID_MAX as u64 + 1 {
        return None;
    }
    // A field that the data does not have is 0. A field that is not valid makes the run not valid.
    let or_zero = |name: &str, max: u64| match data.get(name) {
        None => Some(0),
        Some(value) => count(value).filter(|&n| n <= max),
    };
    let seed = or_zero("seed", SEED_MAX)?;
    let rolls = or_zero("rolls", u32::MAX as u64)? as u32;

    let mut army = Vec::new();
    let mut ids = HashSet::new();
    for unit in list(data.get("army").unwrap_or(&Value::Null)) {
        let Value::Object(unit) = unit else { return None };
        let id = count(unit.get("id")?).filter(|&id| id <= UNIT_ID_MAX as u64)? as UnitId;
        let kind = kind(unit.get("type")?)?;
        let home = square(unit.get("home")?, 15)?;
        if !ids.insert(id) {
            return None;
        }
        army.push(Unit { id, kind, home });
    }
    let mut pieces = Vec::new();
    for piece in list(enemy.get("pieces").unwrap_or(&Value::Null)) {
        let Value::Object(piece) = piece else { return None };
        pieces.push(EnemyPiece { kind: kind(piece.get("type")?)?, square: square(piece.get("square")?, 63)? });
    }
    let one_king = |kinds: &mut dyn Iterator<Item = Kind>| kinds.filter(|&k| k == Kind::King).count() == 1;
    let homes: HashSet<Square> = army.iter().map(|u| u.home).collect();
    if !one_king(&mut army.iter().map(|u| u.kind))
        || !one_king(&mut pieces.iter().map(|p| p.kind))
        || homes.len() != army.len()
        || pieces.len() > ENEMY_PIECES_MAX
    {
        return None;
    }

    let mut owned = relics(data.get("relics").unwrap_or(&Value::Null));
    owned.truncate(RELICS_MAX);
    let mut traits = relics(enemy.get("traits").unwrap_or(&Value::Null));
    traits.retain(|id| id.is_trait());
    traits.truncate(TRAITS_MAX);
    let draft = match data.get("draft") {
        None | Some(Value::Null) => None,
        Some(value) => Some(offers(value)),
    };
    Some(Run {
        seed,
        floor: floor as usize,
        gold,
        army,
        next_id: next_id as UnitId,
        relics: owned,
        enemy: Enemy { pieces, traits },
        phase: if data.get("phase").and_then(Value::as_str) == Some("camp") { Phase::Camp } else { Phase::Battle },
        draft,
        shop: offers(data.get("shop").unwrap_or(&Value::Null)),
        rolls,
    })
}
