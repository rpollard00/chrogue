//! Move generation and attack detection.
//!
//! The officers (knight, bishop, rook, queen, king) move by the tables that come from the
//! rules data. The pawn moves, en passant, and castling are code that reads `PawnRules`
//! and `SideRules::castling`.

use crate::state::State;
use crate::tables::SideTables;
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

/// True if a piece of side `by` attacks the square. An attack is a move that can capture there.
pub fn is_attacked(state: &State, s: Square, by: Color) -> bool {
    let side = state.tables().side(by);
    let theirs = state.color_set(by);
    let target = s as usize;
    if side.pawn_attackers[target] & state.kind_set(Kind::Pawn) & theirs != 0 {
        return true;
    }
    for &kind in &side.leap_attacker_kinds {
        if side.leap_attackers[kind.index()][target] & state.kind_set(kind) & theirs != 0 {
            return true;
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
            {
                return true;
            }
        }
    }
    false
}

/// True if the king of a side is attacked. A side with no king is never in check.
#[inline]
pub fn in_check(state: &State, color: Color) -> bool {
    match state.king_square(color) {
        Some(king) => is_attacked(state, king, color.other()),
        None => false,
    }
}

/// The squares where an officer on `from` can go.
#[inline(always)]
fn officer_targets(
    side: &SideTables,
    kind: Kind,
    from: Square,
    occupied: Bitboard,
    foes: Bitboard,
    captures_only: bool,
) -> Bitboard {
    let leaps = &side.leaps[kind.index()][from as usize];
    let mut captures = (leaps.both | leaps.capture) & foes;
    let mut quiets = (leaps.both | leaps.quiet) & !occupied;
    for slide in &side.slides[kind.index()] {
        let ray = slide.rays[from as usize];
        let (empty, hit) = match first_blocker(ray & occupied, slide.ascending) {
            Some(blocker) => (ray & !slide.rays[blocker as usize] & !bit(blocker), bit(blocker)),
            None => (ray, 0),
        };
        if slide.quiet {
            quiets |= empty;
        }
        if slide.capture {
            captures |= hit & foes;
        }
    }
    if captures_only { captures } else { captures | quiets }
}

#[inline(always)]
fn push_pawn_move(side: &SideTables, list: &mut MoveList, from: Square, to: Square, promo: bool, special: Special) {
    if promo {
        for kind in side.promotions {
            list.push(Move { from, to, promo: Some(kind), special });
        }
    } else {
        list.push(Move { from, to, promo: None, special });
    }
}

fn add_pawn_moves(
    state: &State,
    side: &SideTables,
    color: Color,
    from: Square,
    captures_only: bool,
    list: &mut MoveList,
) {
    let occupied = state.occupied();
    let foes = state.color_set(color.other());
    let is_promo = |s: Square| side.promo_zone & bit(s) != 0;
    let step = 8 * color.forward();
    let on_board = |s: i8| (0..64).contains(&s);

    let one = from as i8 + step;
    if on_board(one) {
        let one = one as Square;
        if occupied & bit(one) == 0 {
            if !captures_only || is_promo(one) {
                push_pawn_move(side, list, from, one, is_promo(one), Special::None);
            }
            let two = one as i8 + step;
            let moved = state.piece_at(from).is_some_and(|pawn| pawn.moved);
            if (!moved || side.double_step_always) && on_board(two) && occupied & bit(two as Square) == 0 {
                let two = two as Square;
                // A double step into the promotion zone promotes and makes no en passant square.
                let promo = is_promo(two);
                if !captures_only || promo {
                    let special = if promo { Special::None } else { Special::DoubleStep };
                    push_pawn_move(side, list, from, two, promo, special);
                }
            }
        }
        let attacks = side.pawn_captures[from as usize];
        let mut captures = attacks & foes;
        while captures != 0 {
            let to = pop_square(&mut captures);
            push_pawn_move(side, list, from, to, is_promo(to), Special::None);
        }
        if let Some(ep) = state.ep
            && attacks & bit(ep) & !occupied != 0
        {
            let victim = (ep as i8 - step) as Square;
            if state.pieces(color.other(), Kind::Pawn) & bit(victim) != 0 {
                push_pawn_move(side, list, from, ep, is_promo(ep), Special::EnPassant);
            }
        }
    }
    let back = from as i8 - step;
    if side.backward_step && !captures_only && on_board(back) && occupied & bit(back as Square) == 0 {
        list.push(Move { from, to: back as Square, promo: None, special: Special::Backward });
    }
}

