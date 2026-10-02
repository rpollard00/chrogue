//! Lookup tables for one battle. `Tables::new` builds them one time from the `Rules`.
//!
//! The tables have two directions. The forward tables answer "where can this piece go".
//! The reverse tables answer "from where can a piece attack this square". The reverse
//! tables come from the same data, thus an offset does not have to be symmetric.

use crate::eval::EvalTables;
use crate::rules::{Atom, DoubleStep, Offset, Rules, SideRules};
use crate::types::{Bitboard, Color, Kind, Square, bit};

/// The leap targets of one kind from one square, divided by what the leap can do there.
#[derive(Clone, Copy, Default, Debug)]
pub struct LeapSet {
    /// Targets where the piece can move or capture.
    pub both: Bitboard,
    /// Targets where the piece can only move to an empty square.
    pub quiet: Bitboard,
    /// Targets where the piece can only capture.
    pub capture: Bitboard,
}

/// One slide direction of a kind.
#[derive(Clone, Debug)]
pub struct Slide {
    /// For each square, the squares of the line from that square to the edge of the board.
    pub rays: [Bitboard; 64],
    /// True if the square index increases along the line. Then the first blocker is the lowest bit.
    pub ascending: bool,
    /// The piece can move to the empty squares of the line.
    pub quiet: bool,
    /// The piece can capture the first enemy piece on the line.
    pub capture: bool,
}

/// One line along which sliders of a side can attack a square.
#[derive(Clone, Debug)]
pub struct AttackLine {
    /// For each attacked square, the squares where an attacker can be.
    pub rays: [Bitboard; 64],
    /// True if the square index increases from the attacked square toward the attacker.
    pub ascending: bool,
}

/// The lines along which the same set of kinds attacks. Ordinary chess has two groups:
/// the files and ranks (rook and queen), and the diagonals (bishop and queen).
#[derive(Clone, Debug)]
pub struct SlideAttackGroup {
    pub kinds: Vec<Kind>,
    /// For each attacked square, all the squares of the lines of this group.
    pub reach: [Bitboard; 64],
    pub lines: Vec<AttackLine>,
}

/// The tables of one side.
#[derive(Clone, Debug)]
pub struct SideTables {
    /// `leaps[kind][from]`
    pub leaps: [[LeapSet; 64]; Kind::COUNT],
    /// `slides[kind]`
    pub slides: [Vec<Slide>; Kind::COUNT],
    /// `leap_attackers[kind][target]`: the squares from which a leap of this kind can capture on the target.
    pub leap_attackers: [[Bitboard; 64]; Kind::COUNT],
    /// The kinds that have a leap that can capture.
    pub leap_attacker_kinds: Vec<Kind>,
    pub slide_attackers: Vec<SlideAttackGroup>,
    /// `pawn_captures[from]`: the two squares where a pawn can capture.
    pub pawn_captures: [Bitboard; 64],
    /// `pawn_attackers[target]`: the squares from which a pawn can capture on the target.
    pub pawn_attackers: [Bitboard; 64],
    /// The squares where a pawn promotes.
    pub promo_zone: Bitboard,
    pub double_step_always: bool,
    pub backward_step: bool,
    pub promotions: [Kind; 4],
    pub castling: bool,
}

/// The rules of a battle and the tables that come from them.
#[derive(Clone, Debug)]
pub struct Tables {
    rules: Rules,
    sides: [SideTables; 2],
    eval: EvalTables,
}

