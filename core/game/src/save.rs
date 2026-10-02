//! Saved data. A `Storage` keeps two documents: the meta (crowns and upgrades) and the run in
//! progress. This module writes them as versioned JSON and checks the data that it reads, as
//! `src/game/storage.ts` does: unknown relic and upgrade ids are dropped, and a run that the game
//! cannot continue loads as no run.
//!
//! The `data` field of a document has the shape of the TypeScript saved data, thus a saved run of
//! the browser game can be put in a document as it is.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::chess::{self, Kind, Square};
use crate::content::{FLOORS, RelicId, UpgradeId};
use crate::run::{ENEMY_PIECES_MAX, Enemy, EnemyPiece, Meta, Offer, Phase, Run, UNIT_ID_MAX, Unit, UnitId};

/// The version of the saved documents.
pub const SAVE_VERSION: u64 = 1;
const META_FORMAT: &str = "chrogue.meta";
const RUN_FORMAT: &str = "chrogue.run";

/// The place where the saved documents are. A document is a JSON text.
pub trait Storage {
    fn load_meta(&mut self) -> Option<String>;
    fn save_meta(&mut self, text: &str) -> Result<(), String>;
    fn load_run(&mut self) -> Option<String>;
    fn save_run(&mut self, text: &str) -> Result<(), String>;
    fn clear_run(&mut self) -> Result<(), String>;
}

/// A storage in memory, for tests and for `--no-save`. A clone shares nothing.
#[derive(Clone, Debug, Default)]
pub struct MemoryStorage {
    pub meta: Option<String>,
    pub run: Option<String>,
}

impl Storage for MemoryStorage {
    fn load_meta(&mut self) -> Option<String> {
        self.meta.clone()
    }

    fn save_meta(&mut self, text: &str) -> Result<(), String> {
        self.meta = Some(text.to_string());
        Ok(())
    }

    fn load_run(&mut self) -> Option<String> {
        self.run.clone()
    }

    fn save_run(&mut self, text: &str) -> Result<(), String> {
        self.run = Some(text.to_string());
        Ok(())
    }

    fn clear_run(&mut self) -> Result<(), String> {
        self.run = None;
        Ok(())
    }
}

/// A storage in a directory: `meta.json` and `run.json`. A save writes a temporary file and
/// then renames it, thus a crash does not leave half a document.
#[derive(Clone, Debug)]
pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    pub fn new(dir: impl Into<PathBuf>) -> FileStorage {
        FileStorage { dir: dir.into() }
    }

    fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(name)).ok()
    }

    fn write(&self, name: &str, text: &str) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let path = self.dir.join(name);
        let temp = self.dir.join(format!("{name}.tmp"));
        fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
        fs::rename(&temp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl Storage for FileStorage {
    fn load_meta(&mut self) -> Option<String> {
        self.read("meta.json")
    }

    fn save_meta(&mut self, text: &str) -> Result<(), String> {
        self.write("meta.json", text)
    }

    fn load_run(&mut self) -> Option<String> {
        self.read("run.json")
    }

    fn save_run(&mut self, text: &str) -> Result<(), String> {
        self.write("run.json", text)
    }

    fn clear_run(&mut self) -> Result<(), String> {
        match fs::remove_file(self.dir.join("run.json")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

// ---- Documents ----

fn document(format: &str, data: Value) -> String {
    json!({ "format": format, "version": SAVE_VERSION, "data": data }).to_string()
}

/// The data of a document, or Null if the text is not a document of this format and version.
fn open(text: Option<String>, format: &str) -> Value {
    let Some(Ok(Value::Object(mut doc))) = text.map(|t| serde_json::from_str::<Value>(&t)) else { return Value::Null };
    if doc.get("format") != Some(&Value::from(format)) || doc.get("version") != Some(&Value::from(SAVE_VERSION)) {
        return Value::Null;
    }
    doc.remove("data").unwrap_or(Value::Null)
}

pub fn load_meta(storage: &mut dyn Storage) -> Meta {
    parse_meta(&open(storage.load_meta(), META_FORMAT))
}

pub fn load_run(storage: &mut dyn Storage) -> Option<Run> {
    parse_run(&open(storage.load_run(), RUN_FORMAT))
}

pub fn meta_document(meta: &Meta) -> String {
    document(META_FORMAT, meta_json(meta))
}

pub fn run_document(run: &Run) -> String {
    document(RUN_FORMAT, run_json(run))
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
    })
}

// ---- Reading ----

/// A whole number that is 0 or more (`isCount`). A JSON number such as 4.0 counts.
fn count(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_u64().or_else(|| {
            let f = n.as_f64()?;
            (f >= 0.0 && f.fract() == 0.0 && f <= 9_007_199_254_740_991.0).then_some(f as u64)
        }),
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

/// Returns None if the data is not a run that the game can continue.
///
/// The checks of `parseRun`, and three more: a unit id is at most `UNIT_ID_MAX` and appears one
/// time, `nextId` is at most `UNIT_ID_MAX + 1`, and the enemy has at most `ENEMY_PIECES_MAX` pieces.
pub fn parse_run(raw: &Value) -> Option<Run> {
    let Value::Object(data) = raw else { return None };
    let Some(Value::Object(enemy)) = data.get("enemy") else { return None };
    let floor = count(data.get("floor")?)?;
    let gold = count(data.get("gold")?)?;
    let next_id = count(data.get("nextId")?)?;
    if floor < 1 || floor > FLOORS.len() as u64 || next_id > UNIT_ID_MAX as u64 + 1 {
        return None;
    }

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

    let draft = match data.get("draft") {
        None | Some(Value::Null) => None,
        Some(value) => Some(offers(value)),
    };
    Some(Run {
        floor: floor as usize,
        gold,
        army,
        next_id: next_id as UnitId,
        relics: relics(data.get("relics").unwrap_or(&Value::Null)),
        enemy: Enemy { pieces, traits: relics(enemy.get("traits").unwrap_or(&Value::Null)) },
        phase: if data.get("phase").and_then(Value::as_str) == Some("camp") { Phase::Camp } else { Phase::Battle },
        draft,
        shop: offers(data.get("shop").unwrap_or(&Value::Null)),
    })
}