/// Adds the castles of a king. The checks are the same as in `addCastles` of the TypeScript engine:
/// the king square and the square that the king crosses must not be attacked. The legality filter
/// rejects a castle that puts the king on an attacked square.
fn add_castles(state: &State, color: Color, from: Square, list: &mut MoveList) {
    let foe = color.other();
    let home: Square = if color == Color::White { 4 } else { 60 };
    let unmoved = state.piece_at(from).is_some_and(|king| !king.moved);
    if !unmoved || from != home || is_attacked(state, from, foe) {
        return;
    }
    let occupied = state.occupied();
    let rook_ready =
        |s: Square| state.piece_at(s).is_some_and(|rook| rook.kind == Kind::Rook && rook.color == color && !rook.moved);
    if rook_ready(from + 3) && occupied & (bit(from + 1) | bit(from + 2)) == 0 && !is_attacked(state, from + 1, foe) {
        list.push(Move { from, to: from + 2, promo: None, special: Special::Castle });
    }
    if rook_ready(from - 4)
        && occupied & (bit(from - 1) | bit(from - 2) | bit(from - 3)) == 0
        && !is_attacked(state, from - 1, foe)
    {
        list.push(Move { from, to: from - 2, promo: None, special: Special::Castle });
    }
}

/// Adds the moves of the piece on `from` that obey piece movement but can leave the king in check.
fn add_piece_moves(state: &State, from: Square, captures_only: bool, list: &mut MoveList) {
    let Some(piece) = state.piece_at(from) else { return };
    let side = state.tables().side(piece.color);
    if piece.kind == Kind::Pawn {
        add_pawn_moves(state, side, piece.color, from, captures_only, list);
        return;
    }
    let foes = state.color_set(piece.color.other());
    let mut targets = officer_targets(side, piece.kind, from, state.occupied(), foes, captures_only);
    while targets != 0 {
        list.push(Move::new(from, pop_square(&mut targets)));
    }
    if piece.kind == Kind::King && side.castling && !captures_only {
        add_castles(state, piece.color, from, list);
    }
}

/// Adds the moves of a side that obey piece movement but can leave the king in check.
///
/// With `captures_only`, the list has the captures and the promotions only.
pub fn pseudo_moves(state: &State, color: Color, captures_only: bool, list: &mut MoveList) {
    let side = state.tables().side(color);
    let own = state.color_set(color);
    let foes = state.color_set(color.other());
    let occupied = own | foes;

    let mut pawns = state.kind_set(Kind::Pawn) & own;
    while pawns != 0 {
        add_pawn_moves(state, side, color, pop_square(&mut pawns), captures_only, list);
    }
    for kind in Kind::OFFICERS {
        let mut pieces = state.kind_set(kind) & own;
        while pieces != 0 {
            let from = pop_square(&mut pieces);
            let mut targets = officer_targets(side, kind, from, occupied, foes, captures_only);
            while targets != 0 {
                list.push(Move::new(from, pop_square(&mut targets)));
            }
        }
    }
    if side.castling && !captures_only {
        let mut kings = state.kind_set(Kind::King) & own;
        while kings != 0 {
            add_castles(state, color, pop_square(&mut kings), list);
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
    let color = state.turn;
    let start = list.len();
    pseudo_moves(state, color, false, list);
    retain_from(list, start, |m| is_legal(state, m, color));
}

/// The legal moves of the piece on a square, as if its side has the move.
/// A side that does not have the move cannot capture en passant.
pub fn moves_from(state: &State, from: Square, list: &mut MoveList) {
    let Some(piece) = state.piece_at(from) else { return };
    let mut state = state.clone();
    if piece.color != state.turn {
        state.turn = piece.color;
        state.ep = None;
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
