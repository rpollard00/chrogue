//! Move generation and attack detection.
//!
//! Each kind, the pawn too, moves by the tables that come from the rules data. There are three
//! paths, and they give the same moves:
//!
//! - A kind with one group of atoms with no condition and no special property, and no
//!   promotion (`KindTables::simple`), takes the bitboards of its leaps and slides.
//! - The other kinds test the probes of the square of the piece (`Probe`).
//! - A piece that can capture en passant goes group by group: see `add_group_moves`. Its
//!   documentation has the rules of the order and of the properties of the moves.
//!
//! The castles come from the rows of `SideRules::castles`.

use crate::rules::{Condition, Promotions};
use crate::state::{State, Undo};
use crate::tables::{Group, HookAttackLine, HookSlide, KindTables, LeapSet, SideTables, Slide, Steps};
use crate::types::{Bitboard, Color, Kind, Move, MoveList, Special, Square, bit, pop_square};

/// The first piece on a line, or None if the line has no piece.
#[inline(always)]
fn first_blocker(blockers: Bitboard, ascending: bool) -> Option<Square> {
    if blockers == 0 {
        None
    } else if ascending {
        Some(blockers.trailing_zeros() as Square)
    } else {
        Some(63 - blockers.leading_zeros() as Square)
    }
}

/// The empty squares of a slide from `from` before the first piece, and the square of that piece.
#[inline(always)]
fn slide_parts(slide: &Slide, from: Square, occupied: Bitboard) -> (Bitboard, Bitboard) {
    let ray = slide.rays[from as usize];
    match first_blocker(ray & occupied, slide.ascending) {
        Some(blocker) => (ray & !slide.rays[blocker as usize] & !bit(blocker), bit(blocker)),
        None => (ray, 0),
    }
}

/// The squares where the last step of a hook from `from` can end: the leg is empty to its corner.
#[inline(always)]
fn hook_targets(hook: &HookSlide, from: Square, occupied: Bitboard) -> Bitboard {
    let (empty, _) = slide_parts(&hook.leg, from, occupied);
    hook.last_step(empty & !hook.near[from as usize])
}

/// The first square of a line.
#[inline(always)]
fn first_square(ray: Bitboard, ascending: bool) -> Bitboard {
    match first_blocker(ray, ascending) {
        Some(s) => bit(s),
        None => 0,
    }
}

#[inline(always)]
fn has_moved(state: &State, s: Square) -> bool {
    state.piece_at(s).is_some_and(|piece| piece.moved)
}

/// True if a condition is true for the piece of `color` on `s`.
#[inline(always)]
pub fn condition_holds(state: &State, condition: Condition, color: Color, s: Square) -> bool {
    match condition {
        Condition::Always => true,
        Condition::Unmoved => !has_moved(state, s),
        Condition::Near { kind, range } => state.pieces(color, kind) & Condition::near_zone(s, range) != 0,
    }
}

/// The pieces that make a condition true for the piece of `color` on `s`. An attack by an atom
/// with the condition ends when no such piece is left.
#[inline(always)]
fn condition_anchors(state: &State, condition: Condition, color: Color, s: Square) -> Bitboard {
    match condition {
        Condition::Near { kind, range } => state.pieces(color, kind) & Condition::near_zone(s, range),
        _ => 0,
    }
}

/// The pieces of `color` that the enemy cannot capture: the pieces with a shield of the side
/// (`Shield`).
pub fn shielded(state: &State, color: Color) -> Bitboard {
    let mut set = 0;
    for shield in &state.rules().side(color).shields {
        let mut protectors = state.pieces(color, shield.protector);
        let mut zone = 0;
        while protectors != 0 {
            zone |= Condition::near_zone(pop_square(&mut protectors), shield.range);
        }
        set |= zone & state.pieces(color, shield.protected);
    }
    set
}

/// The pieces that a piece of `color` can capture: the pieces of the other side with no shield.
#[inline(always)]
fn capturable(state: &State, color: Color) -> Bitboard {
    let them = color.other();
    let foes = state.color_set(them);
    if state.rules().side(them).shields.is_empty() { foes } else { foes & !shielded(state, them) }
}

