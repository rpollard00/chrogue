//! The Rust side of the differential test. `core/difftest/run.ts` is the other side.
//!
//! The tool reads one JSON request from each line of stdin and writes one JSON answer to
//! each line of stdout. The JSON shapes are those of the TypeScript engine (`src/engine/types.ts`).
//!
//! Requests:
//! - `{"op":"analyze","state":S,"rules":R,"perft":[depth, ...]}` gives the moves, the checks,
//!   the attacked squares, the outcome, and the perft counts of a state.
//! - `{"op":"playout","state":S,"rules":R,"moves":[M, ...]}` plays the moves and gives the
//!   state after each move. After each move and after each take-back, it compares the bitboards
//!   with the mailbox and the Zobrist key with the key from all the pieces. It also compares
//!   the state after each take-back with the state before that move.
//!
//! `S` is `{"board":[null | {"id","type","color","moved"}, ...64],"turn","ep","clock"}`.
//! `R` is `{"w":[flag, ...],"b":[flag, ...]}` with the flag names of `MoveRules`.
//!
//! The game gives a piece a number or a string as its id (`PieceId` in `src/engine/types.ts`),
//! for example `3`, `"e0"`, and `"conscript"`. The engine has `u16` ids. `Ids` gives each id
//! of a request its own number, and changes the numbers back to the ids in the answer.

use std::io::{BufRead, BufWriter, Write};

use chrogue_engine::zobrist::key_from_scratch;
use chrogue_engine::{
    Color, Kind, Move, MoveList, Outcome, Piece, Placement, Rules, SideRules, Special, Square, State, in_check,
    is_attacked, legal_moves, moves_from, outcome, perft, pseudo_moves,
};
use serde_json::{Map, Value, json};

type Res<T> = Result<T, String>;

fn main() {
    let stdin = std::io::stdin().lock();
    let mut stdout = BufWriter::new(std::io::stdout().lock());
    for (index, line) in stdin.lines().enumerate() {
        let line = line.expect("stdin must be text");
        if line.trim().is_empty() {
            continue;
        }
        let answer = answer(&line).unwrap_or_else(|error| json!({ "error": format!("line {}: {error}", index + 1) }));
        writeln!(stdout, "{answer}").expect("stdout must accept the answer");
    }
}

/// The ids of the pieces of one request. The number of an id is its index in the list.
#[derive(Default)]
struct Ids {
    names: Vec<Value>,
}

impl Ids {
    fn number(&mut self, id: &Value) -> Res<u16> {
        if !(id.is_string() || id.is_u64()) {
            return Err(format!("the piece id {id} is not a string or a whole number"));
        }
        let index = match self.names.iter().position(|name| name == id) {
            Some(index) => index,
            None => {
                self.names.push(id.clone());
                self.names.len() - 1
            }
        };
        u16::try_from(index).map_err(|_| "the request has too many piece ids".to_string())
    }

    fn name(&self, number: u16) -> Value {
        self.names.get(number as usize).cloned().unwrap_or(Value::Null)
    }
}

fn answer(line: &str) -> Res<Value> {
    let request: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let mut ids = Ids::default();
    let mut state = read_state(&request["state"], read_rules(&request["rules"])?, &mut ids)?;
    match request["op"].as_str() {
        Some("analyze") => analyze(&mut state, &request["perft"]),
        Some("playout") => playout(&mut state, &request["moves"], &ids),
        _ => Err("\"op\" must be \"analyze\" or \"playout\"".to_string()),
    }
}