impl Tables {
    /// Panics if `rules.validate()` gives an error.
    pub fn new(rules: Rules) -> Tables {
        if let Err(error) = rules.validate() {
            panic!("{error}");
        }
        let sides =
            [side_tables(rules.side(Color::White), Color::White), side_tables(rules.side(Color::Black), Color::Black)];
        let eval = EvalTables::new(&rules);
        Tables { rules, sides, eval }
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// The evaluation data that comes from the rules: the material value of each kind.
    #[inline(always)]
    pub fn eval(&self) -> &EvalTables {
        &self.eval
    }

    #[inline(always)]
    pub fn side(&self, color: Color) -> &SideTables {
        &self.sides[color.index()]
    }
}

/// The square at an offset from a square, or None if it is off the board.
pub(crate) fn offset_square(from: Square, (df, dr): Offset) -> Option<Square> {
    let f = (from & 7) as i8 + df;
    let r = (from >> 3) as i8 + dr;
    ((0..8).contains(&f) && (0..8).contains(&r)).then(|| (r * 8 + f) as Square)
}

/// For each square, the squares of the line in one direction.
fn rays((df, dr): Offset) -> [Bitboard; 64] {
    let mut rays = [0; 64];
    for (from, ray) in rays.iter_mut().enumerate() {
        let mut s = from as Square;
        while let Some(next) = offset_square(s, (df, dr)) {
            *ray |= bit(next);
            s = next;
        }
    }
    rays
}

/// True if the square index increases along a direction.
fn ascending((df, dr): Offset) -> bool {
    dr as i32 * 8 + df as i32 > 0
}

fn side_tables(rules: &SideRules, color: Color) -> SideTables {
    let forward = color.forward();
    let mut t = SideTables {
        leaps: [[LeapSet::default(); 64]; Kind::COUNT],
        slides: std::array::from_fn(|_| Vec::new()),
        leap_attackers: [[0; 64]; Kind::COUNT],
        leap_attacker_kinds: Vec::new(),
        slide_attackers: Vec::new(),
        pawn_captures: [0; 64],
        pawn_attackers: [0; 64],
        promo_zone: 0,
        double_step_always: rules.pawn.double_step == DoubleStep::Always,
        backward_step: rules.pawn.backward_step,
        promotions: rules.pawn.promotions,
        castling: rules.castling,
    };

    for from in 0..64u8 {
        for df in [-1, 1] {
            if let Some(to) = offset_square(from, (df, forward)) {
                t.pawn_captures[from as usize] |= bit(to);
                t.pawn_attackers[to as usize] |= bit(from);
            }
        }
        let rank = from >> 3;
        let to_last_rank = if color == Color::White { 7 - rank } else { rank };
        if to_last_rank <= rules.pawn.promo_distance {
            t.promo_zone |= bit(from);
        }
    }

    // The direction of each slide, after the mirror for Black, with its two abilities.
    let mut dirs: [Vec<(Offset, bool, bool)>; Kind::COUNT] = std::array::from_fn(|_| Vec::new());
    for kind in Kind::OFFICERS {
        let k = kind.index();
        for atom in &rules.kind(kind).atoms {
            match atom {
                Atom::Leap { offsets, mode } => {
                    for &(df, dr) in offsets {
                        for from in 0..64u8 {
                            let Some(to) = offset_square(from, (df, dr * forward)) else { continue };
                            let set = &mut t.leaps[k][from as usize];
                            if mode.can_move() && mode.can_capture() {
                                set.both |= bit(to);
                            } else if mode.can_move() {
                                set.quiet |= bit(to);
                            } else {
                                set.capture |= bit(to);
                            }
                            if mode.can_capture() {
                                t.leap_attackers[k][to as usize] |= bit(from);
                            }
                        }
                    }
                }
                Atom::Slide { dirs: atom_dirs, mode } => {
                    for &(df, dr) in atom_dirs {
                        let dir = (df, dr * forward);
                        match dirs[k].iter_mut().find(|entry| entry.0 == dir) {
                            Some(entry) => {
                                entry.1 |= mode.can_move();
                                entry.2 |= mode.can_capture();
                            }
                            None => dirs[k].push((dir, mode.can_move(), mode.can_capture())),
                        }
                    }
                }
            }
        }
    }

    // Two atoms can give the same leap target. Keep each target in one set only.
    for sets in &mut t.leaps {
        for set in sets {
            set.both |= set.quiet & set.capture;
            set.quiet &= !set.both;
            set.capture &= !set.both;
        }
    }

    t.leap_attacker_kinds =
        Kind::OFFICERS.into_iter().filter(|kind| t.leap_attackers[kind.index()].iter().any(|&set| set != 0)).collect();

    // For each direction from a target toward an attacker, the kinds that attack along it.
    let mut attack_dirs: Vec<(Offset, Vec<Kind>)> = Vec::new();
    for kind in Kind::OFFICERS {
        let k = kind.index();
        for &(dir, quiet, capture) in &dirs[k] {
            t.slides[k].push(Slide { rays: rays(dir), ascending: ascending(dir), quiet, capture });
            if !capture {
                continue;
            }
            // An attacker that slides in direction `dir` is in direction `-dir` from its target.
            let toward_attacker = (-dir.0, -dir.1);
            match attack_dirs.iter_mut().find(|entry| entry.0 == toward_attacker) {
                Some(entry) => entry.1.push(kind),
                None => attack_dirs.push((toward_attacker, vec![kind])),
            }
        }
    }
    for (dir, kinds) in attack_dirs {
        let line = AttackLine { rays: rays(dir), ascending: ascending(dir) };
        let group = match t.slide_attackers.iter_mut().find(|group| group.kinds == kinds) {
            Some(group) => group,
            None => {
                t.slide_attackers.push(SlideAttackGroup { kinds, reach: [0; 64], lines: Vec::new() });
                t.slide_attackers.last_mut().expect("the list has the new group")
            }
        };
        for (reach, ray) in group.reach.iter_mut().zip(&line.rays) {
            *reach |= ray;
        }
        group.lines.push(line);
    }
    t
}
