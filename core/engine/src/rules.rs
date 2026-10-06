//! The movement rules as data. Each side of a battle has its own `SideRules`.
//!
//! The engine has no code for a specific relic. A relic is an edit of `SideRules::standard()`.
//!
//! Each kind, the pawn too, moves by a list of atoms. An atom is a list of offsets, a number of
//! steps, a mode, a condition, and three properties for en passant and the clock. A leap is an
//! atom with one step, and a slide is an atom with `Atom::MAX_STEPS`. A kind can also have a
//! promotion. A kind can also have hooks: slides that end with one step in another direction.
//! The castles of a side are rows of `Castle`.

use crate::types::{Bitboard, Color, Kind, Square, bit};

/// A step of (file, rank) from the view of White. For Black, the engine mirrors the rank step.
pub type Offset = (i8, i8);

pub const KNIGHT: [Offset; 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
pub const CAMEL: [Offset; 8] = [(1, 3), (3, 1), (3, -1), (1, -3), (-1, -3), (-3, -1), (-3, 1), (-1, 3)];
pub const KING: [Offset; 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
pub const ORTHO: [Offset; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
pub const DIAG: [Offset; 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];
/// Two squares on a file or a rank.
pub const DABBABA: [Offset; 4] = [(2, 0), (0, 2), (-2, 0), (0, -2)];
/// Two squares on a diagonal.
pub const ALFIL: [Offset; 4] = [(2, 2), (-2, 2), (-2, -2), (2, -2)];
/// One square forward, from the view of the side.
pub const FORWARD: [Offset; 1] = [(0, 1)];
/// One square backward, from the view of the side.
pub const BACKWARD: [Offset; 1] = [(0, -1)];
/// The two squares diagonally forward, from the view of the side.
pub const FORWARD_DIAG: [Offset; 2] = [(-1, 1), (1, 1)];

/// What a movement atom can do on its target square.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Mode {
    /// Move to an empty square or capture an enemy piece.
    MoveOrCapture,
    /// Move to an empty square only. This atom does not attack a square, thus it does not give check.
    MoveOnly,
    /// Capture an enemy piece only.
    CaptureOnly,
}

impl Mode {
    pub const fn can_move(self) -> bool {
        matches!(self, Mode::MoveOrCapture | Mode::MoveOnly)
    }

    pub const fn can_capture(self) -> bool {
        matches!(self, Mode::MoveOrCapture | Mode::CaptureOnly)
    }
}

/// When a piece can use an atom.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Condition {
    Always,
    /// Only while the piece has not moved (`Piece::moved` is false). An atom with this condition
    /// also attacks only while the piece has not moved.
    Unmoved,
    /// Only while another piece of the same side and of this kind is `range` squares away or
    /// less, as a king counts squares. An atom with this condition also attacks only then. The
    /// atom cannot have an en passant property (`RulesError::BadCondition`).
    Near {
        kind: Kind,
        range: u8,
    },
}

/// `NEAR[range][s]`: the squares that are `range` squares away from `s` or less, without `s`.
static NEAR: [[Bitboard; 64]; 8] = {
    let mut zones = [[0; 64]; 8];
    let mut range = 0;
    while range < 8 {
        let mut s = 0usize;
        while s < 64 {
            let mut other = 0usize;
            while other < 64 {
                let files = (s % 8) as i32 - (other % 8) as i32;
                let ranks = (s / 8) as i32 - (other / 8) as i32;
                if other != s && files.abs() <= range as i32 && ranks.abs() <= range as i32 {
                    zones[range][s] |= 1 << other;
                }
                other += 1;
            }
            s += 1;
        }
        range += 1;
    }
    zones
};

impl Condition {
    /// The squares where a piece of `Condition::Near` with this range lets a piece on `s` move.
    #[inline(always)]
    pub fn near_zone(s: Square, range: u8) -> Bitboard {
        NEAR[range.min(7) as usize][s as usize]
    }
}

/// One part of the movement of a kind: steps along each offset.
///
/// A piece goes one to `max_steps` steps along an offset. Each step must end on an empty
/// square, except the last step, which can capture (see `mode`). The squares inside one step
/// do not matter, thus an atom with one step is a leap.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct Atom {
    pub offsets: Vec<Offset>,
    /// From 1 (a leap) to `Atom::MAX_STEPS` (a slide that stops only at a piece or at the edge).
    pub max_steps: u8,
    pub mode: Mode,
    pub condition: Condition,
    /// The squares that a move of this atom passes become the en passant squares of the next
    /// half move. A move of one step passes no square. A move that promotes makes no en passant
    /// squares. A king cannot have this property (`RulesError::KingMakesEnPassant`): a king is
    /// never the victim of an en passant capture.
    pub makes_en_passant: bool,
    /// The atom can capture en passant: it can go to an en passant square as if the square has
    /// the piece that made it, and that piece is captured. The atom must be able to capture.
    pub captures_en_passant: bool,
}

