//! Lookup tables for one battle. `Tables::new` builds them one time from the `Rules`.
//!
//! The tables have two directions. The forward tables answer "where can this piece go".
//! The reverse tables answer "from where can a piece attack this square". The reverse
//! tables come from the same data, thus an offset does not have to be symmetric.

use crate::eval::EvalTables;
use crate::rules::{Atom, Castle, Condition, Offset, Promotions, Rules, RulesError, SideRules};
use crate::types::{Bitboard, Color, Kind, Special, Square, bit};

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

/// One slide direction of a kind, with its number of steps.
#[derive(Clone, Debug)]
pub struct Slide {
    /// The direction, after the mirror for Black.
    pub dir: Offset,
    pub steps: u8,
    /// True if the square index increases along the line. Then the first blocker is the lowest bit.
    pub ascending: bool,
    /// The piece can move to the empty squares of the line.
    pub quiet: bool,
    /// The piece can capture the first enemy piece on the line.
    pub capture: bool,
    /// For each square, the squares of the line from that square, at most `steps` squares.
    pub rays: [Bitboard; 64],
}

/// The leaps and the slides of some atoms.
#[derive(Clone, Debug)]
pub struct Steps {
    /// `leaps[from]`
    pub leaps: [LeapSet; 64],
    pub slides: Vec<Slide>,
}

impl Steps {
    fn new() -> Steps {
        Steps { leaps: [LeapSet::default(); 64], slides: Vec::new() }
    }

    fn add(&mut self, atom: &Atom, forward: i8) {
        let (can_move, can_capture) = (atom.mode.can_move(), atom.mode.can_capture());
        for &(df, dr) in &atom.offsets {
            let dir = (df, dr * forward);
            if atom.max_steps == 1 {
                for from in 0..64u8 {
                    let Some(to) = offset_square(from, dir) else { continue };
                    let set = &mut self.leaps[from as usize];
                    match (can_move, can_capture) {
                        (true, true) => set.both |= bit(to),
                        (true, false) => set.quiet |= bit(to),
                        _ => set.capture |= bit(to),
                    }
                }
                continue;
            }
            let steps = atom.max_steps;
            match self.slides.iter_mut().find(|slide| (slide.dir, slide.steps) == (dir, steps)) {
                Some(slide) => {
                    slide.quiet |= can_move;
                    slide.capture |= can_capture;
                }
                None => self.slides.push(Slide {
                    dir,
                    steps,
                    rays: rays(dir, steps),
                    ascending: ascending(dir),
                    quiet: can_move,
                    capture: can_capture,
                }),
            }
        }
    }

    /// Two atoms can give the same leap target. Keeps each target in one set only.
    fn normalize(&mut self) {
        for set in &mut self.leaps {
            set.both |= set.quiet & set.capture;
            set.quiet &= !set.both;
            set.capture &= !set.both;
        }
    }

    /// The squares that a leap or a slide can get to on an empty board, with any mode.
    fn reach(&self, from: Square) -> Bitboard {
        let leaps = &self.leaps[from as usize];
        self.slides.iter().fold(leaps.both | leaps.quiet | leaps.capture, |set, slide| set | slide.rays[from as usize])
    }

    fn can_capture(&self) -> bool {
        self.slides.iter().any(|slide| slide.capture) || self.leaps.iter().any(|set| set.both | set.capture != 0)
    }
}

/// The atoms of a kind that give their moves the same properties (`Atom::same_group`). The move
/// generation gives the moves group by group, in the order of the first atom of each group.
#[derive(Clone, Copy, Debug)]
pub struct Group {
    pub condition: Condition,
    pub makes_en_passant: bool,
    pub captures_en_passant: bool,
    /// The slides of the group: `KindTables::group_slides[slides.0..slides.1]`.
    pub slides: (u16, u16),
}

/// The possible moves of a piece of a kind that is not simple, from one square, to one or
/// more target squares. The move generation tests the probes of a square in order, and gives
/// the targets of a probe in ascending order. The first probe that can go to a square gives
/// the move there.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    /// The squares that must be empty: the squares that a slide passes. A probe with more than
    /// one target has no path.
    pub path: Bitboard,
    /// The targets where the probe can move if the target is empty.
    pub quiet_targets: Bitboard,
    /// The targets where the probe can capture an enemy.
    pub capture_targets: Bitboard,
    /// The moves promote.
    pub promo: bool,
    /// The moves have no promotion and no special property.
    pub plain: bool,
    /// The probe is only for a piece that has not moved (`Condition::Unmoved`).
    pub unmoved: bool,
    pub quiet_special: Special,
    pub capture_special: Special,
}

