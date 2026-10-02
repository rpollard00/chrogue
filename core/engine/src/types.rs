//! The basic values of the engine: squares, colors, kinds, pieces, and moves.

use std::mem::MaybeUninit;

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
    /// The kinds that are not the pawn.
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
    /// True after the first move of the piece. Castles and atoms with `Condition::Unmoved` read this flag.
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
    /// The move passes squares, and its atom makes en passant squares (`Atom::makes_en_passant`):
    /// the squares that it passes become the en passant squares. The pawn double step.
    DoubleStep,
    /// The move captures en passant: the captured piece is the piece that made the en passant
    /// squares, not a piece on the `to` square.
    EnPassant,
    /// A move that is not a capture and does not reset the clock, of a kind that has an atom
    /// that resets the clock. The backward step of the pawn.
    Backward,
    /// The king and a partner piece move by a row of `SideRules::castles`. See
    /// `Rules::castle_partner`.
    Castle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    /// The kind that the piece becomes. An en passant capture can also have a promotion.
    pub promo: Option<Kind>,
    pub special: Special,
}

impl Move {
    pub const NULL: Move = Move { from: 0, to: 0, promo: None, special: Special::None };

    pub const fn new(from: Square, to: Square) -> Move {
        Move { from, to, promo: None, special: Special::None }
    }
}

/// A list of moves. The first `CAPACITY` moves are in the list itself. A list with more moves
/// puts all its moves on the heap, thus a push cannot fail for any rules or any position.
///
/// A new list does not write its slots, because the search makes a list at each node.
#[derive(Clone)]
pub struct MoveList {
    /// The slots `..len` have a move while `len <= CAPACITY`.
    inline: [MaybeUninit<Move>; MoveList::CAPACITY],
    len: usize,
    /// Empty while `len <= CAPACITY`. Then it has all the moves.
    heap: Vec<Move>,
}

impl MoveList {
    /// The number of moves that the list holds without the heap. Ordinary chess has no position
    /// with more than 218 moves. Added movement rules and added pieces can give more.
    pub const CAPACITY: usize = 384;

    #[inline(always)]
    pub const fn new() -> MoveList {
        MoveList { inline: [const { MaybeUninit::uninit() }; MoveList::CAPACITY], len: 0, heap: Vec::new() }
    }

    /// Adds a move.
    #[inline(always)]
    pub fn push(&mut self, m: Move) {
        match self.inline.get_mut(self.len) {
            Some(slot) => {
                slot.write(m);
            }
            None => self.push_to_heap(m),
        }
        self.len += 1;
    }

    #[cold]
    #[inline(never)]
    fn push_to_heap(&mut self, m: Move) {
        if self.heap.is_empty() {
            let moves = self.as_slice().to_vec();
            self.heap = moves;
        }
        self.heap.push(m);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
        self.heap.clear();
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
        if self.len <= MoveList::CAPACITY {
            // SAFETY: the slots `..len` have a move, and `MaybeUninit<Move>` has the layout of `Move`.
            unsafe { std::slice::from_raw_parts(self.inline.as_ptr().cast::<Move>(), self.len) }
        } else {
            &self.heap
        }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        if self.len <= MoveList::CAPACITY {
            // SAFETY: the same as in `as_slice`.
            unsafe { std::slice::from_raw_parts_mut(self.inline.as_mut_ptr().cast::<Move>(), self.len) }
        } else {
            &mut self.heap
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    /// Keeps only the moves for which `keep` returns true. The order stays the same.
    pub fn retain(&mut self, mut keep: impl FnMut(Move) -> bool) {
        let moves = self.as_mut_slice();
        let mut kept = 0;
        for i in 0..moves.len() {
            let m = moves[i];
            if keep(m) {
                moves[kept] = m;
                kept += 1;
            }
        }
        // A list that is small again goes back from the heap.
        if self.len > MoveList::CAPACITY && kept <= MoveList::CAPACITY {
            for (slot, &m) in self.inline.iter_mut().zip(&self.heap[..kept]) {
                slot.write(m);
            }
            self.heap.clear();
        }
        self.heap.truncate(kept);
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
