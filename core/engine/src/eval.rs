//! The evaluation of a position. It comes from the movement rules of the battle.
//!
//! The module has two parts:
//!
//! - `EvalTables` has the material value of each kind for each side. `Tables::new` builds it
//!   one time for each battle from the rules data. The module has no table of piece values
//!   and no code for a specific relic.
//! - `Evaluator::evaluate` gives the score of a position in centipawns for the side that has
//!   the move.
//!
//! # The value of a kind
//!
//! The value comes from the *reach* of the kind. Put the piece on a square of a board where
//! each other square is empty with the probability `P_EMPTY`, and has a friend or an enemy
//! with equal probabilities if it is not empty. The reach on that square is the expected value of
//!
//! ```text
//! W_MOVE * (empty squares where the piece can move) + W_ATTACK * (attacked squares with no friend)
//! ```
//!
//! A square of a slide counts only if the squares before it are empty. An atom with
//! `Condition::Unmoved` counts only on the first two ranks, where the pieces start. An atom with
//! `Condition::Near` counts with the probability `P_NEAR` on each square. `reach` is
//! the mean over the squares of the kind. `coverage` is the part of the board that the piece
//! can get to in any number of moves on an empty board, as a mean over the same squares. Then:
//!
//! ```text
//! value = (VALUE_PER_REACH * reach + VALUE_PER_REACH_SQUARED * reach * reach) * (1 - BOUND * (1 - coverage))
//! ```
//!
//! A kind with a promotion (the pawn) goes through the same steps from its atoms, with three
//! changes. Its squares are those from the second rank to the last rank before the promotion
//! zone. It has no coverage factor, because it leaves the board as itself when it promotes.
//! It gets a term for its promotion:
//! `PROMO_SHARE * (value of the best promotion kind) * PROMO_DECAY^(moves to the promotion zone)`.
//! The number of moves is for an empty board from the second rank, thus the double step and
//! the start of the promotion zone change it.
//!
//! A hook adds its targets in the same way: each square of its leg must be empty.
//!
//! A new atom or hook can only add reach and coverage, thus it never makes a value lower.

use std::sync::Arc;

use crate::movegen::{piece_attacks, piece_reach};
use crate::outcome::CLOCK_LIMIT;
use crate::rules::{Atom, Condition, KindRules, Rules, SideRules};
use crate::search::MATE_BOUND;
use crate::state::State;
use crate::tables::{Tables, offset_square};
use crate::types::{Bitboard, Color, Kind, Square, bit, pop_square};

/// The weight of an empty square where a piece can move.
pub const W_MOVE: i32 = 1;
/// The weight of a square that a piece attacks. An attack is worth more than a quiet move.
pub const W_ATTACK: i32 = 3;
/// The probability that a square is empty.
const P_EMPTY: f64 = 0.7;
/// The probability that a square has no friend.
const P_NOT_FRIEND: f64 = 1.0 - (1.0 - P_EMPTY) / 2.0;

// The calibrated constants. `core/README.md` tells how they were set.
const VALUE_PER_REACH: f64 = 18.157;
const VALUE_PER_REACH_SQUARED: f64 = 0.03456;
const BOUND: f64 = 0.178;
const PROMO_SHARE: f64 = 0.2;
const PROMO_DECAY: f64 = 0.5;

/// The mobility term of one officer is `MOBILITY * (reach now - mean reach) / mean reach`.
const MOBILITY: i32 = 40;
/// The largest penalty for enemy attacks on the squares that the king can reach.
const KING_DANGER: i32 = 240;
/// The penalty for a king that has no safe square to go to.
const KING_BOXED: i32 = 25;
/// The rout penalty is `ROUT * 100 / (material that is not the king)`, with `ROUT_MAX` as its limit.
const ROUT: i32 = 300;
const ROUT_MAX: i32 = 400;
/// A piece that the enemy can capture with gain loses `value / THREAT_DIVISOR`.
const THREAT_DIVISOR: i32 = 16;
/// A bonus for the side that has the move.
const TEMPO: i32 = 8;
/// From this value of the clock, the score goes linearly to 0 at `CLOCK_LIMIT`.
pub const CLOCK_FADE_START: u32 = 70;
/// The largest score of `Evaluator::evaluate`. It is less than `MATE_BOUND`, thus a score of
/// the evaluation is never the score of a forced win.
pub const EVAL_LIMIT: i32 = MATE_BOUND - 1;

