//! The algorithm of the old TypeScript AI (`src/engine/ai.ts`), as a baseline opponent.
//!
//! It is a negamax search with a fixed depth and a quiescence search of 4 half moves. Its
//! evaluation has fixed piece values, thus it does not see the movement rules of the battle.
//! The game does not use this module. The self-play tool measures the new search against it.

use crate::movegen::{in_check, legal_moves, pseudo_moves};
use crate::rng::Rng;
use crate::search::SearchResult;
use crate::state::State;
use crate::types::{Color, Kind, Move, MoveList, Special};

/// The piece values of ordinary chess in centipawns: pawn, knight, bishop, rook, queen, king.
///
/// The evaluation of the game does not read these numbers. They are for this baseline, for
/// the `FixedValues` opponent of the self-play tool, and for the tests of the derived values.
pub const FIXED_VALUES: [i32; Kind::COUNT] = [100, 320, 330, 500, 900, 0];
const MATE: i32 = 100_000;
const BARE: i32 = 50_000;
const QUIESCENCE_DEPTH: u32 = 4;

/// The score in centipawns for the side that has the move.
pub fn evaluate(state: &State) -> i32 {
    let mut score = 0;
    for (s, piece) in state.board().iter().enumerate() {
        let Some(piece) = piece else { continue };
        let (file, rank) = ((s & 7) as i32, (s >> 3) as i32);
        let white = piece.color == Color::White;
        let mut value = FIXED_VALUES[piece.kind.index()];
        if piece.kind == Kind::Pawn {
            let advance = if white { rank } else { 7 - rank };
            value += advance * advance * 2;
        } else if piece.kind == Kind::Knight || piece.kind == Kind::Bishop {
            value += (14 - (2 * file - 7).abs() - (2 * rank - 7).abs()) * 2;
        }
        score += if white { value } else { -value };
    }
    if state.men(Color::White) == 0 {
        score -= BARE;
    }
    if state.men(Color::Black) == 0 {
        score += BARE;
    }
    if state.turn == Color::White { score } else { -score }
}

/// Puts captures of valuable pieces and promotions first. The order of equal moves stays.
fn order(state: &State, list: &mut MoveList) {
    let score = |m: &Move| {
        let capture = match (state.piece_at(m.to), state.piece_at(m.from)) {
            (Some(victim), Some(mover)) => FIXED_VALUES[victim.kind.index()] * 10 - FIXED_VALUES[mover.kind.index()],
            _ => 0,
        };
        capture + m.promo.map_or(0, |kind| FIXED_VALUES[kind.index()])
    };
    list.as_mut_slice().sort_by_key(|m| std::cmp::Reverse(score(m)));
}

fn quiesce(state: &mut State, mut alpha: i32, beta: i32, depth: u32, nodes: &mut u64) -> i32 {
    *nodes += 1;
    let stand = evaluate(state);
    if depth == 0 || stand >= beta {
        return stand;
    }
    alpha = alpha.max(stand);
    let color = state.turn;
    let mut list = MoveList::new();
    pseudo_moves(state, color, true, &mut list);
    order(state, &mut list);
    for &m in &list {
        let undo = state.make(m);
        if in_check(state, color) {
            state.unmake(m, undo);
            continue;
        }
        let score = -quiesce(state, -beta, -alpha, depth - 1, nodes);
        state.unmake(m, undo);
        if score > alpha {
            alpha = score;
            if alpha >= beta {
                break;
            }
        }
    }
    alpha
}

fn search(state: &mut State, depth: u32, mut alpha: i32, beta: i32, ply: i32, nodes: &mut u64) -> i32 {
    if depth == 0 {
        return quiesce(state, alpha, beta, QUIESCENCE_DEPTH, nodes);
    }
    *nodes += 1;
    let color = state.turn;
    let mut list = MoveList::new();
    pseudo_moves(state, color, false, &mut list);
    order(state, &mut list);
    let mut any = false;
    for &m in &list {
        let undo = state.make(m);
        if in_check(state, color) {
            state.unmake(m, undo);
            continue;
        }
        any = true;
        let score = -search(state, depth - 1, -beta, -alpha, ply + 1, nodes);
        state.unmake(m, undo);
        if score > alpha {
            alpha = score;
            if alpha >= beta {
                break;
            }
        }
    }
    if any { alpha } else { -MATE + ply }
}

/// Returns None when the side that has the move has no legal move.
pub fn choose_move(state: &mut State, depth: u32, noise_cp: i32, seed: u64) -> Option<SearchResult> {
    let mut rng = Rng::new(seed);
    let mut list = MoveList::new();
    legal_moves(state, &mut list);
    order(state, &mut list);
    let mut nodes = 0;
    let mut best: Option<(Move, i32)> = None;
    let mut best_sum = 0;
    for &m in &list {
        let undo = state.make(m);
        // A move that scores less than `best_sum - noise_cp` cannot become the best move.
        let floor = if best.is_some() { best_sum - noise_cp } else { -2 * MATE };
        let score = -search(state, depth.max(1) - 1, -2 * MATE, -floor, 1, &mut nodes);
        let sum = score + if noise_cp > 0 { rng.below(noise_cp as u64 + 1) as i32 } else { 0 };
        state.unmake(m, undo);
        if best.is_none() || sum > best_sum {
            best = Some((m, score));
            best_sum = sum;
        }
    }
    let (mv, score) = best?;
    debug_assert!(mv.special != Special::Castle || state.piece_at(mv.from).is_some());
    Some(SearchResult { mv, score, depth, nodes, pv: vec![mv] })
}
