//! The movement rules as data. Each side of a battle has its own `SideRules`.
//!
//! The engine has no code for a specific relic. A relic is an edit of `SideRules::standard()`.

use crate::types::{Color, Kind};

/// A step of (file, rank) from the view of White. For Black, the engine mirrors the rank step.
pub type Offset = (i8, i8);

pub const KNIGHT: [Offset; 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
pub const CAMEL: [Offset; 8] = [(1, 3), (3, 1), (3, -1), (1, -3), (-1, -3), (-3, -1), (-3, 1), (-1, 3)];
pub const KING: [Offset; 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
pub const ORTHO: [Offset; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
pub const DIAG: [Offset; 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];

/// What a movement atom can do on its target square.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// One part of the movement of a kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Atom {
    /// One jump to each offset. Pieces between the two squares do not block the jump.
    Leap { offsets: Vec<Offset>, mode: Mode },
    /// Steps in each direction until a piece blocks the line.
    Slide { dirs: Vec<Offset>, mode: Mode },
}

impl Atom {
    pub fn leap(offsets: &[Offset], mode: Mode) -> Atom {
        Atom::Leap { offsets: offsets.to_vec(), mode }
    }

    pub fn slide(dirs: &[Offset], mode: Mode) -> Atom {
        Atom::Slide { dirs: dirs.to_vec(), mode }
    }
}

/// The movement of a knight, bishop, rook, queen, or king.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KindRules {
    pub atoms: Vec<Atom>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DoubleStep {
    /// A pawn can move two squares only on its first move.
    FirstMove,
    /// A pawn can always move two squares.
    Always,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PawnRules {
    pub double_step: DoubleStep,
    /// The number of ranks before the last rank where the promotion zone starts. 0 is ordinary chess.
    pub promo_distance: u8,
    /// A pawn can move one square backward to an empty square.
    pub backward_step: bool,
    /// The kinds that a pawn can become, in the order of the generated moves.
    pub promotions: [Kind; 4],
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SideRules {
    /// The rules for knight, bishop, rook, queen, and king, in this order.
    pub kinds: [KindRules; 5],
    pub pawn: PawnRules,
    pub castling: bool,
}

/// The names of the rule flags of the TypeScript engine (`MoveRules` in `src/engine/types.ts`).
pub const FLAG_NAMES: [&str; 6] = ["forcedMarch", "backpedal", "earlyPromo", "kingKnight", "longLeap", "sidestep"];

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RulesError {
    UnknownFlag(String),
    /// The kind has an offset of (0, 0) or an offset that is larger than the board.
    BadOffset(Kind, Offset),
    /// The promotion zone must leave one rank or more for the pawns.
    BadPromoDistance(u8),
}

impl std::fmt::Display for RulesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulesError::UnknownFlag(name) => write!(f, "The rule flag \"{name}\" is unknown"),
            RulesError::BadOffset(kind, (df, dr)) => {
                write!(f, "The offset ({df}, {dr}) of kind {} is not permitted", kind.letter())
            }
            RulesError::BadPromoDistance(d) => write!(f, "The promotion distance {d} is more than 6"),
        }
    }
}

impl std::error::Error for RulesError {}

impl SideRules {
    /// Ordinary chess.
    pub fn standard() -> SideRules {
        use Mode::MoveOrCapture as Both;
        SideRules {
            kinds: [
                KindRules { atoms: vec![Atom::leap(&KNIGHT, Both)] },
                KindRules { atoms: vec![Atom::slide(&DIAG, Both)] },
                KindRules { atoms: vec![Atom::slide(&ORTHO, Both)] },
                KindRules { atoms: vec![Atom::slide(&ORTHO, Both), Atom::slide(&DIAG, Both)] },
                KindRules { atoms: vec![Atom::leap(&KING, Both)] },
            ],
            pawn: PawnRules {
                double_step: DoubleStep::FirstMove,
                promo_distance: 0,
                backward_step: false,
                promotions: [Kind::Queen, Kind::Knight, Kind::Rook, Kind::Bishop],
            },
            castling: true,
        }
    }

    /// The rules of a kind. Panics for `Kind::Pawn`, because the pawn has `PawnRules`.
    pub fn kind(&self, kind: Kind) -> &KindRules {
        &self.kinds[officer_index(kind)]
    }

    /// See `kind`.
    pub fn kind_mut(&mut self, kind: Kind) -> &mut KindRules {
        &mut self.kinds[officer_index(kind)]
    }

    /// Adds an atom to the movement of a kind.
    pub fn with_atom(mut self, kind: Kind, atom: Atom) -> SideRules {
        self.kind_mut(kind).atoms.push(atom);
        self
    }

    /// Replaces the movement of a kind.
    pub fn with_kind(mut self, kind: Kind, atoms: Vec<Atom>) -> SideRules {
        self.kind_mut(kind).atoms = atoms;
        self
    }

    pub fn with_pawn(mut self, edit: impl FnOnce(&mut PawnRules)) -> SideRules {
        edit(&mut self.pawn);
        self
    }

    pub fn with_castling(mut self, castling: bool) -> SideRules {
        self.castling = castling;
        self
    }

    /// Pawns can always move two squares forward.
    pub fn forced_march(self) -> SideRules {
        self.with_pawn(|pawn| pawn.double_step = DoubleStep::Always)
    }

    /// Pawns can move one square backward to an empty square.
    pub fn backpedal(self) -> SideRules {
        self.with_pawn(|pawn| pawn.backward_step = true)
    }

    /// Pawns promote one rank earlier.
    pub fn early_promo(self) -> SideRules {
        self.with_pawn(|pawn| pawn.promo_distance = 1)
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

    /// Applies one rule flag of the TypeScript engine by its name.
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

    /// Ordinary chess with the given rule flags of the TypeScript engine.
    pub fn from_flags<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<SideRules, RulesError> {
        names.into_iter().try_fold(SideRules::standard(), SideRules::with_flag)
    }

    pub fn validate(&self) -> Result<(), RulesError> {
        if self.pawn.promo_distance > 6 {
            return Err(RulesError::BadPromoDistance(self.pawn.promo_distance));
        }
        for kind in Kind::OFFICERS {
            for atom in &self.kind(kind).atoms {
                let (Atom::Leap { offsets, .. } | Atom::Slide { dirs: offsets, .. }) = atom;
                for &(df, dr) in offsets {
                    if (df, dr) == (0, 0) || df.abs() > 7 || dr.abs() > 7 {
                        return Err(RulesError::BadOffset(kind, (df, dr)));
                    }
                }
            }
        }
        Ok(())
    }
}

fn officer_index(kind: Kind) -> usize {
    assert!(kind != Kind::Pawn, "The pawn has PawnRules, not KindRules");
    kind.index() - 1
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

    pub fn validate(&self) -> Result<(), RulesError> {
        self.sides.iter().try_for_each(SideRules::validate)
    }
}

impl Default for Rules {
    fn default() -> Self {
        Rules::standard()
    }
}
