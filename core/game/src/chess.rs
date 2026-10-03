//! The one module that calls the chess engine. The rest of the crate uses these functions and
//! types only, thus a change of the engine API changes this file only.

use chrogue_engine as engine;
pub use engine::rng::{Rng, mix};
pub use engine::rules::{CAMEL, KNIGHT, ORTHO};
pub use engine::{Atom, Color, Kind, Mode, Move, Offset, Outcome, Piece, Placement, SideRules, Special, Square};

/// The state of a battle in the engine.
pub type State = engine::State;

/// Makes a battle with White to move. Returns an error if the rules are not valid.
pub fn new_state(pieces: &[Placement], white: SideRules, black: SideRules) -> Result<State, String> {
    let rules = engine::Rules::new(white, black);
    rules.validate().map_err(|error| error.to_string())?;
    State::new(pieces, rules).map_err(|error| error.to_string())
}

pub fn turn(state: &State) -> Color {
    state.turn()
}

pub fn piece_at(state: &State, square: Square) -> Option<Piece> {
    state.piece_at(square)
}

/// The movement rules of one side of a battle.
pub fn side_rules(state: &State, color: Color) -> &SideRules {
    state.rules().side(color)
}

/// The pieces on the board with their squares, from a1 to h8.
pub fn pieces(state: &State) -> Vec<(Square, Piece)> {
    (0..64).filter_map(|s| state.piece_at(s).map(|p| (s, p))).collect()
}

/// The number of half moves with no capture.
pub fn clock(state: &State) -> u32 {
    state.clock
}

/// The legal moves of the side that has the move.
pub fn legal_moves(state: &mut State) -> Vec<Move> {
    let mut list = engine::MoveList::new();
    engine::legal_moves(state, &mut list);
    list.as_slice().to_vec()
}

/// The legal moves of the piece on a square, as if its side has the move.
pub fn moves_from(state: &State, square: Square) -> Vec<Move> {
    let mut list = engine::MoveList::new();
    engine::moves_from(state, square, &mut list);
    list.as_slice().to_vec()
}

/// The piece that a move captured, and its square.
pub struct Played {
    pub captured: Option<(Piece, Square)>,
}

/// Plays a legal move.
pub fn play(state: &mut State, m: Move) -> Played {
    let undo = state.make(m);
    Played { captured: undo.captured.map(|piece| (piece, undo.captured_square)) }
}

pub fn outcome(state: &mut State) -> Option<Outcome> {
    engine::outcome(state)
}

/// The square of the king of the side that has the move, if that king is in check.
pub fn check_square(state: &State) -> Option<Square> {
    let color = state.turn();
    if engine::in_check(state, color) { state.king_square(color) } else { None }
}

/// True if the king of the side is in check. A side with no king is never in check.
pub fn in_check(state: &State, color: Color) -> bool {
    engine::in_check(state, color)
}

/// The number of AI levels.
pub const LEVELS: usize = engine::Level::LADDER.len();

/// The move of an AI level (1 to `LEVELS`) for the side that has the move.
pub fn ai_move(state: &mut State, level: usize, seed: u64) -> Option<Move> {
    let level = engine::Level::LADDER[level.clamp(1, LEVELS) - 1];
    engine::choose_move(state, &level, seed).map(|result| result.mv)
}

/// The name of an AI level.
pub fn level_name(level: usize) -> &'static str {
    engine::Level::LADDER[level.clamp(1, LEVELS) - 1].name
}

pub const fn kind_letter(kind: Kind) -> char {
    kind.letter()
}

pub const fn kind_from_letter(letter: char) -> Option<Kind> {
    Kind::from_letter(letter)
}

pub const fn color_letter(color: Color) -> &'static str {
    match color {
        Color::White => "w",
        Color::Black => "b",
    }
}

pub const fn special_name(special: Special) -> &'static str {
    match special {
        Special::None => "none",
        Special::DoubleStep => "double_step",
        Special::EnPassant => "en_passant",
        Special::Castle => "castle",
    }
}

/// The partner move of a castle of the side that made it: (from, to). None if the move is not a castle.
pub fn castle_partner(state: &State, color: Color, m: Move) -> Option<(Square, Square)> {
    state.rules().castle_partner(color, m)
}