/// The piece values of ordinary chess in centipawns: pawn, knight, bishop, rook, queen, king.
///
/// The evaluation of the game does not read these numbers. They are for the `FixedValues`
/// opponent of the self-play tool, for the reference AI of the tools, and for the tests of the
/// derived values.
pub const FIXED_VALUES: [i32; Kind::COUNT] = [100, 320, 330, 500, 900, 0];

/// The reach and the coverage of a kind. See the module text.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Profile {
    pub reach: f64,
    pub coverage: f64,
}

/// For one start square: the probability that each square can be moved to, the probability that
/// each square is attacked, and the squares that one move can get to on an empty board.
struct SquareReach {
    quiet: [f64; 64],
    attack: [f64; 64],
    targets: Bitboard,
}

impl SquareReach {
    fn new() -> SquareReach {
        SquareReach { quiet: [0.0; 64], attack: [0.0; 64], targets: 0 }
    }

    /// Adds a target. `clear` is the probability that the squares before the target are empty.
    fn add(&mut self, to: Square, clear: f64, can_move: bool, can_capture: bool) {
        let t = to as usize;
        if can_move {
            self.quiet[t] = self.quiet[t].max(clear);
        }
        if can_capture {
            self.attack[t] = self.attack[t].max(clear);
        }
        self.targets |= bit(to);
    }

    fn reach(&self) -> f64 {
        let quiet: f64 = self.quiet.iter().sum();
        let attack: f64 = self.attack.iter().sum();
        W_MOVE as f64 * P_EMPTY * quiet + W_ATTACK as f64 * P_NOT_FRIEND * attack
    }
}

/// The mean reach and the mean coverage. `squares` has the start squares that count. With no
/// square, the profile is 0: a pawn with the promotion distance 6 promotes on its first step
/// from the second rank, thus it has no square before its zone.
fn profile(reaches: &[SquareReach; 64], squares: Bitboard) -> Profile {
    if squares == 0 {
        return Profile { reach: 0.0, coverage: 0.0 };
    }
    let count = squares.count_ones() as f64;
    let mut reach = 0.0;
    let mut coverage = 0.0;
    let mut starts = squares;
    while starts != 0 {
        let from = pop_square(&mut starts);
        reach += reaches[from as usize].reach();
        // The squares that the piece can get to in any number of moves.
        let mut seen = bit(from);
        let mut frontier = bit(from);
        while frontier != 0 {
            let next = reaches[pop_square(&mut frontier) as usize].targets & !seen;
            seen |= next;
            frontier |= next;
        }
        coverage += (seen.count_ones() - 1) as f64 / 63.0;
    }
    Profile { reach: reach / count, coverage: coverage / count }
}

/// The first two ranks, where an atom with `Condition::Unmoved` counts.
const HOME_RANKS: Bitboard = 0xFFFF;
/// The second rank: the start of the pawns.
const SECOND_RANK: Bitboard = 0xFF00;

/// The part of the time that a piece is near the piece of its `Condition::Near`.
const P_NEAR: f64 = 0.25;

/// The weight of an atom from `from`: 1 with no condition, 1 on a home rank and 0 on each other
/// square for `Condition::Unmoved`, and `P_NEAR` for `Condition::Near`.
fn atom_weight(atom: &Atom, from: Square) -> f64 {
    match atom.condition {
        Condition::Always => 1.0,
        Condition::Unmoved if HOME_RANKS & bit(from) != 0 => 1.0,
        Condition::Unmoved => 0.0,
        Condition::Near { .. } => P_NEAR,
    }
}

/// True if the piece can use the atom from `from` with no other piece: see `atom_weight`.
fn atom_counts(atom: &Atom, from: Square) -> bool {
    atom_weight(atom, from) == 1.0
}

/// The promotion zone of White, or no square.
fn zone(rules: &KindRules) -> Bitboard {
    rules.promotion.map_or(0, |promotion| promotion.zone())
}

