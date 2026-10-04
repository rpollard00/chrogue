//! The result of a battle.

use crate::movegen::{in_check, is_legal, legal_moves, pseudo_moves};
use crate::state::State;
use crate::types::{Color, MoveList};

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

    /// The name of the reason in the protocol.
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
    let color = state.turn();
    let mut list = MoveList::new();
    pseudo_moves(state, color, false, &mut list);
    list.iter().any(|&m| is_legal(state, m, color))
}

/// The result that comes from the pieces only: bare kings or a rout. This test is cheap.
/// The search uses it at each node and finds "no legal move" in its own move loop.
#[inline]
pub fn material_outcome(state: &State) -> Option<Outcome> {
    match (state.men(Color::White) != 0, state.men(Color::Black) != 0) {
        (false, false) => Some(Outcome::Bare),
        (true, false) => Some(Outcome::Rout { winner: Color::White }),
        (false, true) => Some(Outcome::Rout { winner: Color::Black }),
        (true, true) => None,
    }
}

/// Returns None while the battle continues. The order of the checks is: bare, rout,
/// checkmate or stalemate, clock.
pub fn outcome(state: &mut State) -> Option<Outcome> {
    if let Some(end) = material_outcome(state) {
        return Some(end);
    }
    if !has_legal_move(state) {
        let winner = state.turn().other();
        return Some(if in_check(state, state.turn()) {
            Outcome::Checkmate { winner }
        } else {
            Outcome::Stalemate { winner }
        });
    }
    (state.clock >= CLOCK_LIMIT).then_some(Outcome::Clock)
}

/// True if the side that has the move wins in `moves` of its moves or fewer against each
/// defense: a checkmate, a stalemate, or a rout. The state is the same after the call.
///
/// The function searches each line with no pruning. It is for a small number of moves.
pub fn wins_in(state: &mut State, moves: u32) -> bool {
    if moves == 0 {
        return false;
    }
    let us = state.turn();
    let mut list = MoveList::new();
    legal_moves(state, &mut list);
    list.iter().any(|&m| {
        let undo = state.make(m);
        let win = match outcome(state) {
            Some(end) => end.winner() == Some(us),
            None => moves > 1 && each_reply_loses(state, moves - 1),
        };
        state.unmake(m, undo);
        win
    })
}

/// True if the side that does not have the move wins in `moves` moves or fewer after each
/// legal move. The battle continues, thus the side that has the move has a legal move.
fn each_reply_loses(state: &mut State, moves: u32) -> bool {
    let mut list = MoveList::new();
    legal_moves(state, &mut list);
    list.iter().all(|&reply| {
        let undo = state.make(reply);
        let lost = outcome(state).is_none() && wins_in(state, moves);
        state.unmake(reply, undo);
        lost
    })
}
