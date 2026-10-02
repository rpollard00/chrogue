//! The chess rules of Chrogue: move generation, make and unmake, and the result of a battle.
//!
//! The movement rules are data (`rules`). `tables` changes the data into lookup tables one
//! time for each battle. `movegen` reads only the tables, thus a new movement rule needs no
//! new engine code.

pub mod fen;
pub mod movegen;
pub mod outcome;
pub mod perft;
pub mod rules;
pub mod state;
pub mod tables;
pub mod types;

pub use movegen::{in_check, is_attacked, is_legal, legal_moves, moves_from, pseudo_moves};
pub use outcome::{CLOCK_LIMIT, Outcome, has_legal_move, outcome};
pub use perft::perft;
pub use rules::{Atom, DoubleStep, KindRules, Mode, Offset, PawnRules, Rules, RulesError, SideRules};
pub use state::{State, Undo};
pub use tables::Tables;
pub use types::{Bitboard, Color, Kind, Move, MoveList, Piece, Placement, Special, Square};