impl Atom {
    /// The number of steps of a slide. Seven steps go across the board in each direction.
    pub const MAX_STEPS: u8 = 7;

    /// One jump to each offset. Pieces between the two squares do not block the jump.
    pub fn leap(offsets: &[Offset], mode: Mode) -> Atom {
        Atom {
            offsets: offsets.to_vec(),
            max_steps: 1,
            mode,
            condition: Condition::Always,
            makes_en_passant: false,
            captures_en_passant: false,
        }
    }

    /// Steps in each direction until a piece blocks the line.
    pub fn slide(dirs: &[Offset], mode: Mode) -> Atom {
        Atom { max_steps: Atom::MAX_STEPS, ..Atom::leap(dirs, mode) }
    }

    /// The same atom with at most `steps` steps.
    pub fn max_steps(mut self, steps: u8) -> Atom {
        self.max_steps = steps;
        self
    }

    /// The same atom, only for a piece that has not moved.
    pub fn if_unmoved(mut self) -> Atom {
        self.condition = Condition::Unmoved;
        self
    }

    /// The same atom, only while another piece of the side of this kind is `range` squares away
    /// or less.
    pub fn if_near(mut self, kind: Kind, range: u8) -> Atom {
        self.condition = Condition::Near { kind, range };
        self
    }

    /// The same atom; the squares that its moves pass become en passant squares.
    pub fn makes_en_passant(mut self) -> Atom {
        self.makes_en_passant = true;
        self
    }

    /// The same atom; it can also capture en passant.
    pub fn captures_en_passant(mut self) -> Atom {
        self.captures_en_passant = true;
        self
    }

    /// True if two atoms give their moves the same properties. The move generation puts such
    /// atoms in one group. See `core/README.md`.
    pub fn same_group(&self, other: &Atom) -> bool {
        self.condition == other.condition
            && self.makes_en_passant == other.makes_en_passant
            && self.captures_en_passant == other.captures_en_passant
    }
}

/// A slide that turns: the piece goes `min_leg` to `max_leg` squares along a leg, and then one
/// last step in another direction. Each square of the leg must be empty. The last step can
/// capture (see `mode`). A hook with the legs of a rook and a last step to the side is a knight
/// move with a long leg that a piece can block.
///
/// Only a kind with plain atoms and no promotion can have a hook (`RulesError::BadHook`).
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct Hook {
    /// The direction of the leg and the last step of each bend, from the view of White.
    pub bends: Vec<(Offset, Offset)>,
    /// The smallest number of squares of the leg, from 1.
    pub min_leg: u8,
    /// The largest number of squares of the leg, to `Atom::MAX_STEPS`.
    pub max_leg: u8,
    pub mode: Mode,
}

impl Hook {
    /// A leg of `min_leg` squares or more along each direction, then one step to the left or
    /// to the right of the leg.
    pub fn right_angle(dirs: &[Offset], min_leg: u8, mode: Mode) -> Hook {
        let bends = dirs.iter().flat_map(|&(df, dr)| [((df, dr), (-dr, df)), ((df, dr), (dr, -df))]).collect();
        Hook { bends, min_leg, max_leg: Atom::MAX_STEPS, mode }
    }
}

/// The kinds that a piece can become: one to four different kinds. A piece cannot become a
/// pawn or a king.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Promotions {
    kinds: [Kind; 4],
    len: u8,
}

impl Promotions {
    /// Ordinary chess: queen, knight, rook, bishop.
    pub const STANDARD: Promotions =
        Promotions { kinds: [Kind::Queen, Kind::Knight, Kind::Rook, Kind::Bishop], len: 4 };

