//! The chess rules of Chrogue (move generation, make and unmake, the result of a battle) and
//! the enemy AI (`eval`, `search`, `level`).
//!
//! The movement rules are data (`rules`). `tables` changes the data into lookup tables one
//! time for each battle. `movegen` reads only the tables, thus a new movement rule needs no
//! new engine code.

pub mod eval;
pub mod fen;
pub mod level;
pub mod movegen;
pub mod outcome;
pub mod perft;
pub mod reference;
pub mod rng;
pub mod rules;
pub mod search;
pub mod state;
pub mod tables;
pub mod types;
pub mod zobrist;

pub use eval::{EvalTables, EvalVariant, Evaluator};
pub use level::{Brain, Level, choose_move};
pub use movegen::{in_check, is_attacked, is_legal, legal_moves, moves_from, pseudo_moves};
pub use outcome::{CLOCK_LIMIT, Outcome, has_legal_move, material_outcome, outcome};
pub use perft::perft;
pub use rules::{Atom, DoubleStep, KindRules, Mode, Offset, PawnRules, Rules, RulesError, SideRules};
pub use search::{Limits, MATE, MATE_BOUND, SearchOptions, SearchResult, search};
pub use state::{State, Undo};
pub use tables::Tables;
pub use types::{Bitboard, Color, Kind, Move, MoveList, Piece, Placement, Special, Square};