fn analyze(state: &mut State, depths: &Value) -> Res<Value> {
    let mut list = MoveList::new();
    legal_moves(state, &mut list);
    let moves = write_moves(&list);

    let mut from_each = Map::new();
    for s in 0..64u8 {
        if state.piece_at(s).is_some() {
            list.clear();
            moves_from(state, s, &mut list);
            from_each.insert(s.to_string(), write_moves(&list));
        }
    }

    let mut pseudo = Map::new();
    let mut attacked = Map::new();
    let mut check = Map::new();
    for color in Color::ALL {
        let mut lists = Map::new();
        for (name, captures_only) in [("all", false), ("captures", true)] {
            list.clear();
            pseudo_moves(state, color, captures_only, &mut list);
            lists.insert(name.to_string(), write_moves(&list));
        }
        pseudo.insert(color_name(color).to_string(), Value::Object(lists));
        // One character for each square: 1 if this color attacks the square.
        let squares: String = (0..64u8).map(|s| if is_attacked(state, s, color) { '1' } else { '0' }).collect();
        attacked.insert(color_name(color).to_string(), Value::String(squares));
        check.insert(color_name(color).to_string(), Value::Bool(in_check(state, color)));
    }

    let mut counts = Vec::new();
    for depth in depths.as_array().map(Vec::as_slice).unwrap_or_default() {
        let depth = depth.as_u64().ok_or("a perft depth must be a number")? as u32;
        counts.push(perft(state, depth));
    }

    Ok(json!({
        "moves": moves,
        "movesFrom": from_each,
        "pseudo": pseudo,
        "attacked": attacked,
        "inCheck": check,
        "outcome": write_outcome(outcome(state)),
        "perft": counts,
    }))
}

/// True if the bitboards agree with the mailbox and the Zobrist key is the key of the pieces.
fn consistent(state: &State) -> bool {
    state.is_consistent() && state.key() == key_from_scratch(state)
}

fn playout(state: &mut State, moves: &Value, ids: &Ids) -> Res<Value> {
    let mut before = Vec::new();
    let mut states = Vec::new();
    let mut undos = Vec::new();
    // The first move after which the state was not consistent, from 1. 0 is the start.
    let mut inconsistent: Option<usize> = (!consistent(state)).then_some(0);
    for (index, value) in moves.as_array().ok_or("\"moves\" must be a list")?.iter().enumerate() {
        let m = read_move(value)?;
        if state.piece_at(m.from).is_none() {
            return Err(format!("the move {value} starts on an empty square"));
        }
        before.push(state.clone());
        undos.push((m, state.make(m)));
        states.push(write_state(state, ids));
        if inconsistent.is_none() && !consistent(state) {
            inconsistent = Some(index + 1);
        }
    }
    // The first move, from 1, that its take-back did not undo.
    let mut not_restored: Option<usize> = None;
    for (index, (m, undo)) in undos.into_iter().enumerate().rev() {
        state.unmake(m, undo);
        if *state != before[index] || !consistent(state) {
            not_restored = Some(index + 1);
        }
    }
    Ok(json!({ "states": states, "inconsistentAfter": inconsistent, "notRestored": not_restored }))
}

fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "w",
        Color::Black => "b",
    }
}

fn read_color(value: &Value) -> Res<Color> {
    match value.as_str() {
        Some("w") => Ok(Color::White),
        Some("b") => Ok(Color::Black),
        _ => Err(format!("{value} is not a color")),
    }
}

fn read_kind(value: &Value) -> Res<Kind> {
    let letter = value.as_str().and_then(|text| text.chars().next());
    letter.and_then(Kind::from_letter).ok_or_else(|| format!("{value} is not a piece type"))
}

fn read_square(value: &Value) -> Res<Square> {
    value.as_u64().filter(|&s| s < 64).map(|s| s as Square).ok_or_else(|| format!("{value} is not a square"))
}

fn read_rules(value: &Value) -> Res<Rules> {
    let side = |color: &str| -> Res<SideRules> {
        let flags = value[color].as_array().map(Vec::as_slice).unwrap_or_default();
        let names = flags.iter().map(|flag| flag.as_str().ok_or("a rule flag must be a string"));
        let names = names.collect::<Result<Vec<_>, _>>()?;
        SideRules::from_flags(names).map_err(|error| error.to_string())
    };
    Ok(Rules::new(side("w")?, side("b")?))
}