/// A line along which a move of a kind makes en passant squares.
#[derive(Clone, Debug)]
pub struct Trail {
    pub condition: Condition,
    /// For each square, the squares of the line, at most the number of steps of the atom.
    pub rays: [Bitboard; 64],
}

/// The tables of one kind of one side.
#[derive(Clone, Debug)]
pub struct KindTables {
    pub groups: Vec<Group>,
    /// `group_leaps[from * groups.len() + group]`: the leaps of each group. The leaps of the
    /// groups of one square are next to each other in memory.
    pub group_leaps: Vec<LeapSet>,
    pub group_slides: Vec<Slide>,
    pub probes: Vec<Probe>,
    /// `probe_ranges[from][captures_only]`: the probes of a piece on `from`, from `probes`. The
    /// list for `captures_only` has no probe that only gives quiet moves with no promotion:
    /// such a probe can only take a square that no capture can use.
    pub probe_ranges: [[(u32, u32); 2]; 64],
    /// `en_passant_reach[from]`: the squares where the atoms that capture en passant can go
    /// on an empty board.
    pub en_passant_reach: [Bitboard; 64],
    /// The atoms with `Condition::Unmoved`, of all groups.
    pub unmoved: Steps,
    /// True if the kind has one group with no condition and no en passant property, and no
    /// promotion. The move generation of such a kind needs only
    /// `SideTables::always`, and the kind has no probes.
    pub simple: bool,
    pub has_unmoved: bool,
    /// A group of the kind can capture en passant.
    pub captures_en_passant: bool,
    pub promotions: Option<Promotions>,
    /// The squares where the kind promotes.
    pub promo_zone: Bitboard,
    /// `promo_targets[from]`: the squares where a move from `from` promotes. A move that goes
    /// backward does not promote.
    pub promo_targets: [Bitboard; 64],
    pub trails: Vec<Trail>,
    /// True if the kind attacks only by the leaps of `SideTables::always`.
    pub leap_attacks_only: bool,
    /// The squares that the kind reaches by the leaps of `SideTables::always`, with any mode. The king zone
    /// of the evaluation uses it.
    pub leap_reach: [Bitboard; 64],
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
    /// The attackers must not have moved: the atoms have `Condition::Unmoved`.
    pub unmoved_only: bool,
    /// For each attacked square, all the squares of the lines of this group.
    pub reach: [Bitboard; 64],
    pub lines: Vec<AttackLine>,
}

/// The tables of one side.
#[derive(Clone, Debug)]
pub struct SideTables {
    /// `always[kind]`: the atoms of the kind with `Condition::Always`, of all groups. The
    /// tables of the kinds are next to each other in memory, because the move generation and
    /// the evaluation read them most.
    pub always: [Steps; Kind::COUNT],
    /// `kinds[kind]`
    pub kinds: [KindTables; Kind::COUNT],
    /// `leap_attackers[kind][target]`: the squares from which a leap of this kind with no
    /// condition can capture on the target.
    pub leap_attackers: [[Bitboard; 64]; Kind::COUNT],
    /// The kinds that have a leap with no condition that can capture.
    pub leap_attacker_kinds: Vec<Kind>,
    /// The same as `leap_attackers` for the leaps with `Condition::Unmoved`.
    pub unmoved_leap_attackers: [[Bitboard; 64]; Kind::COUNT],
    pub unmoved_leap_attacker_kinds: Vec<Kind>,
    pub slide_attackers: Vec<SlideAttackGroup>,
    /// `attack_zone[kind][target]`: the squares from which a piece of the kind can attack the
    /// target on an empty board, by a leap or a slide, with or without a condition.
    pub attack_zone: [[Bitboard; 64]; Kind::COUNT],
    /// `slide_zone[target]`: the squares of the lines along which a slide of this side can
    /// attack the target. A piece that leaves such a square can open a line.
    pub slide_zone: [Bitboard; 64],
    /// The castles, with the squares of this color.
    pub castles: Vec<Castle>,
    /// True if the movement of the king can go from the `from` square of a castle to its `to`
    /// square. Then the move generation removes that king move when the castle is possible.
    pub king_move_to_castle_square: bool,
    /// Bit `kind`: an atom or a castle reads the `moved` flag of the kind. See `zobrist`.
    pub moved_keyed: u8,
}

