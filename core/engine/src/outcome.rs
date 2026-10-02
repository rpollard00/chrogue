//! The result of a battle.

use crate::movegen::{in_check, is_legal, pseudo_moves};
use crate::state::State;
use crate::types::{Color, Kind, MoveList};

/// A battle ends when the clock reaches this number of half moves.
pub const CLOCK_LIMIT: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Each side has only its king, or no piece. The battle is a draw.
    Bare,
    /// The loser has only its king, or no piece.
    Rout { winner: Color },
    /// The loser has no legal move and its king is in check.
    Checkmate { winner: Color },
    /// The loser has no legal move and its king is not in check.
    Stalemate { winner: Color },
    /// The clock reached `CLOCK_LIMIT`. The battle is a draw.
    Clock,
}

impl Outcome {
    pub const fn winner(self) -> Option<Color> {
        match self {
            Outcome::Rout { winner } | Outcome::Checkmate { winner } | Outcome::Stalemate { winner } => Some(winner),
            Outcome::Bare | Outcome::Clock => None,
        }
    }

    /// The name of the reason in the TypeScript engine.
    pub const fn reason(self) -> &'static str {
        match self {
            Outcome::Bare => "bare",
            Outcome::Rout { .. } => "rout",
            Outcome::Checkmate { .. } => "checkmate",
            Outcome::Stalemate { .. } => "stalemate",
            Outcome::Clock => "clock",
        }
    }
}

/// True if the side that has the move has one legal move or more.
pub fn has_legal_move(state: &mut State) -> bool {
    let color = state.turn;
    let mut list = MoveList::new();
    pseudo_moves(state, color, false, &mut list);
    list.iter().any(|&m| is_legal(state, m, color))
}

/// Returns None while the battle continues. The order of the checks is: bare, rout,
/// checkmate or stalemate, clock.
pub fn outcome(state: &mut State) -> Option<Outcome> {
    let men = |color: Color| state.color_set(color) & !state.kind_set(Kind::King) != 0;
    match (men(Color::White), men(Color::Black)) {
        (false, false) => return Some(Outcome::Bare),
        (true, false) => return Some(Outcome::Rout { winner: Color::White }),
        (false, true) => return Some(Outcome::Rout { winner: Color::Black }),
        (true, true) => {}
    }
    if !has_legal_move(state) {
        let winner = state.turn.other();
        return Some(if in_check(state, state.turn) {
            Outcome::Checkmate { winner }
        } else {
            Outcome::Stalemate { winner }
        });
    }
    (state.clock >= CLOCK_LIMIT).then_some(Outcome::Clock)
}