/// True if a piece of side `by` attacks the square. An attack is a move that can capture there,
/// or that could capture there if the piece on the square had no shield.
pub fn is_attacked(state: &State, s: Square, by: Color) -> bool {
    let side = state.tables().side(by);
    let theirs = state.color_set(by);
    let target = s as usize;
    for &kind in &side.leap_attacker_kinds {
        if side.leap_attackers[kind.index()][target] & state.kind_set(kind) & theirs != 0 {
            return true;
        }
    }
    for leaps in &side.conditional_leap_attackers {
        let mut attackers = leaps.from[target] & state.kind_set(leaps.kind) & theirs;
        while attackers != 0 {
            if condition_holds(state, leaps.condition, by, pop_square(&mut attackers)) {
                return true;
            }
        }
    }
    let occupied = state.occupied();
    for group in &side.slide_attackers {
        let sliders = group.kinds.iter().fold(0, |set, &kind| set | state.kind_set(kind)) & theirs;
        if group.reach[target] & sliders == 0 {
            continue;
        }
        for line in &group.lines {
            let ray = line.rays[target];
            if ray & sliders == 0 {
                continue;
            }
            if let Some(blocker) = first_blocker(ray & occupied, line.ascending)
                && sliders & bit(blocker) != 0
                && condition_holds(state, group.condition, by, blocker)
            {
                return true;
            }
        }
    }
    for line in &side.hook_attackers {
        if hook_attacker(state, line, target, theirs, occupied).is_some() {
            return true;
        }
    }
    false
}

/// The piece of `theirs` that attacks the target along a line of hooks, and the corner of its leg.
#[inline(always)]
fn hook_attacker(
    state: &State,
    line: &HookAttackLine,
    target: usize,
    theirs: Bitboard,
    occupied: Bitboard,
) -> Option<(Square, Square)> {
    let corner = line.corner[target];
    if corner == 64 || occupied & bit(corner) != 0 {
        return None;
    }
    let ray = line.rays[corner as usize];
    let attackers = line.kinds.iter().fold(0, |set, &kind| set | state.kind_set(kind)) & theirs;
    if ray & attackers == 0 {
        return None;
    }
    let blocker = first_blocker(ray & occupied, line.ascending)?;
    (attackers & bit(blocker) != 0 && line.near[corner as usize] & bit(blocker) == 0).then_some((blocker, corner))
}

/// True if the king of a side is attacked. A side with no king is never in check.
#[inline]
pub fn in_check(state: &State, color: Color) -> bool {
    match state.king_square(color) {
        Some(king) => is_attacked(state, king, color.other()),
        None => false,
    }
}