/// The profile of a kind with these rules, for White. See the module text.
pub fn kind_profile(rules: &KindRules) -> Profile {
    let zone = zone(rules);
    let reaches: [SquareReach; 64] = std::array::from_fn(|from| {
        let from = from as Square;
        let mut reach = SquareReach::new();
        // A piece in its promotion zone has promoted.
        if zone & bit(from) != 0 {
            return reach;
        }
        for atom in rules.atoms.iter().filter(|atom| atom_weight(atom, from) > 0.0) {
            for &offset in &atom.offsets {
                let mut clear = atom_weight(atom, from);
                let mut s = from;
                for _ in 0..atom.max_steps {
                    let Some(to) = offset_square(s, offset) else { break };
                    reach.add(to, clear, atom.mode.can_move(), atom.mode.can_capture());
                    clear *= P_EMPTY;
                    s = to;
                }
            }
        }
        for hook in &rules.hooks {
            for &(leg, last) in &hook.bends {
                let mut clear = 1.0;
                let mut s = from;
                for squares in 1..=hook.max_leg {
                    let Some(corner) = offset_square(s, leg) else { break };
                    clear *= P_EMPTY;
                    s = corner;
                    if squares >= hook.min_leg
                        && let Some(to) = offset_square(corner, last)
                    {
                        reach.add(to, clear, hook.mode.can_move(), hook.mode.can_capture());
                    }
                }
            }
        }
        reach
    });
    let squares = if rules.promotion.is_some() { !zone & !0xFF } else { !0 };
    profile(&reaches, squares)
}

/// The profile of a kind with these atoms and no promotion.
pub fn officer_profile(atoms: &[Atom]) -> Profile {
    kind_profile(&KindRules { atoms: atoms.to_vec(), promotion: None, hooks: Vec::new() })
}

/// The number of moves from each square to the promotion zone of White on an empty board, by
/// the atoms that can move. None if the piece cannot get to the zone.
fn promotion_steps(rules: &KindRules) -> [Option<u32>; 64] {
    let zone = zone(rules);
    let targets: [Bitboard; 64] = std::array::from_fn(|from| {
        let from = from as Square;
        let mut set = 0;
        for atom in rules.atoms.iter().filter(|atom| atom.mode.can_move() && atom_counts(atom, from)) {
            for &offset in &atom.offsets {
                let mut s = from;
                for _ in 0..atom.max_steps {
                    let Some(to) = offset_square(s, offset) else { break };
                    set |= bit(to);
                    s = to;
                }
            }
        }
        set
    });
    let mut steps: [Option<u32>; 64] = std::array::from_fn(|s| (zone & bit(s as Square) != 0).then_some(0));
    loop {
        let mut changed = false;
        for from in 0..64 {
            if zone & bit(from as Square) != 0 {
                continue;
            }
            let mut set = targets[from];
            let mut best = steps[from];
            while set != 0 {
                if let Some(next) = steps[pop_square(&mut set) as usize] {
                    best = Some(best.map_or(next + 1, |old: u32| old.min(next + 1)));
                }
            }
            if best != steps[from] {
                steps[from] = best;
                changed = true;
            }
        }
        if !changed {
            return steps;
        }
    }
}

fn reach_value(reach: f64) -> f64 {
    VALUE_PER_REACH * reach + VALUE_PER_REACH_SQUARED * reach * reach
}

fn mobility_value(profile: Profile) -> f64 {
    reach_value(profile.reach) * (1.0 - BOUND * (1.0 - profile.coverage))
}

/// The material value of an officer with these atoms, in centipawns.
pub fn officer_value(atoms: &[Atom]) -> i32 {
    mobility_value(officer_profile(atoms)).round() as i32
}

/// The value of the promotion of a piece that is `steps` moves from the promotion zone.
fn promotion_term(best_promotion: i32, steps: Option<u32>) -> f64 {
    steps.map_or(0.0, |steps| PROMO_SHARE * best_promotion as f64 * PROMO_DECAY.powi(steps as i32))
}

/// The evaluation data of one side.
#[derive(Clone, Debug)]
pub struct SideEval {
    /// `value[kind]`: the material value in centipawns. The value of a kind with a promotion
    /// is for a piece on the second rank. The value of the king is not part of the material;
    /// the move ordering uses it.
    pub value: [i32; Kind::COUNT],
    /// `reach[kind]`: the mean reach of the kind.
    pub reach: [f64; Kind::COUNT],
    /// `MOBILITY * 1024 / reach[kind]`
    mobility_scale: [i32; Kind::COUNT],
    /// `advance[kind][square]`: the bonus of a piece of a kind with a promotion on a square. It
    /// is 0 on the second rank, and 0 for a kind with no promotion.
    pub advance: [[i32; 64]; Kind::COUNT],
}