    /// The kinds in the order of the generated moves. Gives an error for an empty list, for the
    /// pawn or the king, and for a kind that is in the list two times. Thus a list has four
    /// kinds or fewer.
    pub fn new(kinds: &[Kind]) -> Result<Promotions, RulesError> {
        let mut list = Promotions { kinds: [Kind::Queen; 4], len: 0 };
        for &kind in kinds {
            if matches!(kind, Kind::Pawn | Kind::King) || list.as_slice().contains(&kind) {
                return Err(RulesError::BadPromotion(kind));
            }
            list.kinds[list.len as usize] = kind;
            list.len += 1;
        }
        if list.len == 0 { Err(RulesError::NoPromotion) } else { Ok(list) }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[Kind] {
        &self.kinds[..self.len as usize]
    }
}

/// The promotion of a kind. A move that ends in the promotion zone gives one move for each
/// kind of the list, except a move that goes backward: it never promotes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Promotion {
    /// The number of ranks before the last rank where the promotion zone starts. 0 is ordinary
    /// chess. The largest value is 6: the zone starts on the second rank, thus a piece promotes
    /// on its first move forward from the first or the second rank.
    pub distance: u8,
    /// The kinds that the piece can become, in the order of the generated moves.
    pub kinds: Promotions,
}

impl Promotion {
    pub const STANDARD: Promotion = Promotion { distance: 0, kinds: Promotions::STANDARD };

    /// The squares of the promotion zone of White. The zone of Black is the mirror.
    pub fn zone(self) -> Bitboard {
        let first_rank = 7 - self.distance.min(7) as u32;
        !0u64 << (first_rank * 8)
    }
}

/// The movement of one kind.
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
pub struct KindRules {
    pub atoms: Vec<Atom>,
    pub promotion: Option<Promotion>,
    pub hooks: Vec<Hook>,
}

/// One castle: a king and a partner piece move in one move. The squares are from the view of
/// White; for Black, the engine mirrors the ranks.
///
/// The castle is possible when the king is on `king_from` and has not moved, a piece of kind
/// `partner` of the same side is on `partner_from` and has not moved, the squares of `empty`
/// are empty, and the enemy attacks no square of `safe`. The engine also requires the two `to`
/// squares to be empty, unless the king or the partner stands there. The legality filter
/// rejects a castle that leaves the king in check, as for each move.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Castle {
    pub king_from: Square,
    pub king_to: Square,
    pub partner: Kind,
    pub partner_from: Square,
    pub partner_to: Square,
    pub empty: Bitboard,
    pub safe: Bitboard,
}

impl Castle {
    /// e1-c1 with the rook a1-d1. b1, c1, and d1 are empty. e1 and d1 are not attacked.
    pub const QUEEN_SIDE: Castle = Castle {
        king_from: 4,
        king_to: 2,
        partner: Kind::Rook,
        partner_from: 0,
        partner_to: 3,
        empty: bit(1) | bit(2) | bit(3),
        safe: bit(4) | bit(3),
    };
    /// e1-g1 with the rook h1-f1. f1 and g1 are empty. e1 and f1 are not attacked.
    pub const KING_SIDE: Castle = Castle {
        king_from: 4,
        king_to: 6,
        partner: Kind::Rook,
        partner_from: 7,
        partner_to: 5,
        empty: bit(5) | bit(6),
        safe: bit(4) | bit(5),
    };
    /// The castles of ordinary chess, in the order of the generated moves.
    pub const STANDARD: [Castle; 2] = [Castle::QUEEN_SIDE, Castle::KING_SIDE];

    /// The same castle for a color: Black gets the mirror of the ranks.
    pub fn for_color(self, color: Color) -> Castle {
        if color == Color::White {
            return self;
        }
        Castle {
            king_from: self.king_from ^ 56,
            king_to: self.king_to ^ 56,
            partner_from: self.partner_from ^ 56,
            partner_to: self.partner_to ^ 56,
            empty: self.empty.swap_bytes(),
            safe: self.safe.swap_bytes(),
            ..self
        }
    }