/// The squares where a move of a piece that is not a king can end the check of the king of
/// `color` (the king on the lowest square). For each enemy piece that attacks the king: the
/// square of the piece, and for a slide or a hook the squares that the piece passes. A move
/// must capture each such piece or block each such slide or hook. A leap cannot be blocked. A
/// capture of a piece that makes the condition of the attack true can also end the check. All the
/// squares if the king is not in check or the side has no king.
///
/// The attackers are those of `is_attacked`, thus a move of a piece that is not a king to
/// another square (an en passant capture: with its victim on another square) leaves the king
/// in check.
pub fn evasion_squares(state: &State, color: Color) -> Bitboard {
    let Some(king) = state.king_square(color) else { return !0 };
    let by = color.other();
    let side = state.tables().side(by);
    let theirs = state.color_set(by);
    let target = king as usize;
    let mut squares = !0;
    for &kind in &side.leap_attacker_kinds {
        let mut attackers = side.leap_attackers[kind.index()][target] & state.kind_set(kind) & theirs;
        while attackers != 0 {
            squares &= bit(pop_square(&mut attackers));
        }
    }
    for leaps in &side.conditional_leap_attackers {
        let mut attackers = leaps.from[target] & state.kind_set(leaps.kind) & theirs;
        while attackers != 0 {
            let from = pop_square(&mut attackers);
            if condition_holds(state, leaps.condition, by, from) {
                squares &= bit(from) | condition_anchors(state, leaps.condition, by, from);
            }
        }
    }
    let occupied = state.occupied();
    for group in &side.slide_attackers {
        let sliders = group.kinds.iter().fold(0, |set, &kind| set | state.kind_set(kind)) & theirs;
        if group.reach[target] & sliders == 0 {
            continue;
        }
        for line in &group.lines {
            let ray = line.rays[target];
            if ray & sliders == 0 {
                continue;
            }
            if let Some(blocker) = first_blocker(ray & occupied, line.ascending)
                && sliders & bit(blocker) != 0
                && condition_holds(state, group.condition, by, blocker)
            {
                // The squares of the line from the king to the attacker, with the attacker.
                squares &= ray & !line.rays[blocker as usize] | condition_anchors(state, group.condition, by, blocker);
            }
        }
    }
    for line in &side.hook_attackers {
        if let Some((attacker, corner)) = hook_attacker(state, line, target, theirs, occupied) {
            // The corner and the squares of the leg, with the attacker.
            let ray = line.rays[corner as usize];
            squares &= bit(corner) | ray & !line.rays[attacker as usize];
        }
    }
    squares
}

/// False if the move that `make` just played (with its `undo`) cannot have put the side to
/// move in check, if that side was not in check before the move: the moved piece cannot attack
/// the king from its `to` square (`SideTables::attack_zone`), and the move leaves no square of
/// a line toward the king (`SideTables::slide_zone`). A castle can always give check.
///
/// The search uses it to skip `in_check`. It is not valid when the side to move was in check
/// before the move, as at the root of a search from a state where the side that does not have
/// the move is in check.
#[inline(always)]
pub fn may_give_check(state: &State, m: Move, undo: &Undo) -> bool {
    if m.special == Special::Castle {
        return true;
    }
    let Some(king) = state.king_square(state.turn()) else { return false };
    let Some(piece) = state.piece_at(m.to) else { return true };
    let side = state.tables().side(piece.color);
    // A piece that makes a condition true can give another piece an attack.
    if side.anchors & (1 << piece.kind.index()) != 0 {
        return true;
    }
    let left = if m.special == Special::EnPassant { bit(m.from) | bit(undo.captured_square) } else { bit(m.from) };
    bit(m.to) & side.attack_zone[piece.kind.index()][king as usize] != 0 || left & side.slide_zone[king as usize] != 0
}

/// Adds the pseudo moves of `color` that can end a check, with the `squares` of
/// `evasion_squares`: the moves of the king, the castles, and the moves of the other pieces
/// that end on `squares` or capture en passant a victim there. The legal moves of a side in
/// check are the legal moves of this list.
pub fn evasion_moves(state: &State, color: Color, squares: Bitboard, list: &mut MoveList) {
    let side = state.tables().side(color);
    let own = state.color_set(color);
    let start = list.len();
    // With no empty square to block on, the other pieces can end the check only by a capture.
    let captures_only = squares & !state.occupied() == 0;
    for kind in Kind::ALL {
        let only = captures_only && kind != Kind::King;
        add_kind_moves(state, side, kind, color, state.kind_set(kind) & own, only, list);
    }
    let kings = state.kind_set(Kind::King) & own;
    if !side.castles.is_empty() {
        let mut each = kings;
        while each != 0 {
            add_castles(state, side, color, pop_square(&mut each), start, list);
        }
    }
    let victim = bit(state.ep_victim());
    retain_from(list, start, |m| {
        let removes = if m.special == Special::EnPassant { victim } else { 0 };
        kings & bit(m.from) != 0 || (bit(m.to) | removes) & squares != 0
    });
}

