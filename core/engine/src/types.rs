//! The basic values of the engine: squares, colors, kinds, pieces, and moves.

/// A square is an index from 0 (a1) to 63 (h8). White moves toward higher ranks.
pub type Square = u8;

/// A set of squares. Bit `s` is set when square `s` is in the set.
pub type Bitboard = u64;

#[inline(always)]
pub const fn bit(s: Square) -> Bitboard {
    1u64 << s
}

/// Removes the lowest square from a set that is not empty, and returns the square.
#[inline(always)]
pub fn pop_square(set: &mut Bitboard) -> Square {
    let s = set.trailing_zeros() as Square;
    *set &= *set - 1;
    s
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    #[inline(always)]
    pub const fn other(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    #[inline(always)]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The rank step of a pawn of this color: 1 for White, -1 for Black.
    #[inline(always)]
    pub const fn forward(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Kind {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl Kind {
    pub const COUNT: usize = 6;
    pub const ALL: [Kind; 6] = [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen, Kind::King];
    /// The kinds that get their movement from `KindRules`. The pawn has `PawnRules`.
    pub const OFFICERS: [Kind; 5] = [Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen, Kind::King];

    #[inline(always)]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The letter of the kind in the TypeScript engine: p, n, b, r, q, or k.
    pub const fn letter(self) -> char {
        match self {
            Kind::Pawn => 'p',
            Kind::Knight => 'n',
            Kind::Bishop => 'b',
            Kind::Rook => 'r',
            Kind::Queen => 'q',
            Kind::King => 'k',
        }
    }

    pub const fn from_letter(letter: char) -> Option<Kind> {
        Some(match letter {
            'p' => Kind::Pawn,
            'n' => Kind::Knight,
            'b' => Kind::Bishop,
            'r' => Kind::Rook,
            'q' => Kind::Queen,
            'k' => Kind::King,
            _ => return None,
        })
    }
}

/// A piece on the board. The id stays the same when the piece moves or promotes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub id: u16,
    pub kind: Kind,
    pub color: Color,
    /// True after the first move of the piece. Castling and the pawn double step read this flag.
    pub moved: bool,
}

/// A piece and its start square.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placement {
    pub piece: Piece,
    pub square: Square,
}

/// The one special property that a move can have. A move has no more than one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Special {
    None,
    /// A pawn moves two squares and the square that it crosses becomes the en passant square.
    DoubleStep,
    /// A pawn captures the pawn behind the `to` square.
    EnPassant,
    /// A pawn moves one square backward. This move does not reset the draw clock.
    Backward,
    /// The king moves two squares and the rook moves to the square that the king crossed.
    Castle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    /// The kind that a pawn becomes. An en passant capture can also have a promotion.
    pub promo: Option<Kind>,
    pub special: Special,
}

impl Move {
    pub const NULL: Move = Move { from: 0, to: 0, promo: None, special: Special::None };

    pub const fn new(from: Square, to: Square) -> Move {
        Move { from, to, promo: None, special: Special::None }
    }

    /// The en passant square that a double step makes.
    #[inline(always)]
    pub const fn crossed_square(self) -> Square {
        (self.from + self.to) / 2
    }

    /// The square of the pawn that an en passant capture removes.
    #[inline(always)]
    pub const fn en_passant_victim(self, mover: Color) -> Square {
        match mover {
            Color::White => self.to - 8,
            Color::Black => self.to + 8,
        }
    }

    /// The rook move of a castle: (from, to).
    #[inline(always)]
    pub const fn castle_rook(self) -> (Square, Square) {
        if self.to > self.from { (self.from + 3, self.from + 1) } else { (self.from - 4, self.from - 1) }
    }
}

/// A list of moves with a fixed capacity. The list does not use the heap.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; MoveList::CAPACITY],
    len: usize,
}

impl MoveList {
    /// Ordinary chess has no position with more than 218 moves. Added movement rules can give more.
    pub const CAPACITY: usize = 384;

    #[inline(always)]
    pub const fn new() -> MoveList {
        MoveList { moves: [Move::NULL; MoveList::CAPACITY], len: 0 }
    }

    /// Adds a move. Panics if the list is full.
    #[inline(always)]
    pub fn push(&mut self, m: Move) {
        self.moves[self.len] = m;
        self.len += 1;
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    /// Keeps only the moves for which `keep` returns true. The order stays the same.
    pub fn retain(&mut self, mut keep: impl FnMut(Move) -> bool) {
        let mut kept = 0;
        for i in 0..self.len {
            let m = self.moves[i];
            if keep(m) {
                self.moves[kept] = m;
                kept += 1;
            }
        }
        self.len = kept;
    }
}

impl Default for MoveList {
    fn default() -> Self {
        MoveList::new()
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
