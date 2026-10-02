//! The state of a battle, and the functions that make and unmake a move.

use std::sync::Arc;

use crate::rules::{Rules, RulesError};
use crate::tables::Tables;
use crate::types::{Bitboard, Color, Kind, Move, Piece, Placement, Special, Square, bit};
use crate::zobrist;

/// The state of a battle.
///
/// The board has two forms that always agree: a mailbox (one entry for each square) and
/// bitboards (one set of squares for each color and for each kind).
#[derive(Clone, Debug)]
pub struct State {
    board: [Option<Piece>; 64],
    by_color: [Bitboard; 2],
    by_kind: [Bitboard; Kind::COUNT],
    /// The Zobrist key of the pieces. `put` and `remove` keep it up to date.
    piece_key: u64,
    /// The side that has the move.
    turn: Color,
    /// The square that a pawn crossed with a double step on the last move. Only `make`,
    /// `unmake`, the null move, and `with_en_passant` set it.
    ep: Option<Square>,
    /// The number of half moves with no capture and no pawn advance.
    pub clock: u32,
    tables: Arc<Tables>,
}

/// The data that `State::unmake` needs to take back a move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Undo {
    pub captured: Option<Piece>,
    /// The square of the captured piece. It is not the `to` square for an en passant capture.
    pub captured_square: Square,
    ep: Option<Square>,
    clock: u32,
    moved: bool,
    kind: Kind,
}

/// The data that `State::unmake_null` needs to take back a null move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NullUndo {
    ep: Option<Square>,
}

/// The reason why the engine cannot make a state from the data of a caller.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StateError {
    Rules(RulesError),
    /// A placement has a square that is more than 63.
    BadSquare(Square),
    /// The en passant square is not on the board, is not empty, or has no pawn of the side
    /// that does not have the move in front of it.
    BadEnPassant(Square),
    /// The text is not a piece field of a FEN string.
    BadFen(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::Rules(error) => error.fmt(f),
            StateError::BadSquare(s) => write!(f, "Square {s} is not on the board"),
            StateError::BadEnPassant(s) => write!(f, "Square {s} cannot be the en passant square"),
            StateError::BadFen(text) => write!(f, "\"{text}\" is not a piece field of a FEN string"),
        }
    }
}

impl std::error::Error for StateError {}

impl From<RulesError> for StateError {
    fn from(error: RulesError) -> StateError {
        StateError::Rules(error)
    }
}

impl State {
    /// Makes a state with White to move and no en passant square. If two pieces have the same
    /// square, the last one stays.
    pub fn new(pieces: &[Placement], rules: Rules) -> Result<State, StateError> {
        State::with_tables(pieces, Arc::new(Tables::new(rules)?))
    }

    /// Makes a state that shares the tables of a battle.
    pub fn with_tables(pieces: &[Placement], tables: Arc<Tables>) -> Result<State, StateError> {
        let mut state = State {
            board: [None; 64],
            by_color: [0; 2],
            by_kind: [0; Kind::COUNT],
            piece_key: 0,
            turn: Color::White,
            ep: None,
            clock: 0,
            tables,
        };
        for placement in pieces {
            if placement.square >= 64 {
                return Err(StateError::BadSquare(placement.square));
            }
            state.remove(placement.square);
            state.put(placement.square, placement.piece);
        }
        Ok(state)
    }

    /// The same state with another side to move and no en passant square.
    pub fn with_turn(mut self, turn: Color) -> State {
        self.turn = turn;
        self.ep = None;
        self
    }

    /// The same state with an en passant square. The square must be empty, and a pawn of the
    /// side that does not have the move must stand on the next square in its direction of
    /// movement.
    pub fn with_en_passant(mut self, ep: Option<Square>) -> Result<State, StateError> {
        if let Some(s) = ep {
            let mover = self.turn.other();
            let victim = s as i8 + 8 * mover.forward();
            let valid = s < 64
                && self.board[s as usize].is_none()
                && (0..64).contains(&victim)
                && self.pieces(mover, Kind::Pawn) & bit(victim as Square) != 0;
            if !valid {
                return Err(StateError::BadEnPassant(s));
            }
        }
        self.ep = ep;
        Ok(self)
    }

    /// The side that has the move.
    #[inline(always)]
    pub fn turn(&self) -> Color {
        self.turn
    }

    /// The square that a pawn crossed with a double step on the last move.
    #[inline(always)]
    pub fn ep(&self) -> Option<Square> {
        self.ep
    }

    #[inline(always)]
    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    pub fn shared_tables(&self) -> Arc<Tables> {
        Arc::clone(&self.tables)
    }

    pub fn rules(&self) -> &Rules {
        self.tables.rules()
    }

    #[inline(always)]
    pub fn board(&self) -> &[Option<Piece>; 64] {
        &self.board
    }

    #[inline(always)]
    pub fn piece_at(&self, s: Square) -> Option<Piece> {
        self.board[s as usize]
    }

    #[inline(always)]
    pub fn occupied(&self) -> Bitboard {
        self.by_color[0] | self.by_color[1]
    }