/// The squares where some steps from `from` can go: (empty squares, squares with an enemy).
#[inline(always)]
fn step_targets(steps: &Steps, from: Square, occupied: Bitboard, foes: Bitboard) -> (Bitboard, Bitboard) {
    let leaps = &steps.leaps[from as usize];
    let mut captures = (leaps.both | leaps.capture) & foes;
    let mut quiets = (leaps.both | leaps.quiet) & !occupied;
    for slide in &steps.slides {
        let (empty, hit) = slide_parts(slide, from, occupied);
        if slide.quiet {
            quiets |= empty;
        }
        if slide.capture {
            captures |= hit & foes;
        }
    }
    for hook in &steps.hooks {
        let targets = hook_targets(hook, from, occupied);
        if hook.leg.quiet {
            quiets |= targets & !occupied;
        }
        if hook.leg.capture {
            captures |= targets & foes;
        }
    }
    (quiets, captures)
}

/// The reach of some steps from `from`: the empty squares where the piece can move, and the
/// squares that it attacks. An attacked square can be empty or can have a piece of either color.
#[inline(always)]
fn step_reach(steps: &Steps, from: Square, occupied: Bitboard) -> (Bitboard, Bitboard) {
    let leaps = &steps.leaps[from as usize];
    let mut quiets = (leaps.both | leaps.quiet) & !occupied;
    let mut attacks = leaps.both | leaps.capture;
    for slide in &steps.slides {
        let (empty, hit) = slide_parts(slide, from, occupied);
        if slide.quiet {
            quiets |= empty;
        }
        if slide.capture {
            attacks |= empty | hit;
        }
    }
    for hook in &steps.hooks {
        let targets = hook_targets(hook, from, occupied);
        if hook.leg.quiet {
            quiets |= targets & !occupied;
        }
        if hook.leg.capture {
            attacks |= targets;
        }
    }
    (quiets, attacks)
}

/// The reach of the piece of `color` on `from`: the empty squares where it can move, and the
/// squares that it attacks. The atoms with a condition count if the condition is true.
#[inline]
pub fn piece_reach(
    state: &State,
    side: &SideTables,
    color: Color,
    kind: Kind,
    from: Square,
    occupied: Bitboard,
) -> (Bitboard, Bitboard) {
    let tables = &side.kinds[kind.index()];
    let (mut quiets, mut attacks) = step_reach(&side.always[kind.index()], from, occupied);
    for (condition, steps) in &tables.conditional {
        if condition_holds(state, *condition, color, from) {
            let (more_quiets, more_attacks) = step_reach(steps, from, occupied);
            quiets |= more_quiets;
            attacks |= more_attacks;
        }
    }
    (quiets, attacks)
}

/// The squares that the piece of `color` on `from` attacks.
#[inline(always)]
pub fn piece_attacks(
    state: &State,
    side: &SideTables,
    color: Color,
    kind: Kind,
    from: Square,
    occupied: Bitboard,
) -> Bitboard {
    let tables = &side.kinds[kind.index()];
    if tables.leap_attacks_only {
        let leaps = &side.always[kind.index()].leaps[from as usize];
        leaps.both | leaps.capture
    } else {
        piece_reach(state, side, color, kind, from, occupied).1
    }
}

#[inline(always)]
fn push_promotions(list: &mut MoveList, from: Square, to: Square, promotions: &Option<Promotions>, special: Special) {
    if let Some(promotions) = promotions {
        for &kind in promotions.as_slice() {
            list.push(Move { from, to, promo: Some(kind), special });
        }
    }
}

/// The targets of one group: (empty squares, squares with an enemy, targets that pass squares
/// on a slide of a group that makes en passant squares).
#[inline(never)]
fn group_targets(
    tables: &KindTables,
    group: &Group,
    leaps: &LeapSet,
    from: Square,
    occupied: Bitboard,
    foes: Bitboard,
) -> (Bitboard, Bitboard, Bitboard) {
    let mut captures = (leaps.both | leaps.capture) & foes;
    let mut quiets = (leaps.both | leaps.quiet) & !occupied;
    let mut trail = 0;
    for slide in &tables.group_slides[group.slides.0 as usize..group.slides.1 as usize] {
        let (empty, hit) = slide_parts(slide, from, occupied);
        let mut given = 0;
        if slide.quiet {
            given |= empty;
        }
        if slide.capture {
            given |= hit & foes;
        }
        quiets |= given & !occupied;
        captures |= given & foes;
        if group.makes_en_passant {
            trail |= given & !first_square(slide.rays[from as usize], slide.ascending);
        }
    }
    (quiets, captures, trail)
}

