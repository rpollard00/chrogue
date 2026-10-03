//! Zobrist keys. `State` keeps the key of its pieces up to date in `make` and `unmake`.
//!
//! The key of a state has a part for each piece (its kind, color, square, and `moved` flag),
//! a part for the side that has the move, a part for each en passant square, and a part for the
//! square of the victim of an en passant capture when the state has en passant squares. The `moved`
//! flag must be in the key for each kind whose moves can read it: a kind with an atom with
//! `Condition::Unmoved`, and the king and the partners of the castles. There are two tables.
//! The standard table has the flag for the pawn, the rook, and the king, as in ordinary chess.
//! The other table has it for each kind. A battle uses the other table only if its rules read
//! the flag of another kind (`Tables::all_moved_keyed`). A flag in the key that no move reads
//! only makes fewer positions equal. The clock is not in the key.

use crate::rng::split_mix;
use crate::state::State;
use crate::types::{Bitboard, Color, Kind, Piece, Square, pop_square};

/// `[color][kind][moved][square]`
pub type PieceKeys = [[[[u64; 64]; 2]; Kind::COUNT]; 2];

struct Keys {
    /// The `moved` flag counts for the pawn, the rook, and the king.
    standard: PieceKeys,
    /// The `moved` flag counts for each kind.
    all_moved: PieceKeys,
    side: u64,
    ep: [u64; 64],
    /// The square of the victim of an en passant capture.
    victim: [u64; 64],
    clock: [u64; 128],
}

const fn build() -> Keys {
    let mut seed = 0x00C0_FFEE_C420_60E5;
    let mut keys = Keys {
        standard: [[[[0; 64]; 2]; Kind::COUNT]; 2],
        all_moved: [[[[0; 64]; 2]; Kind::COUNT]; 2],
        side: 0,
        ep: [0; 64],
        victim: [0; 64],
        clock: [0; 128],
    };
    let mut color = 0;
    while color < 2 {
        let mut kind = 0;
        while kind < Kind::COUNT {
            let mut s = 0;
            while s < 64 {
                let unmoved = split_mix(&mut seed);
                let moved = split_mix(&mut seed);
                let standard =
                    kind == Kind::Pawn as usize || kind == Kind::Rook as usize || kind == Kind::King as usize;
                keys.standard[color][kind][0][s] = unmoved;
                keys.standard[color][kind][1][s] = if standard { moved } else { unmoved };
                keys.all_moved[color][kind][0][s] = unmoved;
                keys.all_moved[color][kind][1][s] = moved;
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
    // After the other keys, thus the other keys are the same as before this part.
    let mut s = 0;
    while s < 64 {
        keys.victim[s] = split_mix(&mut seed);
        s += 1;
    }
    keys
}

static KEYS: Keys = build();

/// The piece keys of a battle. `all_moved`: the `moved` flag counts for each kind.
pub fn piece_keys(all_moved: bool) -> &'static PieceKeys {
    if all_moved { &KEYS.all_moved } else { &KEYS.standard }
}

/// The key part of one piece on one square.
#[inline(always)]
pub fn piece_key(keys: &PieceKeys, piece: Piece, s: Square) -> u64 {
    keys[piece.color.index()][piece.kind.index()][piece.moved as usize][s as usize]
}

/// The key part of the side that has the move, of the en passant squares, and of their victim.
/// `victim` counts only when `ep` is not empty: two rules can make the same en passant squares
/// with different victims.
#[inline(always)]
pub fn turn_key(turn: Color, mut ep: Bitboard, victim: Square) -> u64 {
    let mut key = if turn == Color::Black { KEYS.side } else { 0 };
    if ep != 0 {
        key ^= KEYS.victim[victim as usize & 63];
    }
    while ep != 0 {
        key ^= KEYS.ep[pop_square(&mut ep) as usize];
    }
    key
}

/// A key part for a value of the clock. The search adds it when the clock is near its limit.
#[inline(always)]
pub fn clock_key(clock: u32) -> u64 {
    KEYS.clock[clock.min(127) as usize]
}

/// The key of a state, from all its pieces. `State::key` gives the same number faster.
pub fn key_from_scratch(state: &State) -> u64 {
    let mut key = turn_key(state.turn(), state.ep_squares(), state.ep_victim());
    for (s, piece) in state.board().iter().enumerate() {
        if let Some(piece) = piece {
            key ^= piece_key(piece_keys(state.tables().all_moved_keyed()), *piece, s as Square);
        }
    }
    key
}