    /// The squares that must be empty: `empty` and the two `to` squares, without the squares
    /// of the king and the partner.
    pub fn required_empty(self) -> Bitboard {
        (self.empty | bit(self.king_to) | bit(self.partner_to)) & !bit(self.king_from) & !bit(self.partner_from)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct SideRules {
    /// The movement of each kind, by `Kind::index()`: pawn, knight, bishop, rook, queen, king.
    pub kinds: [KindRules; Kind::COUNT],
    /// The castles of the side, in the order of the generated moves.
    pub castles: Vec<Castle>,
}

/// The names of the rule flags: the six named edits of ordinary chess (`SideRules::with_flag`).
pub const FLAG_NAMES: [&str; 6] = ["forcedMarch", "backpedal", "earlyPromo", "kingKnight", "longLeap", "sidestep"];

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RulesError {
    UnknownFlag(String),
    /// The kind has an offset of (0, 0) or an offset that is larger than the board.
    BadOffset(Kind, Offset),
    /// The number of steps of an atom must be from 1 to `Atom::MAX_STEPS`.
    BadSteps(Kind, u8),
    /// The promotion zone must leave one rank or more for the pieces.
    BadPromoDistance(u8),
    /// A piece cannot become a pawn or a king, and the list cannot have a kind two times.
    BadPromotion(Kind),
    /// The list of promotion kinds is empty.
    NoPromotion,
    /// An atom of the king makes en passant squares. Then an en passant capture could remove a
    /// king that no piece attacks.
    KingMakesEnPassant,
    /// The castle at this index has a square off the board, a king that does not move, the king
    /// and the partner on the same square, or the same king squares as an earlier castle.
    BadCastle(usize),
    /// A hook of the kind has a leg or a last step of (0, 0) or larger than the board, or a number
    /// of leg squares that is not from 1 to `Atom::MAX_STEPS`. Or the kind has a promotion, or an
    /// atom with a condition or an en passant property.
    BadHook(Kind),
    /// An atom of the kind has `Condition::Near` with a range that is not from 1 to 7, or with an
    /// en passant property.
    BadCondition(Kind),
}

impl std::fmt::Display for RulesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulesError::UnknownFlag(name) => write!(f, "The rule flag \"{name}\" is unknown"),
            RulesError::BadOffset(kind, (df, dr)) => {
                write!(f, "The offset ({df}, {dr}) of kind {} is not permitted", kind.letter())
            }
            RulesError::BadSteps(kind, steps) => {
                write!(f, "An atom of kind {} has {steps} steps; it must have 1 to 7", kind.letter())
            }
            RulesError::BadPromoDistance(d) => write!(f, "The promotion distance {d} is more than 6"),
            RulesError::BadPromotion(kind) => {
                write!(f, "The promotion kind {} is a pawn, a king, or in the list two times", kind.letter())
            }
            RulesError::NoPromotion => write!(f, "The list of promotion kinds is empty"),
            RulesError::KingMakesEnPassant => write!(f, "An atom of the king makes en passant squares"),
            RulesError::BadCastle(index) => write!(f, "The castle at index {index} is not valid"),
            RulesError::BadHook(kind) => write!(f, "A hook of kind {} is not valid", kind.letter()),
            RulesError::BadCondition(kind) => {
                write!(f, "An atom of kind {} has a condition that is not valid", kind.letter())
            }
        }
    }
}

impl std::error::Error for RulesError {}

/// The atoms of the pawn of ordinary chess: the step, the double step on the first move, and
/// the diagonal captures.
pub fn standard_pawn_atoms() -> Vec<Atom> {
    vec![
        Atom::leap(&FORWARD, Mode::MoveOnly),
        Atom::slide(&FORWARD, Mode::MoveOnly).max_steps(2).if_unmoved().makes_en_passant(),
        Atom::leap(&FORWARD_DIAG, Mode::CaptureOnly).captures_en_passant(),
    ]
}