/// The empty squares where a capture of a group can go if the square has an enemy.
#[inline(always)]
fn capture_reach(tables: &KindTables, group: &Group, leaps: &LeapSet, from: Square, occupied: Bitboard) -> Bitboard {
    let mut reach = (leaps.both | leaps.capture) & !occupied;
    for slide in &tables.group_slides[group.slides.0 as usize..group.slides.1 as usize] {
        if slide.capture {
            reach |= slide_parts(slide, from, occupied).0;
        }
    }
    reach
}

/// Adds the moves of a piece of a kind that is not simple, group by group. This is the
/// definition of the moves of such a piece; the probes give the same moves.
///
/// - The groups come in the order of their first atom. A group gives its targets in ascending
///   order, then its en passant captures.
/// - A target that an earlier group gave is not given again.
/// - An en passant capture takes the place of a quiet move to the same square.
/// - A target in the promotion zone that is not behind the piece gives one move for each
///   promotion kind. A promotion makes no en passant squares.
/// - A target is a `DoubleStep` move if a slide of a group with `makes_en_passant` gives it
///   after one square or more.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn add_group_moves(
    state: &State,
    tables: &KindTables,
    color: Color,
    from: Square,
    occupied: Bitboard,
    foes: Bitboard,
    captures_only: bool,
    list: &mut MoveList,
) {
    let promo = tables.promo_targets[from as usize];
    let count = tables.groups.len();
    let leaps = &tables.group_leaps[from as usize * count..from as usize * count + count];
    let mut en_passant = 0;
    let ep = state.ep_squares();
    if ep != 0 && tables.captures_en_passant && foes & bit(state.ep_victim()) != 0 {
        for (group, leaps) in tables.groups.iter().zip(leaps) {
            if group.captures_en_passant && condition_holds(state, group.condition, color, from) {
                en_passant |= capture_reach(tables, group, leaps, from, occupied) & ep;
            }
        }
    }
    let mut en_passant_left = en_passant;
    let mut given = 0;
    for (group, leaps) in tables.groups.iter().zip(leaps) {
        if !condition_holds(state, group.condition, color, from) {
            continue;
        }
        let (quiets, captures, trail) = group_targets(tables, group, leaps, from, occupied, foes);
        let mut targets = (quiets | captures) & !given & !en_passant;
        given |= quiets | captures;
        if captures_only {
            targets &= captures | promo;
        }
        while targets != 0 {
            let to = pop_square(&mut targets);
            if promo & bit(to) != 0 {
                push_promotions(list, from, to, &tables.promotions, Special::None);
            } else {
                let special = if trail & bit(to) != 0 { Special::DoubleStep } else { Special::None };
                list.push(Move { from, to, promo: None, special });
            }
        }
        if en_passant_left != 0 && group.captures_en_passant {
            let mut targets = capture_reach(tables, group, leaps, from, occupied) & en_passant_left;
            en_passant_left &= !targets;
            while targets != 0 {
                let to = pop_square(&mut targets);
                if promo & bit(to) != 0 {
                    push_promotions(list, from, to, &tables.promotions, Special::EnPassant);
                } else {
                    list.push(Move { from, to, promo: None, special: Special::EnPassant });
                }
            }
        }
    }
}