fn read_state(value: &Value, rules: Rules, ids: &mut Ids) -> Res<State> {
    let board = value["board"].as_array().filter(|board| board.len() == 64).ok_or("\"board\" must have 64 items")?;
    let mut pieces = Vec::new();
    for (square, entry) in board.iter().enumerate() {
        if entry.is_null() {
            continue;
        }
        let piece = Piece {
            id: ids.number(&entry["id"])?,
            kind: read_kind(&entry["type"])?,
            color: read_color(&entry["color"])?,
            moved: entry["moved"].as_bool().ok_or("\"moved\" must be true or false")?,
        };
        pieces.push(Placement { piece, square: square as Square });
    }
    let ep = match value["ep"].as_i64() {
        Some(-1) => None,
        _ => Some(read_square(&value["ep"])?),
    };
    let state = State::new(&pieces, rules).map_err(|error| error.to_string())?;
    let mut state =
        state.with_turn(read_color(&value["turn"])?).with_en_passant(ep).map_err(|error| error.to_string())?;
    state.clock = value["clock"].as_u64().ok_or("\"clock\" must be a number")? as u32;
    Ok(state)
}

fn write_state(state: &State, ids: &Ids) -> Value {
    let board: Vec<Value> = state
        .board()
        .iter()
        .map(|entry| match entry {
            Some(piece) => json!({
                "id": ids.name(piece.id),
                "type": piece.kind.letter().to_string(),
                "color": color_name(piece.color),
                "moved": piece.moved,
            }),
            None => Value::Null,
        })
        .collect();
    json!({
        "board": board,
        "turn": color_name(state.turn()),
        "ep": state.ep().map_or(-1, i64::from),
        "clock": state.clock,
    })
}

/// Changes a move of the TypeScript engine into a move of this engine. The two forms hold
/// the same data: `ep` and `castle` come from the `from` and `to` squares.
fn read_move(value: &Value) -> Res<Move> {
    let from = read_square(&value["from"])?;
    let to = read_square(&value["to"])?;
    let promo = if value["promo"].is_null() { None } else { Some(read_kind(&value["promo"])?) };
    let flags = [!value["ep"].is_null(), value["epCapture"] == true, value["back"] == true, !value["castle"].is_null()];
    let special = match flags {
        [false, false, false, false] => Special::None,
        [true, false, false, false] => Special::DoubleStep,
        [false, true, false, false] => Special::EnPassant,
        [false, false, true, false] => Special::Backward,
        [false, false, false, true] => Special::Castle,
        _ => return Err(format!("the move {value} has more than one special property")),
    };
    let m = Move { from, to, promo, special };
    if special == Special::DoubleStep && value["ep"] != m.crossed_square() {
        return Err(format!("the en passant square of {value} is not between the two squares"));
    }
    if special == Special::Castle {
        let (rook_from, rook_to) = m.castle_rook();
        if value["castle"] != json!([rook_from, rook_to]) {
            return Err(format!("the rook move of {value} is not the rook move of a castle"));
        }
    }
    Ok(m)
}

fn write_move(m: Move) -> Value {
    let mut out = Map::new();
    out.insert("from".to_string(), json!(m.from));
    out.insert("to".to_string(), json!(m.to));
    if let Some(kind) = m.promo {
        out.insert("promo".to_string(), json!(kind.letter().to_string()));
    }
    match m.special {
        Special::None => {}
        Special::DoubleStep => drop(out.insert("ep".to_string(), json!(m.crossed_square()))),
        Special::EnPassant => drop(out.insert("epCapture".to_string(), json!(true))),
        Special::Backward => drop(out.insert("back".to_string(), json!(true))),
        Special::Castle => {
            let (rook_from, rook_to) = m.castle_rook();
            out.insert("castle".to_string(), json!([rook_from, rook_to]));
        }
    }
    Value::Object(out)
}

fn write_moves(list: &MoveList) -> Value {
    Value::Array(list.iter().map(|&m| write_move(m)).collect())
}

fn write_outcome(result: Option<Outcome>) -> Value {
    match result {
        Some(result) => json!({ "winner": result.winner().map(color_name), "reason": result.reason() }),
        None => Value::Null,
    }
}
