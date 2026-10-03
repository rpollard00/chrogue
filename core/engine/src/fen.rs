//! Reads the piece field of a FEN string. The tests and the tools use this module.

use crate::rules::Rules;
use crate::state::{State, StateError};
use crate::types::{Color, Kind, Piece, Placement, Square};

/// The square of a name such as "e4". The tests and the tools use it with names that they
/// know.
///
/// Panics if the name is not a square.
pub fn square(name: &str) -> Square {
    let bytes = name.as_bytes();
    assert!(
        bytes.len() == 2 && (b'a'..=b'h').contains(&bytes[0]) && (b'1'..=b'8').contains(&bytes[1]),
        "\"{name}\" is not a square"
    );
    (bytes[1] - b'1') * 8 + (bytes[0] - b'a')
}

/// The pieces of the piece field of a FEN string. The ids count from 0 in the order of the string.
///
/// A pawn that is not on its start rank counts as moved. All other pieces count as not moved.
pub fn placements(fen: &str) -> Result<Vec<Placement>, StateError> {
    let bad = || StateError::BadFen(fen.to_string());
    let rows: Vec<&str> = fen.split('/').collect();
    if rows.len() != 8 {
        return Err(bad());
    }
    let mut pieces = Vec::new();
    for (row, text) in rows.into_iter().enumerate() {
        let rank = 7 - row as u8;
        let mut file = 0u8;
        for ch in text.chars() {
            if let Some(empty) = ch.to_digit(10).filter(|n| (1..=8).contains(n)) {
                file += empty as u8;
            } else {
                let kind = Kind::from_letter(ch.to_ascii_lowercase()).ok_or_else(bad)?;
                if file >= 8 {
                    return Err(bad());
                }
                let color = if ch.is_ascii_uppercase() { Color::White } else { Color::Black };
                let start_rank = if color == Color::White { 1 } else { 6 };
                let moved = kind == Kind::Pawn && rank != start_rank;
                let piece = Piece { id: pieces.len() as u16, kind, color, moved };
                pieces.push(Placement { piece, square: rank * 8 + file });
                file += 1;
            }
            if file > 8 {
                return Err(bad());
            }
        }
        if file != 8 {
            return Err(bad());
        }
    }
    Ok(pieces)
}

/// A state from the piece field of a FEN string, with no en passant square.
pub fn from_fen(fen: &str, turn: Color, rules: Rules) -> Result<State, StateError> {
    Ok(State::new(&placements(fen)?, rules)?.with_turn(turn))
}

pub const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR";
pub const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R";