/// Adds the moves of the pieces of one kind, or of one piece if `pieces` has one square.
#[inline(always)]
fn add_kind_moves(
    state: &State,
    side: &SideTables,
    kind: Kind,
    color: Color,
    mut pieces: Bitboard,
    captures_only: bool,
    list: &mut MoveList,
) {
    let tables = &side.kinds[kind.index()];
    if !tables.simple {
        add_group_kind_moves(state, tables, color, pieces, captures_only, list);
        return;
    }
    let steps = &side.always[kind.index()];
    let occupied = state.occupied();
    let foes = capturable(state, color);
    while pieces != 0 {
        let from = pop_square(&mut pieces);
        let (quiets, captures) = step_targets(steps, from, occupied, foes);
        let mut targets = if captures_only { captures } else { captures | quiets };
        while targets != 0 {
            list.push(Move::new(from, pop_square(&mut targets)));
        }
    }
}

/// `add_kind_moves` for a kind that is not simple. The usual case (no en passant capture is
/// possible) tests the probes of the square (see `Probe`); they give the same moves as
/// `add_group_moves`.
#[inline(never)]
fn add_group_kind_moves(
    state: &State,
    tables: &KindTables,
    color: Color,
    mut pieces: Bitboard,
    captures_only: bool,
    list: &mut MoveList,
) {
    let occupied = state.occupied();
    let foes = capturable(state, color);
    let ep = state.ep_squares();
    let en_passant = ep != 0 && tables.captures_en_passant && foes & bit(state.ep_victim()) != 0;
    while pieces != 0 {
        let from = pop_square(&mut pieces);
        if en_passant && tables.en_passant_reach[from as usize] & ep != 0 {
            add_group_moves(state, tables, color, from, occupied, foes, captures_only, list);
            continue;
        }
        let (start, end) = tables.probe_ranges[from as usize][captures_only as usize];
        let mut given = 0;
        // The condition of a probe with no moved piece (`Condition::Unmoved`) is read one time.
        let mut moved = None;
        for probe in &tables.probes[start as usize..end as usize] {
            let holds = match probe.condition {
                Condition::Always => true,
                Condition::Unmoved => !*moved.get_or_insert_with(|| has_moved(state, from)),
                condition => condition_holds(state, condition, color, from),
            };
            if occupied & probe.path != 0 || !holds {
                continue;
            }
            let quiets = probe.quiet_targets & !occupied;
            let captures = probe.capture_targets & foes;
            let mut targets = (quiets | captures) & !given;
            given |= targets;
            if captures_only && !probe.promo {
                targets &= captures;
            }
            if targets == 0 {
                continue;
            }
            if probe.plain {
                while targets != 0 {
                    list.push(Move::new(from, pop_square(&mut targets)));
                }
            } else if probe.path != 0 && !probe.promo {
                // A probe with a path has one target.
                let to = targets.trailing_zeros() as Square;
                let special = if quiets != 0 { probe.quiet_special } else { probe.capture_special };
                list.push(Move { from, to, promo: None, special });
            } else {
                while targets != 0 {
                    let to = pop_square(&mut targets);
                    let special = if quiets & bit(to) != 0 { probe.quiet_special } else { probe.capture_special };
                    if probe.promo {
                        push_promotions(list, from, to, &tables.promotions, special);
                    } else {
                        list.push(Move { from, to, promo: None, special });
                    }
                }
            }
        }
    }
}