impl SideRules {
    /// Ordinary chess.
    pub fn standard() -> SideRules {
        use Mode::MoveOrCapture as Both;
        SideRules {
            kinds: [
                KindRules { atoms: standard_pawn_atoms(), promotion: Some(Promotion::STANDARD), hooks: Vec::new() },
                KindRules { atoms: vec![Atom::leap(&KNIGHT, Both)], promotion: None, hooks: Vec::new() },
                KindRules { atoms: vec![Atom::slide(&DIAG, Both)], promotion: None, hooks: Vec::new() },
                KindRules { atoms: vec![Atom::slide(&ORTHO, Both)], promotion: None, hooks: Vec::new() },
                KindRules {
                    atoms: vec![Atom::slide(&ORTHO, Both), Atom::slide(&DIAG, Both)],
                    promotion: None,
                    hooks: Vec::new(),
                },
                KindRules { atoms: vec![Atom::leap(&KING, Both)], promotion: None, hooks: Vec::new() },
            ],
            castles: Castle::STANDARD.to_vec(),
        }
    }

    /// The atoms of a kind.
    #[inline(always)]
    pub fn atoms(&self, kind: Kind) -> &[Atom] {
        &self.kinds[kind.index()].atoms
    }

    /// The rules of a kind.
    #[inline(always)]
    pub fn kind(&self, kind: Kind) -> &KindRules {
        &self.kinds[kind.index()]
    }

    /// Adds an atom to the movement of a kind.
    pub fn with_atom(mut self, kind: Kind, atom: Atom) -> SideRules {
        self.kinds[kind.index()].atoms.push(atom);
        self
    }

    /// Adds a hook to the movement of a kind.
    pub fn with_hook(mut self, kind: Kind, hook: Hook) -> SideRules {
        self.kinds[kind.index()].hooks.push(hook);
        self
    }

    /// Replaces the movement of a kind.
    pub fn with_kind(mut self, kind: Kind, atoms: Vec<Atom>) -> SideRules {
        self.kinds[kind.index()].atoms = atoms;
        self
    }

    /// Replaces the promotion of a kind. `None`: the kind does not promote.
    pub fn with_promotion(mut self, kind: Kind, promotion: Option<Promotion>) -> SideRules {
        self.kinds[kind.index()].promotion = promotion;
        self
    }

    /// Replaces the castles.
    pub fn with_castles(mut self, castles: Vec<Castle>) -> SideRules {
        self.castles = castles;
        self
    }

    /// `false` removes all castles. `true` gives the castles of ordinary chess.
    pub fn with_castling(self, castling: bool) -> SideRules {
        self.with_castles(if castling { Castle::STANDARD.to_vec() } else { Vec::new() })
    }

    /// Pawns can always move two squares forward: the atoms of the pawn that make en passant
    /// squares lose their condition.
    pub fn forced_march(mut self) -> SideRules {
        for atom in &mut self.kinds[Kind::Pawn.index()].atoms {
            if atom.makes_en_passant {
                atom.condition = Condition::Always;
            }
        }
        self
    }

    /// Pawns can move one square backward to an empty square.
    pub fn backpedal(mut self) -> SideRules {
        let step = Atom::leap(&BACKWARD, Mode::MoveOnly);
        let atoms = &mut self.kinds[Kind::Pawn.index()].atoms;
        if !atoms.contains(&step) {
            atoms.push(step);
        }
        self
    }

    /// Pawns promote one rank earlier.
    pub fn early_promo(mut self) -> SideRules {
        if let Some(promotion) = &mut self.kinds[Kind::Pawn.index()].promotion {
            promotion.distance = 1;
        }
        self
    }

    /// The king can also move as a knight.
    pub fn king_knight(self) -> SideRules {
        self.with_atom(Kind::King, Atom::leap(&KNIGHT, Mode::MoveOrCapture))
    }

    /// Knights can also jump three squares in one direction and one square to the side.
    pub fn long_leap(self) -> SideRules {
        self.with_atom(Kind::Knight, Atom::leap(&CAMEL, Mode::MoveOrCapture))
    }

    /// Bishops can move one square orthogonally to an empty square.
    pub fn sidestep(self) -> SideRules {
        self.with_atom(Kind::Bishop, Atom::leap(&ORTHO, Mode::MoveOnly))
    }

    /// Applies one rule flag by its name.
    pub fn with_flag(self, name: &str) -> Result<SideRules, RulesError> {
        Ok(match name {
            "forcedMarch" => self.forced_march(),
            "backpedal" => self.backpedal(),
            "earlyPromo" => self.early_promo(),
            "kingKnight" => self.king_knight(),
            "longLeap" => self.long_leap(),
            "sidestep" => self.sidestep(),
            _ => return Err(RulesError::UnknownFlag(name.to_string())),
        })
    }