impl SideTables {
    /// The castle of this side whose king goes from `from` to `to`.
    #[inline]
    pub fn castle(&self, from: Square, to: Square) -> Option<&Castle> {
        self.castles.iter().find(|castle| castle.king_from == from && castle.king_to == to)
    }

    /// The en passant squares of a move of a piece of kind `kind` from `from` to `to`: the
    /// squares that the move passes on each trail of the kind, if those squares are empty in
    /// `occupied`. `moved` is the flag of the piece before the move.
    pub fn trail_squares(&self, kind: Kind, moved: bool, from: Square, to: Square, occupied: Bitboard) -> Bitboard {
        let mut squares = 0;
        for trail in &self.kinds[kind.index()].trails {
            if trail.condition == Condition::Unmoved && moved {
                continue;
            }
            let ray = trail.rays[from as usize];
            if ray & bit(to) == 0 {
                continue;
            }
            let between = ray & !trail.rays[to as usize] & !bit(to);
            if between & occupied == 0 {
                squares |= between;
            }
        }
        squares
    }
}

/// The rules of a battle and the tables that come from them.
#[derive(Clone, Debug)]
pub struct Tables {
    rules: Rules,
    sides: [SideTables; 2],
    eval: EvalTables,
}

impl Tables {
    /// Gives the error of `rules.validate()` if the rules are not valid.
    pub fn new(rules: Rules) -> Result<Tables, RulesError> {
        rules.validate()?;
        Ok(Tables::build(rules))
    }

    /// The tables of ordinary chess.
    pub fn standard() -> Tables {
        Tables::build(Rules::standard())
    }

