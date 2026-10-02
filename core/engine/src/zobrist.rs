//! Zobrist keys. `State` keeps the key of its pieces up to date in `make` and `unmake`.
//!
//! The key of a state has a part for each piece (its kind, color, square, and `moved` flag),
//! a part for the side that has the move, and a part for the en passant square. The `moved`
//! flag is in the key only for the kinds whose moves can read it: the pawn (double step),
//! and the king and the rook (castling). The clock is not in the key.

use crate::rng::split_mix;
use crate::state::State;
use crate::types::{Color, Kind, Piece, Square};

struct Keys {
    /// `piece[color][kind][moved][square]`
    piece: [[[[u64; 64]; 2]; Kind::COUNT]; 2],
    side: u64,
    ep: [u64; 64],
    clock: [u64; 128],
}

const fn moved_matters(kind: usize) -> bool {
    kind == Kind::Pawn as usize || kind == Kind::Rook as usize || kind == Kind::King as usize
}

const fn build() -> Keys {
    let mut seed = 0x00C0_FFEE_C420_60E5;
    let mut keys = Keys { piece: [[[[0; 64]; 2]; Kind::COUNT]; 2], side: 0, ep: [0; 64], clock: [0; 128] };
    let mut color = 0;
    while color < 2 {
        let mut kind = 0;
        while kind < Kind::COUNT {
            let mut s = 0;
            while s < 64 {
                let unmoved = split_mix(&mut seed);
                let moved = split_mix(&mut seed);
                keys.piece[color][kind][0][s] = unmoved;
                keys.piece[color][kind][1][s] = if moved_matters(kind) { moved } else { unmoved };
                s += 1;
            }
            kind += 1;
        }
        color += 1;
    }
    keys.side = split_mix(&mut seed);
    let mut s = 0;
    while s < 64 {
        keys.ep[s] = split_mix(&mut seed);
        s += 1;
    }
    let mut c = 0;
    while c < 128 {
        keys.clock[c] = split_mix(&mut seed);
        c += 1;
    }
    keys
}

static KEYS: Keys = build();

/// The key part of one piece on one square.
#[inline(always)]
pub fn piece_key(piece: Piece, s: Square) -> u64 {
    KEYS.piece[piece.color.index()][piece.kind.index()][piece.moved as usize][s as usize]
}

/// The key part of the side that has the move and of the en passant square.
#[inline(always)]
pub fn turn_key(turn: Color, ep: Option<Square>) -> u64 {
    let side = if turn == Color::Black { KEYS.side } else { 0 };
    match ep {
        Some(s) => side ^ KEYS.ep[s as usize],
        None => side,
    }
}

/// A key part for a value of the clock. The search adds it when the clock is near its limit.
#[inline(always)]
pub fn clock_key(clock: u32) -> u64 {
    KEYS.clock[clock.min(127) as usize]
}

/// The key of a state, from all its pieces. `State::key` gives the same number faster.
pub fn key_from_scratch(state: &State) -> u64 {
    let mut key = turn_key(state.turn, state.ep);
    for (s, piece) in state.board().iter().enumerate() {
        if let Some(piece) = piece {
            key ^= piece_key(*piece, s as Square);
        }
    }
    key
}
