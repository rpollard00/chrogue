//! Helpers that the test files share.
#![allow(dead_code)]

use chrogue_engine::fen::square;
use chrogue_engine::{Move, MoveList, Square, State, legal_moves, moves_from};

pub fn legal(state: &mut State) -> Vec<Move> {
    let mut list = MoveList::new();
    legal_moves(state, &mut list);
    list.as_slice().to_vec()
}

/// The sorted target squares of the legal moves from a square, for the side that has the move.
pub fn targets(state: &mut State, from: &str) -> Vec<Square> {
    let from = square(from);
    let mut to: Vec<Square> = legal(state).iter().filter(|m| m.from == from).map(|m| m.to).collect();
    to.sort_unstable();
    to
}

/// The sorted target squares of `moves_from`.
pub fn targets_from(state: &State, from: &str) -> Vec<Square> {
    let mut list = MoveList::new();
    moves_from(state, square(from), &mut list);
    let mut to: Vec<Square> = list.iter().map(|m| m.to).collect();
    to.sort_unstable();
    to
}

pub fn squares(names: &[&str]) -> Vec<Square> {
    let mut list: Vec<Square> = names.iter().map(|name| square(name)).collect();
    list.sort_unstable();
    list
}