/// Adds the castles of a king. The moves at and after `start` are the moves of this side.
/// A king whose own movement also goes to a castle square gets only the castle there if the
/// castle is legal, and also its own move if the castle is not legal.
///
/// The legality filter checks the `to` square of the king, as for each move.
fn add_castles(state: &State, side: &SideTables, color: Color, from: Square, start: usize, list: &mut MoveList) {
    if state.piece_at(from).is_none_or(|king| king.moved) {
        return;
    }
    let occupied = state.occupied();
    // Each square of `safe` is tested one time for all the castles of the king.
    let (mut safe, mut attacked) = (0, 0);
    let mut targets = 0;
    for castle in side.castles.iter().filter(|castle| castle.king_from == from) {
        let partner_ready = state
            .piece_at(castle.partner_from)
            .is_some_and(|piece| piece.kind == castle.partner && piece.color == color && !piece.moved);
        if !partner_ready || occupied & castle.required_empty() != 0 || castle.safe & attacked != 0 {
            continue;
        }
        let mut unknown = castle.safe & !safe;
        while unknown != 0 {
            let s = pop_square(&mut unknown);
            if is_attacked(state, s, color.other()) {
                attacked |= bit(s);
                break;
            }
            safe |= bit(s);
        }
        if castle.safe & attacked == 0 {
            targets |= bit(castle.king_to);
        }
    }
    if targets == 0 {
        return;
    }
    if side.king_move_to_castle_square {
        // The castle takes the place of the king move only if the castle is legal: the partner
        // can open a line to the `to` square that the king move keeps closed.
        let mut legal = 0;
        let mut squares = targets;
        while squares != 0 {
            let to = pop_square(&mut squares);
            let mut next = state.clone();
            next.make(Move { from, to, promo: None, special: Special::Castle });
            if !in_check(&next, color) {
                legal |= bit(to);
            }
        }
        retain_from(list, start, |m| m.from != from || legal & bit(m.to) == 0);
    }
    for castle in side.castles.iter().filter(|castle| castle.king_from == from && targets & bit(castle.king_to) != 0) {
        list.push(Move { from, to: castle.king_to, promo: None, special: Special::Castle });
    }
}

/// Adds the moves of the piece on `from` that obey piece movement but can leave the king in check.
fn add_piece_moves(state: &State, from: Square, captures_only: bool, list: &mut MoveList) {
    let Some(piece) = state.piece_at(from) else { return };
    let side = state.tables().side(piece.color);
    let start = list.len();
    add_kind_moves(state, side, piece.kind, piece.color, bit(from), captures_only, list);
    if piece.kind == Kind::King && !side.castles.is_empty() && !captures_only {
        add_castles(state, side, piece.color, from, start, list);
    }
}

/// Adds the moves of a side that obey piece movement but can leave the king in check.
///
/// With `captures_only`, the list has the captures and the promotions only.
pub fn pseudo_moves(state: &State, color: Color, captures_only: bool, list: &mut MoveList) {
    let side = state.tables().side(color);
    let own = state.color_set(color);
    let start = list.len();
    for kind in Kind::ALL {
        add_kind_moves(state, side, kind, color, state.kind_set(kind) & own, captures_only, list);
    }
    if !side.castles.is_empty() && !captures_only {
        let mut kings = state.kind_set(Kind::King) & own;
        while kings != 0 {
            add_castles(state, side, color, pop_square(&mut kings), start, list);
        }
    }
}

/// True if a pseudo move does not leave the king of the mover in check.
#[inline]
pub fn is_legal(state: &mut State, m: Move, mover: Color) -> bool {
    let undo = state.make(m);
    let legal = !in_check(state, mover);
    state.unmake(m, undo);
    legal
}

/// Adds the legal moves of the side that has the move. The state is the same after the call.
pub fn legal_moves(state: &mut State, list: &mut MoveList) {
    let color = state.turn();
    let start = list.len();
    pseudo_moves(state, color, false, list);
    retain_from(list, start, |m| is_legal(state, m, color));
}

/// The legal moves of the piece on a square, as if its side has the move.
/// A side that does not have the move cannot capture en passant.
pub fn moves_from(state: &State, from: Square, list: &mut MoveList) {
    let Some(piece) = state.piece_at(from) else { return };
    let mut state = state.clone();
    if piece.color != state.turn() {
        state = state.with_turn(piece.color);
    }
    let start = list.len();
    add_piece_moves(&state, from, false, list);
    retain_from(list, start, |m| is_legal(&mut state, m, piece.color));
}

/// Keeps the moves at and after `start` for which `keep` returns true.
fn retain_from(list: &mut MoveList, start: usize, mut keep: impl FnMut(Move) -> bool) {
    let mut index = 0;
    list.retain(|m| {
        index += 1;
        index <= start || keep(m)
    });
}