impl SideEval {
    pub fn new(rules: &SideRules, color: Color) -> SideEval {
        let mut value = [0; Kind::COUNT];
        let mut reach = [0.0; Kind::COUNT];
        let mut mobility_scale = [0; Kind::COUNT];
        let profiles = Kind::ALL.map(|kind| kind_profile(rules.kind(kind)));
        for kind in Kind::ALL {
            let profile = profiles[kind.index()];
            reach[kind.index()] = profile.reach;
            value[kind.index()] = mobility_value(profile).round() as i32;
        }
        for kind in Kind::OFFICERS {
            let profile = profiles[kind.index()];
            mobility_scale[kind.index()] =
                if profile.reach > 0.0 { (MOBILITY as f64 * 1024.0 / profile.reach).round() as i32 } else { 0 };
        }

        // The promotion kinds get the values without a promotion term.
        let base = value;
        let mut advance = [[0; 64]; Kind::COUNT];
        for kind in Kind::ALL {
            let kind_rules = rules.kind(kind);
            let Some(promotion) = kind_rules.promotion else { continue };
            let best = promotion.kinds.as_slice().iter().map(|kind| base[kind.index()]).max().unwrap_or(0);
            let steps = promotion_steps(kind_rules);
            let terms: [f64; 64] = std::array::from_fn(|s| promotion_term(best, steps[s]));
            let mut second = SECOND_RANK;
            let mut start = 0.0;
            while second != 0 {
                start += terms[pop_square(&mut second) as usize];
            }
            let start = start / 8.0;
            value[kind.index()] = (reach_value(profiles[kind.index()].reach) + start).round() as i32;
            let zone = zone(kind_rules);
            for (s, bonus) in advance[kind.index()].iter_mut().enumerate() {
                // The tables are for White. Black gets the mirror.
                let white = if color == Color::White { s } else { s ^ 56 };
                if zone & bit(white as Square) == 0 {
                    *bonus = (terms[white] - start).round() as i32;
                }
            }
        }
        SideEval { value, reach, mobility_scale, advance }
    }
}

/// The evaluation data of a battle: `[White, Black]`.
#[derive(Clone, Debug)]
pub struct EvalTables {
    pub sides: [SideEval; 2],
}

impl EvalTables {
    pub fn new(rules: &Rules) -> EvalTables {
        EvalTables {
            sides: [
                SideEval::new(rules.side(Color::White), Color::White),
                SideEval::new(rules.side(Color::Black), Color::Black),
            ],
        }
    }

    /// The material value of a kind for a side, in centipawns.
    pub fn value(&self, color: Color, kind: Kind) -> i32 {
        self.sides[color.index()].value[kind.index()]
    }
}

/// What the evaluation knows about the rules. Only `Derived` is for the game. The other two
/// are opponents for the self-play tool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvalVariant {
    /// All terms come from the rules of the battle.
    Derived,
    /// The material values are those of ordinary chess for each side. The other terms come
    /// from the rules of the battle.
    FixedValues,
    /// All terms come from the rules of ordinary chess. The evaluation does not see the added
    /// movement rules.
    RuleBlind,
}

/// Evaluates the positions of one battle.
#[derive(Clone, Debug)]
pub struct Evaluator {
    /// The tables of the rules that the evaluation believes.
    tables: Arc<Tables>,
    sides: [SideEval; 2],
    /// The threat term. See `SearchOptions::threats`.
    pub threats: bool,
}

impl Evaluator {
    pub fn new(state: &State, variant: EvalVariant) -> Evaluator {
        let tables = match variant {
            EvalVariant::Derived | EvalVariant::FixedValues => state.shared_tables(),
            EvalVariant::RuleBlind => Arc::new(Tables::standard()),
        };
        let mut sides = tables.eval().sides.clone();
        if variant == EvalVariant::FixedValues {
            for side in &mut sides {
                side.value = FIXED_VALUES;
            }
        }
        Evaluator { tables, sides, threats: true }
    }

    /// The material value of a kind for a side, in centipawns.
    #[inline(always)]
    pub fn value(&self, color: Color, kind: Kind) -> i32 {
        self.sides[color.index()].value[kind.index()]
    }

    /// The squares that the king of a side can reach by a leap, and its own square.
    fn king_zone(&self, state: &State, color: Color) -> Bitboard {
        match state.king_square(color) {
            Some(king) => bit(king) | self.tables.side(color).kinds[Kind::King.index()].leap_reach[king as usize],
            None => 0,
        }
    }