    /// The rules must be valid.
    fn build(rules: Rules) -> Tables {
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

    /// True if the rules read the `moved` flag of a kind that is not the pawn, the rook, or
    /// the king. Then the Zobrist key has the flag of each kind. See `zobrist`.
    pub fn all_moved_keyed(&self) -> bool {
        const STANDARD: u8 = 1 << Kind::Pawn as u8 | 1 << Kind::Rook as u8 | 1 << Kind::King as u8;
        self.sides.iter().any(|side| side.moved_keyed & !STANDARD != 0)
    }
}

/// The square at an offset from a square, or None if it is off the board.
pub(crate) fn offset_square(from: Square, (df, dr): Offset) -> Option<Square> {
    let f = (from & 7) as i8 + df;
    let r = (from >> 3) as i8 + dr;
    ((0..8).contains(&f) && (0..8).contains(&r)).then(|| (r * 8 + f) as Square)
}

/// For each square, the squares of the line in one direction, at most `steps` squares.
fn rays((df, dr): Offset, steps: u8) -> [Bitboard; 64] {
    let mut rays = [0; 64];
    for (from, ray) in rays.iter_mut().enumerate() {
        let mut s = from as Square;
        for _ in 0..steps {
            let Some(next) = offset_square(s, (df, dr)) else { break };
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

/// The squares on the ranks behind the rank of `from`, from the view of `color`.
fn behind(from: Square, color: Color) -> Bitboard {
    let rank = (from >> 3) as u32;
    match color {
        Color::White => (1u64 << (rank * 8)) - 1,
        Color::Black if rank == 7 => 0,
        Color::Black => !0u64 << ((rank + 1) * 8),
    }
}

/// The tables of a kind, and its atoms with `Condition::Always`.
fn kind_tables(kind: Kind, rules: &SideRules, color: Color) -> (KindTables, Steps) {
    let forward = color.forward();
    let kind_rules = rules.kind(kind);
    let atoms = &kind_rules.atoms;

    // The groups in the order of their first atom, with the steps of their atoms.
    let mut built: Vec<(Atom, Group, Steps)> = Vec::new();
    let mut group_of = Vec::new();
    let mut always = Steps::new();
    let mut unmoved = Steps::new();
    let mut trails = Vec::new();
    for atom in atoms {
        let index = match built.iter().position(|(first, ..)| first.same_group(atom)) {
            Some(index) => index,
            None => {
                let group = Group {
                    condition: atom.condition,
                    makes_en_passant: atom.makes_en_passant,
                    captures_en_passant: false,
                    slides: (0, 0),
                };
                built.push((atom.clone(), group, Steps::new()));
                built.len() - 1
            }
        };
        group_of.push(index);
        let (_, group, steps) = &mut built[index];
        steps.add(atom, forward);
        group.captures_en_passant |= atom.captures_en_passant && atom.mode.can_capture();
        match atom.condition {
            Condition::Always => always.add(atom, forward),
            Condition::Unmoved => unmoved.add(atom, forward),
        }
        if atom.makes_en_passant && atom.max_steps > 1 {
            for &(df, dr) in &atom.offsets {
                trails.push(Trail { condition: atom.condition, rays: rays((df, dr * forward), atom.max_steps) });
            }
        }
    }
    always.normalize();
    unmoved.normalize();
    let mut groups = Vec::new();
    let mut group_slides = Vec::new();
    let mut group_leaps = vec![LeapSet::default(); 64 * built.len()];
    for (index, (_, mut group, mut steps)) in built.into_iter().enumerate() {
        steps.normalize();
        let start = group_slides.len() as u16;
        group_slides.extend(steps.slides);
        group.slides = (start, group_slides.len() as u16);
        let count = group_leaps.len() / 64;
        for (from, set) in steps.leaps.iter().enumerate() {
            group_leaps[from * count + index] = *set;
        }
        groups.push(group);
    }

    let (promotions, promo_zone) = match kind_rules.promotion {
        Some(promotion) => {
            let zone = promotion.zone();
            (Some(promotion.kinds), if color == Color::White { zone } else { zone.swap_bytes() })
        }
        None => (None, 0),
    };
    let promo_targets = std::array::from_fn(|from| promo_zone & !behind(from as Square, color));

    let mut en_passant_steps = Steps::new();
    for atom in atoms.iter().filter(|atom| atom.captures_en_passant && atom.mode.can_capture()) {
        en_passant_steps.add(atom, forward);
    }
    let en_passant_reach = std::array::from_fn(|from| en_passant_steps.reach(from as Square));
    let leap_reach = std::array::from_fn(|from| {
        let set = &always.leaps[from];
        set.both | set.quiet | set.capture
    });
    let leap_attacks_only = !unmoved.can_capture() && always.slides.iter().all(|slide| !slide.capture);
    let simple = groups.len() <= 1
        && groups
            .iter()
            .all(|group| group.condition == Condition::Always && !group.makes_en_passant && !group.captures_en_passant)
        && promotions.is_none();
    // A simple kind needs no probes.
    let (probes, probe_ranges) = if simple {
        (Vec::new(), [[(0, 0); 2]; 64])
    } else {
        build_probes(atoms, &group_of, &groups, forward, &promo_targets)
    };
    let tables = KindTables {
        has_unmoved: groups.iter().any(|group| group.condition == Condition::Unmoved),
        captures_en_passant: groups.iter().any(|group| group.captures_en_passant),
        groups,
        group_leaps,
        group_slides,
        probes,
        probe_ranges,
        en_passant_reach,
        unmoved,
        simple,
        promotions,
        promo_zone,
        promo_targets,
        trails,
        leap_attacks_only,
        leap_reach,
    };
    (tables, always)
}

/// A probe with one target, before the merge of probes.
struct RawProbe {
    path: Bitboard,
    to: Square,
    quiet: bool,
    capture: bool,
    promo: bool,
}

/// The probes of each square. See `Probe`.
fn build_probes(
    atoms: &[Atom],
    group_of: &[usize],
    groups: &[Group],
    forward: i8,
    promo_targets: &[Bitboard; 64],
) -> (Vec<Probe>, [[(u32, u32); 2]; 64]) {
    let mut probes = Vec::new();
    let mut ranges = [[(0, 0); 2]; 64];
    for from in 0..64u8 {
        let start = probes.len();
        for (index, group) in groups.iter().enumerate() {
            let unmoved = group.condition == Condition::Unmoved;
            // (probe, passes squares)
            let mut list: Vec<(RawProbe, bool)> = Vec::new();
            for (atom, _) in atoms.iter().zip(group_of).filter(|(_, g)| **g == index) {
                for &(df, dr) in &atom.offsets {
                    let mut path = 0;
                    let mut at = from;
                    for step in 0..atom.max_steps {
                        let Some(to) = offset_square(at, (df, dr * forward)) else { break };
                        let trail = group.makes_en_passant && step >= 1;
                        let promo = promo_targets[from as usize] & bit(to) != 0;
                        let (quiet, capture) = (atom.mode.can_move(), atom.mode.can_capture());
                        match list.iter_mut().find(|(p, t)| p.to == to && p.path == path && *t == trail) {
                            Some((p, _)) => {
                                p.quiet |= quiet;
                                p.capture |= capture;
                            }
                            None => list.push((RawProbe { path, to, quiet, capture, promo }, trail)),
                        }
                        path |= bit(to);
                        at = to;
                    }
                }
            }
            // Ascending targets. For one target, a probe that makes en passant squares comes
            // first, because the group gives its move that property.
            list.sort_by_key(|(p, trail)| (p.to, !trail));
            let group_start = probes.len();
            for (raw, trail) in list {
                let trail = trail && !raw.promo;
                let quiet_special = if trail { Special::DoubleStep } else { Special::None };
                let capture_special = quiet_special;
                // A probe never gives a move if an earlier probe goes to the same square over
                // fewer squares and can do all that it can do, whenever this probe can be used.
                let target = bit(raw.to);
                let covered = probes[start..].iter().any(|earlier: &Probe| {
                    (earlier.quiet_targets | earlier.capture_targets) & target != 0
                        && earlier.path & !raw.path == 0
                        && (earlier.quiet_targets & target != 0 || !raw.quiet)
                        && (earlier.capture_targets & target != 0 || !raw.capture)
                        && (!earlier.unmoved || unmoved)
                });
                if covered {
                    continue;
                }
                let probe = Probe {
                    path: raw.path,
                    quiet_targets: if raw.quiet { target } else { 0 },
                    capture_targets: if raw.capture { target } else { 0 },
                    promo: raw.promo,
                    plain: !raw.promo && quiet_special == Special::None && capture_special == Special::None,
                    unmoved,
                    quiet_special,
                    capture_special,
                };
                // Consecutive probes of a group with no path and the same properties become one
                // probe with more targets.
                if let Some(last) = probes[group_start..].last_mut().filter(|last| {
                    last.path == 0
                        && probe.path == 0
                        && (last.quiet_targets != 0, last.capture_targets != 0) == (raw.quiet, raw.capture)
                        && last.promo == probe.promo
                        && (last.quiet_special, last.capture_special) == (quiet_special, capture_special)
                }) {
                    last.quiet_targets |= probe.quiet_targets;
                    last.capture_targets |= probe.capture_targets;
                } else {
                    probes.push(probe);
                }
            }
        }
        ranges[from as usize][0] = (start as u32, probes.len() as u32);
        let captures: Vec<Probe> =
            probes[start..].iter().filter(|probe| probe.capture_targets != 0 || probe.promo).copied().collect();
        let captures_start = probes.len();
        probes.extend(captures);
        ranges[from as usize][1] = (captures_start as u32, probes.len() as u32);
    }
    (probes, ranges)
}

fn side_tables(rules: &SideRules, color: Color) -> SideTables {
    let mut always: [Steps; Kind::COUNT] = std::array::from_fn(|_| Steps::new());
    let kinds: [KindTables; Kind::COUNT] = std::array::from_fn(|k| {
        let (tables, steps) = kind_tables(Kind::ALL[k], rules, color);
        always[k] = steps;
        tables
    });
    let castles: Vec<Castle> = rules.castles.iter().map(|castle| castle.for_color(color)).collect();

    let mut leap_attackers = [[0; 64]; Kind::COUNT];
    let mut unmoved_leap_attackers = [[0; 64]; Kind::COUNT];
    for kind in Kind::ALL {
        let k = kind.index();
        for from in 0..64u8 {
            let always = &always[k].leaps[from as usize];
            let mut targets = always.both | always.capture;
            while targets != 0 {
                let to = crate::types::pop_square(&mut targets);
                leap_attackers[k][to as usize] |= bit(from);
            }
            let unmoved = &kinds[k].unmoved.leaps[from as usize];
            let mut targets = unmoved.both | unmoved.capture;
            while targets != 0 {
                let to = crate::types::pop_square(&mut targets);
                unmoved_leap_attackers[k][to as usize] |= bit(from);
            }
        }
    }
    let attacker_kinds = |table: &[[Bitboard; 64]; Kind::COUNT]| -> Vec<Kind> {
        Kind::ALL.into_iter().filter(|kind| table[kind.index()].iter().any(|&set| set != 0)).collect()
    };

    // For each line from a target toward an attacker, and each condition, the kinds that attack
    // along it. An attacker that slides in direction `d` is in direction `-d` from its target.
    let mut attack_lines: Vec<(Offset, u8, Condition, Vec<Kind>)> = Vec::new();
    for kind in Kind::ALL {
        let k = kind.index();
        for (condition, steps) in [(Condition::Always, &always[k]), (Condition::Unmoved, &kinds[k].unmoved)] {
            for slide in steps.slides.iter().filter(|slide| slide.capture) {
                let toward_attacker = (-slide.dir.0, -slide.dir.1);
                let key = (toward_attacker, slide.steps, condition);
                match attack_lines.iter_mut().find(|line| (line.0, line.1, line.2) == key) {
                    Some(line) => line.3.push(kind),
                    None => attack_lines.push((toward_attacker, slide.steps, condition, vec![kind])),
                }
            }
        }
    }
    let mut slide_attackers: Vec<SlideAttackGroup> = Vec::new();
    for (dir, steps, condition, kinds) in attack_lines {
        let line = AttackLine { rays: rays(dir, steps), ascending: ascending(dir) };
        let unmoved_only = condition == Condition::Unmoved;
        let index = match slide_attackers.iter().position(|g| g.kinds == kinds && g.unmoved_only == unmoved_only) {
            Some(index) => index,
            None => {
                slide_attackers.push(SlideAttackGroup { kinds, unmoved_only, reach: [0; 64], lines: Vec::new() });
                slide_attackers.len() - 1
            }
        };
        let group = &mut slide_attackers[index];
        for (reach, ray) in group.reach.iter_mut().zip(&line.rays) {
            *reach |= ray;
        }
        group.lines.push(line);
    }

    // The castle squares are empty when the castle is possible, thus only a move to an empty
    // square can have the same squares as a castle.
    let king = &kinds[Kind::King.index()];
    let king_move_to_castle_square = castles.iter().any(|castle| {
        let from = castle.king_from;
        (always[Kind::King.index()].reach(from) | king.unmoved.reach(from)) & bit(castle.king_to) != 0
    });

    let mut moved_keyed = 0u8;
    for kind in Kind::ALL {
        if kinds[kind.index()].has_unmoved {
            moved_keyed |= 1 << kind.index();
        }
    }
    for castle in &castles {
        moved_keyed |= 1 << Kind::King.index() | 1 << castle.partner.index();
    }

    let attack_zone: [[Bitboard; 64]; Kind::COUNT] = std::array::from_fn(|k| {
        std::array::from_fn(|target| {
            let leaps = leap_attackers[k][target] | unmoved_leap_attackers[k][target];
            slide_attackers
                .iter()
                .filter(|group| group.kinds.contains(&Kind::ALL[k]))
                .fold(leaps, |zone, group| zone | group.reach[target])
        })
    });
    let slide_zone: [Bitboard; 64] =
        std::array::from_fn(|target| slide_attackers.iter().fold(0, |zone, group| zone | group.reach[target]));

    SideTables {
        always,
        attack_zone,
        slide_zone,
        leap_attacker_kinds: attacker_kinds(&leap_attackers),
        unmoved_leap_attacker_kinds: attacker_kinds(&unmoved_leap_attackers),
        leap_attackers,
        unmoved_leap_attackers,
        slide_attackers,
        kinds,
        castles,
        king_move_to_castle_square,
        moved_keyed,
    }
}