    /// Ordinary chess with the given rule flags.
    pub fn from_flags<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<SideRules, RulesError> {
        names.into_iter().try_fold(SideRules::standard(), SideRules::with_flag)
    }

    /// The castle of a color whose king goes from `from` to `to`, with the squares of that color.
    pub fn castle(&self, color: Color, from: Square, to: Square) -> Option<Castle> {
        self.castles.iter().map(|castle| castle.for_color(color)).find(|c| c.king_from == from && c.king_to == to)
    }

    pub fn validate(&self) -> Result<(), RulesError> {
        for kind in Kind::ALL {
            let rules = self.kind(kind);
            if let Some(promotion) = rules.promotion
                && promotion.distance > 6
            {
                return Err(RulesError::BadPromoDistance(promotion.distance));
            }
            for atom in &rules.atoms {
                if !(1..=Atom::MAX_STEPS).contains(&atom.max_steps) {
                    return Err(RulesError::BadSteps(kind, atom.max_steps));
                }
                for &(df, dr) in &atom.offsets {
                    if (df, dr) == (0, 0) || df.abs() > 7 || dr.abs() > 7 {
                        return Err(RulesError::BadOffset(kind, (df, dr)));
                    }
                }
                if atom.makes_en_passant && kind == Kind::King {
                    return Err(RulesError::KingMakesEnPassant);
                }
                if let Condition::Near { range, .. } = atom.condition
                    && (!(1..=7).contains(&range) || atom.makes_en_passant || atom.captures_en_passant)
                {
                    return Err(RulesError::BadCondition(kind));
                }
            }
            let plain = rules.promotion.is_none()
                && rules.atoms.iter().all(|atom| {
                    atom.condition == Condition::Always && !atom.makes_en_passant && !atom.captures_en_passant
                });
            for hook in &rules.hooks {
                let on_board = |(df, dr): Offset| (df, dr) != (0, 0) && df.abs() <= 7 && dr.abs() <= 7;
                let legs = 1 <= hook.min_leg && hook.min_leg <= hook.max_leg && hook.max_leg <= Atom::MAX_STEPS;
                if !plain || !legs || hook.bends.iter().any(|&(leg, last)| !on_board(leg) || !on_board(last)) {
                    return Err(RulesError::BadHook(kind));
                }
            }
        }
        for (index, castle) in self.castles.iter().enumerate() {
            let squares = [castle.king_from, castle.king_to, castle.partner_from, castle.partner_to];
            let repeated = self.castles[..index]
                .iter()
                .any(|earlier| (earlier.king_from, earlier.king_to) == (castle.king_from, castle.king_to));
            if squares.iter().any(|&s| s >= 64)
                || castle.king_from == castle.king_to
                || castle.king_from == castle.partner_from
                || castle.king_to == castle.partner_to
                || repeated
            {
                return Err(RulesError::BadCastle(index));
            }
        }
        Ok(())
    }
}

/// The rules of the two sides: `[White, Black]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rules {
    pub sides: [SideRules; 2],
}

impl Rules {
    /// Ordinary chess for the two sides.
    pub fn standard() -> Rules {
        Rules { sides: [SideRules::standard(), SideRules::standard()] }
    }

    pub fn new(white: SideRules, black: SideRules) -> Rules {
        Rules { sides: [white, black] }
    }

    pub fn side(&self, color: Color) -> &SideRules {
        &self.sides[color.index()]
    }

    /// The move of the partner of a castle of `color`: (from, to). None if the move is not a
    /// castle of that color.
    pub fn castle_partner(&self, color: Color, m: crate::types::Move) -> Option<(Square, Square)> {
        if m.special != crate::types::Special::Castle {
            return None;
        }
        self.side(color).castle(color, m.from, m.to).map(|castle| (castle.partner_from, castle.partner_to))
    }

    pub fn validate(&self) -> Result<(), RulesError> {
        self.sides.iter().try_for_each(SideRules::validate)
    }
}

impl Default for Rules {
    fn default() -> Self {
        Rules::standard()
    }
}