    /// The score in centipawns for the side that has the move, from `-EVAL_LIMIT` to `EVAL_LIMIT`.
    pub fn evaluate(&self, state: &State) -> i32 {
        let occupied = state.occupied();
        let zones = [self.king_zone(state, Color::White), self.king_zone(state, Color::Black)];
        let mut score = [0i32; 2];
        let mut attacks = [0u64; 2];
        // The number of officers of a side that attack the zone of the enemy king.
        let mut zone_attackers = [0i32; 2];
        let mut king_moves = [0u64; 2];
        // `attacks_by[color][kind]`: the squares that the pieces of one kind attack.
        let mut attacks_by = [[0u64; Kind::COUNT]; 2];

        for color in Color::ALL {
            let c = color.index();
            let side = self.tables.side(color);
            let eval = &self.sides[c];
            let own = state.color_set(color);
            let enemy_zone = zones[1 - c];

            let mut pawns = state.pieces(color, Kind::Pawn);
            let mut material = pawns.count_ones() as i32 * eval.value[Kind::Pawn.index()];
            let mut total = 0;
            while pawns != 0 {
                let s = pop_square(&mut pawns);
                total += eval.advance[Kind::Pawn.index()][s as usize];
                attacks_by[c][Kind::Pawn.index()] |= piece_attacks(state, side, color, Kind::Pawn, s, occupied);
            }
            attacks[c] |= attacks_by[c][Kind::Pawn.index()];
            for kind in [Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen] {
                let k = kind.index();
                let mut pieces = state.pieces(color, kind);
                material += pieces.count_ones() as i32 * eval.value[k];
                let promotes = side.kinds[k].promotions.is_some();
                while pieces != 0 {
                    let s = pop_square(&mut pieces);
                    if promotes {
                        total += eval.advance[k][s as usize];
                    }
                    let (quiets, attack) = piece_reach(state, side, color, kind, s, occupied);
                    let reach = W_MOVE * quiets.count_ones() as i32 + W_ATTACK * (attack & !own).count_ones() as i32;
                    total += ((reach * eval.mobility_scale[k]) >> 10) - MOBILITY;
                    attacks[c] |= attack;
                    attacks_by[c][k] |= attack;
                    zone_attackers[c] += (attack & enemy_zone != 0) as i32;
                }
            }
            let mut kings = state.pieces(color, Kind::King);
            while kings != 0 {
                let s = pop_square(&mut kings);
                let (quiets, attack) = piece_reach(state, side, color, Kind::King, s, occupied);
                attacks[c] |= attack;
                king_moves[c] |= (quiets | attack) & !own;
            }
            // A side with little material is near a rout. Thus the side that is ahead wants trades.
            if material > 0 {
                total -= (ROUT * 100 / material).min(ROUT_MAX);
            }
            score[c] = total + material;
        }

        for color in Color::ALL {
            let c = color.index();
            let zone = zones[c];
            if zone == 0 {
                continue;
            }
            let foe_attacks = attacks[1 - c];
            let size = zone.count_ones() as i32;
            let hit = (zone & foe_attacks).count_ones() as i32;
            let attackers = zone_attackers[1 - c].min(3);
            score[c] -= KING_DANGER * hit * hit * attackers / (size * size * 3);
            if king_moves[c] & !foe_attacks == 0 {
                score[c] -= KING_BOXED;
            }
        }

        // A piece is under threat if a less valuable enemy piece attacks it, or if an enemy
        // piece attacks it and no friend defends it. The values are those of the battle.
        if self.threats {
            for color in Color::ALL {
                let c = color.index();
                let (own, foe) = (&self.sides[c], &self.sides[1 - c]);
                let undefended = attacks[1 - c] & !attacks[c];
                for kind in [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen] {
                    let value = own.value[kind.index()];
                    let mut danger = undefended;
                    for attacker in [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen] {
                        if foe.value[attacker.index()] * 5 < value * 4 {
                            danger |= attacks_by[1 - c][attacker.index()];
                        }
                    }
                    let threatened = (state.pieces(color, kind) & danger).count_ones() as i32;
                    score[c] -= threatened * value / THREAT_DIVISOR;
                }
            }
        }

        let white = score[0] - score[1];
        let mut result = if state.turn() == Color::White { white } else { -white } + TEMPO;
        // The battle is a draw when the clock gets to its limit. The score goes to 0 before that,
        // thus the side that is ahead prefers a capture.
        if state.clock > CLOCK_FADE_START {
            let left = CLOCK_LIMIT.saturating_sub(state.clock) as i32;
            result = result * left / (CLOCK_LIMIT - CLOCK_FADE_START) as i32;
        }
        // Rules with very strong kinds can give a material sum near the scores of a win.
        result.clamp(-EVAL_LIMIT, EVAL_LIMIT)
    }
}
