//! Perft: the number of move sequences of a given length. The tests compare it with the known
//! counts of chess positions.

use crate::movegen::{in_check, pseudo_moves};
use crate::state::State;
use crate::types::MoveList;

/// The number of legal move sequences of `depth` half moves. The state is the same after the call.
pub fn perft(state: &mut State, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let color = state.turn();
    let mut list = MoveList::new();
    pseudo_moves(state, color, false, &mut list);
    let mut nodes = 0;
    for &m in &list {
        let undo = state.make(m);
        if !in_check(state, color) {
            nodes += if depth == 1 { 1 } else { perft(state, depth - 1) };
        }
        state.unmake(m, undo);
    }
    nodes
}
