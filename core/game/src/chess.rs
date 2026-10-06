//! The one module that calls the chess engine. The rest of the crate uses these functions and
//! types only, thus a change of the engine API changes this file only.

use std::sync::Arc;

use chrogue_engine as engine;
pub use engine::rng::{Rng, mix};
pub use engine::rules::{ALFIL, CAMEL, DABBABA, DIAG, FORWARD, FORWARD_DIAG, KING, KNIGHT, ORTHO};
pub use engine::{
    Atom, Aura, Boon, Color, Hook, Kind, Mode, Move, Offset, Outcome, Piece, Placement, Shield, SideRules, Special,
    Square,
};

/// The state of a battle in the engine.
pub type State = engine::State;
/// The lookup tables of the rules of a battle. The states of one battle share them.
pub type Tables = Arc<engine::Tables>;

/// The tables of the rules of the two sides. Returns an error if the rules are not valid.
pub fn tables(white: SideRules, black: SideRules) -> Result<Tables, String> {
    let rules = engine::Rules::new(white, black);
    rules.validate().map_err(|error| error.to_string())?;
    engine::Tables::new(rules).map(Arc::new).map_err(|error| error.to_string())
}

/// Makes a battle with White to move.
pub fn new_state(pieces: &[Placement], tables: &Tables) -> Result<State, String> {
    State::with_tables(pieces, tables.clone()).map_err(|error| error.to_string())
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

/// An aura of a side on the board now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AuraNow {
    /// The ids of the pieces of the source kind, from the smallest id.
    pub sources: Vec<u16>,
    /// The squares of the pieces that have the boon, from a1 to h8.
    pub holders: Vec<Square>,
    /// The squares where the aura shows, from a1 to h8: the squares of the sources, and the
    /// squares in the range of a source that are empty or have a piece with the boon.
    pub zone: Vec<Square>,
}

/// An aura of a side on the board now.
pub fn aura_now(state: &State, color: Color, aura: &Aura) -> AuraNow {
    let squares = |set: u64| (0..64).filter(|&s| set & (1 << s) != 0).collect::<Vec<Square>>();
    let holders = engine::aura_holders(state, color, aura);
    let empty = !(state.color_set(Color::White) | state.color_set(Color::Black));
    let source_set = state.pieces(color, aura.source);
    let mut sources: Vec<u16> =
        squares(source_set).into_iter().filter_map(|s| state.piece_at(s)).map(|p| p.id).collect();
    sources.sort_unstable();
    AuraNow {
        sources,
        holders: squares(holders),
        zone: squares(source_set | engine::aura_zone(state, color, aura) & (empty | holders)),
    }
}

pub const fn boon_name(boon: Boon) -> &'static str {
    match boon {
        Boon::Shield => "shield",
        Boon::Moves => "moves",
    }
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

/// True if the side that has the move wins in `moves` of its moves or fewer against each defense.
pub fn wins_in(state: &mut State, moves: u32) -> bool {
    engine::wins_in(state, moves)
}

/// The number of AI levels.
pub const LEVELS: usize = engine::Level::LADDER.len();

/// The move of an AI level (1 to `LEVELS`) for the side that has the move.
pub fn ai_move(state: &mut State, level: usize, seed: u64) -> Option<Move> {
    let level = engine::Level::number(level.clamp(1, LEVELS));
    engine::choose_move(state, &level, seed).map(|result| result.mv)
}

/// The name of an AI level.
pub fn level_name(level: usize) -> &'static str {
    engine::Level::number(level.clamp(1, LEVELS)).name
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