    #[inline(always)]
    pub fn color_set(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    #[inline(always)]
    pub fn kind_set(&self, kind: Kind) -> Bitboard {
        self.by_kind[kind.index()]
    }

    #[inline(always)]
    pub fn pieces(&self, color: Color, kind: Kind) -> Bitboard {
        self.by_color[color.index()] & self.by_kind[kind.index()]
    }

    /// The square of the king of a side. If the side has more than one king, this is the lowest square.
    #[inline(always)]
    pub fn king_square(&self, color: Color) -> Option<Square> {
        let kings = self.pieces(color, Kind::King);
        (kings != 0).then(|| kings.trailing_zeros() as Square)
    }

    /// The pieces of a side that are not a king. A side with no such piece loses by rout.
    #[inline(always)]
    pub fn men(&self, color: Color) -> Bitboard {
        self.by_color[color.index()] & !self.by_kind[Kind::King.index()]
    }

    /// The Zobrist key of the state: the pieces, the side that has the move, and the en passant
    /// square. The clock is not in the key. See `zobrist`.
    #[inline(always)]
    pub fn key(&self) -> u64 {
        self.piece_key ^ zobrist::turn_key(self.turn, self.ep)
    }

    #[inline(always)]
    fn put(&mut self, s: Square, piece: Piece) {
        self.board[s as usize] = Some(piece);
        self.by_color[piece.color.index()] |= bit(s);
        self.by_kind[piece.kind.index()] |= bit(s);
        self.piece_key ^= zobrist::piece_key(piece, s);
    }

    #[inline(always)]
    fn remove(&mut self, s: Square) -> Option<Piece> {
        let piece = self.board[s as usize].take();
        if let Some(piece) = piece {
            self.by_color[piece.color.index()] &= !bit(s);
            self.by_kind[piece.kind.index()] &= !bit(s);
            self.piece_key ^= zobrist::piece_key(piece, s);
        }
        piece
    }

    /// Plays a move. The move must come from the move generation for this state.
    ///
    /// Panics if the `from` square is empty.
    #[inline]
    pub fn make(&mut self, m: Move) -> Undo {
        let mut piece = self.remove(m.from).expect("the from square has no piece");
        let mover = piece.color;
        let captured_square = if m.special == Special::EnPassant { m.en_passant_victim(mover) } else { m.to };
        let undo = Undo {
            captured: self.remove(captured_square),
            captured_square,
            ep: self.ep,
            clock: self.clock,
            moved: piece.moved,
            kind: piece.kind,
        };
        if let Some(kind) = m.promo {
            piece.kind = kind;
        }
        piece.moved = true;
        self.put(m.to, piece);
        if m.special == Special::Castle {
            let (rook_from, rook_to) = m.castle_rook();
            let mut rook = self.remove(rook_from).expect("the castle has no rook");
            rook.moved = true;
            self.put(rook_to, rook);
        }
        self.ep = (m.special == Special::DoubleStep).then(|| m.crossed_square());
        let pawn_advance = undo.kind == Kind::Pawn && m.special != Special::Backward;
        self.clock = if undo.captured.is_some() || pawn_advance { 0 } else { self.clock + 1 };
        self.turn = mover.other();
        undo
    }

    /// Takes back the last move.
    #[inline]
    pub fn unmake(&mut self, m: Move, undo: Undo) {
        if m.special == Special::Castle {
            let (rook_from, rook_to) = m.castle_rook();
            let mut rook = self.remove(rook_to).expect("the castle has no rook");
            rook.moved = false;
            self.put(rook_from, rook);
        }
        let mut piece = self.remove(m.to).expect("the to square has no piece");
        piece.kind = undo.kind;
        piece.moved = undo.moved;
        self.put(m.from, piece);
        if let Some(captured) = undo.captured {
            self.put(undo.captured_square, captured);
        }
        self.ep = undo.ep;
        self.clock = undo.clock;
        self.turn = piece.color;
    }

    /// Passes the move to the other side: no piece moves. The search uses it for null-move
    /// pruning. The clock stays the same.
    #[inline]
    pub fn make_null(&mut self) -> NullUndo {
        let undo = NullUndo { ep: self.ep.take() };
        self.turn = self.turn.other();
        undo
    }

    /// Takes back `make_null`. `undo` is the value that `make_null` returned.
    #[inline]
    pub fn unmake_null(&mut self, undo: NullUndo) {
        self.turn = self.turn.other();
        self.ep = undo.ep;
    }

    /// True if the bitboards agree with the mailbox. The tests use this function.
    pub fn is_consistent(&self) -> bool {
        let mut by_color = [0; 2];
        let mut by_kind = [0; Kind::COUNT];
        for (s, piece) in self.board.iter().enumerate() {
            if let Some(piece) = piece {
                by_color[piece.color.index()] |= bit(s as Square);
                by_kind[piece.kind.index()] |= bit(s as Square);
            }
        }
        by_color == self.by_color && by_kind == self.by_kind
    }
}

/// Two states are equal when the boards, the bitboards, the turn, the en passant square,
/// the clock, and the rules are equal.
impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        self.board == other.board
            && self.by_color == other.by_color
            && self.by_kind == other.by_kind
            && self.turn == other.turn
            && self.ep == other.ep
            && self.clock == other.clock
            && (Arc::ptr_eq(&self.tables, &other.tables) || self.rules() == other.rules())
    }
}

impl Eq for State {}
