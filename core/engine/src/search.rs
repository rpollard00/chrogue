//! The search: iterative deepening, negamax with alpha-beta and principal variation search,
//! a transposition table, move ordering, and a quiescence search.
//!
//! The scores of the ends of a battle are those of `outcome`:
//!
//! - A side with no legal move loses, with or without a check.
//! - A side with only its king loses (rout). If each side has only its king, the battle is a draw.
//! - The battle is a draw when the clock gets to `CLOCK_LIMIT`.
//!
//! The game has no rule for a repeated position, thus the search has none.
//!
//! A search with a node limit and no time limit is deterministic.

use std::time::{Duration, Instant};

use crate::eval::{CLOCK_FADE_START, EvalVariant, Evaluator};
use crate::movegen::{in_check, pseudo_moves};
use crate::outcome::{CLOCK_LIMIT, Outcome, has_legal_move, material_outcome};
use crate::rng::mix;
use crate::state::State;
use crate::types::{Color, Kind, Move, MoveList, Special};
use crate::zobrist;

/// The score of a win at the root. A win after `n` half moves has the score `MATE - n`.
pub const MATE: i32 = 30_000;
/// A score above this number is a forced win.
pub const MATE_BOUND: i32 = MATE - 1_000;
const INFINITE: i32 = 32_000;
const MAX_PLY: usize = 96;

/// The limits of a search. The search stops at the first limit that it gets to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// The largest depth of the iterative deepening, in half moves.
    pub max_depth: u32,
    /// The largest number of nodes.
    pub max_nodes: u64,
    /// The largest time. A search with a time limit is not deterministic.
    pub max_time: Option<Duration>,
}

impl Limits {
    pub const fn nodes(max_nodes: u64) -> Limits {
        Limits { max_depth: MAX_PLY as u32 / 2, max_nodes, max_time: None }
    }

    pub const fn depth(max_depth: u32) -> Limits {
        Limits { max_depth, max_nodes: u64::MAX, max_time: None }
    }
}

/// The parts of the search that can be off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SearchOptions {
    /// Null-move pruning. The search does not use it when the side to move has few pieces,
    /// because there a side with no good move (or no move) is a real result.
    pub null_move: bool,
    /// Late-move reductions.
    pub lmr: bool,
    /// The threat term of the evaluation. It is always on in the game. The self-play tool
    /// can set it off to measure it.
    pub threats: bool,
}

impl SearchOptions {
    pub const NONE: SearchOptions = SearchOptions { null_move: false, lmr: false, threats: true };
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SearchResult {
    pub mv: Move,
    /// The score of the move in centipawns for the side that has the move, without the noise.
    pub score: i32,
    /// The depth of the last iteration that gave the move.
    pub depth: u32,
    pub nodes: u64,
    /// The line that the search expects. The first move is `mv`.
    pub pv: Vec<Move>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Empty,
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    mv: Move,
    score: i32,
    depth: i16,
    bound: Bound,
}

const EMPTY_ENTRY: Entry = Entry { key: 0, mv: Move::NULL, score: 0, depth: 0, bound: Bound::Empty };

struct Searcher {
    eval: Evaluator,
    options: SearchOptions,
    max_nodes: u64,
    deadline: Option<Instant>,
    table: Vec<Entry>,
    mask: usize,
    killers: [[Move; 2]; MAX_PLY],
    /// `history[color][from][to]`
    history: Box<[[[i32; 64]; 64]; 2]>,
    nodes: u64,
    stopped: bool,
}

/// The key of a state for the transposition table. Near the limit of the clock, the result
/// of a position depends on the clock, thus the clock is a part of the key there.
#[inline(always)]
fn table_key(state: &State) -> u64 {
    if state.clock > CLOCK_FADE_START { state.key() ^ zobrist::clock_key(state.clock) } else { state.key() }
}

/// True if the move captures the royal king of the other side.
///
/// The legal moves of a search never permit this, because a side cannot leave its king in
/// check. It is possible only when the search starts from a state where the side that does
/// not have the move is in check. The capture is a win for the capturer with the score of a
/// mate. Without this rule the search would continue against a side that has no king and is
/// thus never in check.
#[inline(always)]
fn captures_royal(state: &State, m: Move) -> bool {
    state.king_square(state.turn().other()) == Some(m.to)
}

/// The order scores for a list of `len` moves. A list with more than `MoveList::CAPACITY`
/// moves gets its scores on the heap.
#[inline(always)]
fn score_slots<'a>(inline: &'a mut [i32; MoveList::CAPACITY], heap: &'a mut Vec<i32>, len: usize) -> &'a mut [i32] {
    if len <= MoveList::CAPACITY {
        &mut inline[..len]
    } else {
        heap.resize(len, 0);
        heap
    }
}

#[inline(always)]
fn is_quiet(state: &State, m: Move) -> bool {
    m.promo.is_none() && m.special != Special::EnPassant && state.piece_at(m.to).is_none()
}

impl Searcher {
    fn new(state: &State, limits: &Limits, variant: EvalVariant, options: SearchOptions) -> Searcher {
        // Two entries for each node of the budget, from 1024 to about one million entries.
        let size = limits.max_nodes.saturating_mul(2).clamp(1 << 10, 1 << 20).next_power_of_two() as usize;
        let mut eval = Evaluator::new(state, variant);
        eval.threats = options.threats;
        Searcher {
            eval,
            options,
            max_nodes: limits.max_nodes,
            deadline: limits.max_time.map(|time| Instant::now() + time),
            table: vec![EMPTY_ENTRY; size],
            mask: size - 1,
            killers: [[Move::NULL; 2]; MAX_PLY],
            history: Box::new([[[0; 64]; 64]; 2]),
            nodes: 0,
            stopped: false,
        }
    }

    /// Counts a node. Returns false if a limit stops the search.
    #[inline(always)]
    fn enter(&mut self) -> bool {
        if self.nodes >= self.max_nodes {
            self.stopped = true;
            return false;
        }
        if let Some(deadline) = self.deadline
            && self.nodes & 1023 == 0
            && Instant::now() >= deadline
        {
            self.stopped = true;
            return false;
        }
        self.nodes += 1;
        true
    }

    /// The score if a rout or bare kings end the battle, for the side that has the move.
    #[inline(always)]
    fn rout_score(state: &State, ply: usize) -> Option<i32> {
        Some(match material_outcome(state)? {
            Outcome::Rout { winner } if winner == state.turn() => MATE - ply as i32,
            Outcome::Rout { .. } => -MATE + ply as i32,
            _ => 0,
        })
    }

    /// Gives each move a number for the order of the search: the move of the table, then the
    /// captures and promotions (most valuable victim, least valuable attacker), then the
    /// killer moves, then the other moves by their history.
    fn score_moves(&self, state: &State, list: &MoveList, scores: &mut [i32], table_move: Move, ply: usize) {
        let us = state.turn();
        let them = us.other();
        for (i, &m) in list.iter().enumerate() {
            scores[i] = if m == table_move {
                10_000_000
            } else if !is_quiet(state, m) {
                let victim = match state.piece_at(m.to) {
                    Some(piece) => self.eval.value(them, piece.kind),
                    None if m.special == Special::EnPassant => self.eval.value(them, Kind::Pawn),
                    None => 0,
                };
                let attacker = state.piece_at(m.from).map_or(0, |piece| self.eval.value(us, piece.kind));
                let promo = m.promo.map_or(0, |kind| self.eval.value(us, kind));
                1_000_000 + (victim + promo) * 16 - attacker
            } else if ply < MAX_PLY && m == self.killers[ply][0] {
                900_000
            } else if ply < MAX_PLY && m == self.killers[ply][1] {
                899_999
            } else {
                self.history[us.index()][m.from as usize][m.to as usize]
            };
        }
    }

    /// Moves the best move of `list[from..]` to the index `from` and returns it.
    #[inline(always)]
    fn pick(list: &mut MoveList, scores: &mut [i32], from: usize) -> Move {
        let moves = list.as_mut_slice();
        let mut best = from;
        for i in from + 1..moves.len() {
            if scores[i] > scores[best] {
                best = i;
            }
        }
        moves.swap(from, best);
        scores.swap(from, best);
        moves[from]
    }

    fn quiesce(&mut self, state: &mut State, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if !self.enter() {
            return 0;
        }
        if let Some(score) = Self::rout_score(state, ply) {
            return score;
        }
        let stand = self.eval.evaluate(state);
        if stand >= beta || ply >= MAX_PLY {
            return stand;
        }
        alpha = alpha.max(stand);
        let us = state.turn();
        let mut list = MoveList::new();
        pseudo_moves(state, us, true, &mut list);
        let (mut inline, mut heap) = ([0i32; MoveList::CAPACITY], Vec::new());
        let scores = score_slots(&mut inline, &mut heap, list.len());
        self.score_moves(state, &list, scores, Move::NULL, MAX_PLY);
        let mut best = stand;
        for i in 0..list.len() {
            let m = Self::pick(&mut list, scores, i);
            if captures_royal(state, m) {
                return MATE - ply as i32 - 1;
            }
            let undo = state.make(m);
            if in_check(state, us) {
                state.unmake(m, undo);
                continue;
            }
            let score = -self.quiesce(state, -beta, -alpha, ply + 1);
            state.unmake(m, undo);
            if self.stopped {
                return 0;
            }
            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    if alpha >= beta {
                        break;
                    }
                }
            }
        }
        best
    }

    fn negamax(&mut self, state: &mut State, mut depth: i32, mut alpha: i32, beta: i32, ply: usize, null: bool) -> i32 {
        let us = state.turn();
        let checked = in_check(state, us);
        // A check makes the search one half move longer, thus the quiescence search never
        // starts with a king in check.
        if checked {
            depth += 1;
        }
        if depth <= 0 {
            return self.quiesce(state, alpha, beta, ply);
        }
        if !self.enter() {
            return 0;
        }
        if let Some(score) = Self::rout_score(state, ply) {
            return score;
        }
        if state.clock >= CLOCK_LIMIT {
            return if has_legal_move(state) { 0 } else { -MATE + ply as i32 };
        }
        if ply >= MAX_PLY - 1 {
            return self.eval.evaluate(state);
        }

        let pv_node = beta - alpha > 1;
        let key = table_key(state);
        let slot = key as usize & self.mask;
        let entry = self.table[slot];
        let mut table_move = Move::NULL;
        if entry.bound != Bound::Empty && entry.key == key {
            table_move = entry.mv;
            if !pv_node && entry.depth as i32 >= depth {
                // A win score in the table counts the half moves from the position of the entry.
                let score = if entry.score >= MATE_BOUND {
                    entry.score - ply as i32
                } else if entry.score <= -MATE_BOUND {
                    entry.score + ply as i32
                } else {
                    entry.score
                };
                match entry.bound {
                    Bound::Exact => return score,
                    Bound::Lower if score >= beta => return score,
                    Bound::Upper if score <= alpha => return score,
                    _ => {}
                }
            }
        }

        if self.options.null_move && null && !pv_node && !checked && depth >= 3 && beta.abs() < MATE_BOUND {
            // With few pieces, a side with no good move is a real result: the null move is not safe.
            let officers = state.men(us) & !state.kind_set(Kind::Pawn);
            if officers.count_ones() >= 2 && state.men(us).count_ones() >= 4 && self.eval.evaluate(state) >= beta {
                let null_undo = state.make_null();
                let score = -self.negamax(state, depth - 3 - depth / 4, -beta, -beta + 1, ply + 1, false);
                state.unmake_null(null_undo);
                if self.stopped {
                    return 0;
                }
                if score >= beta && score < MATE_BOUND {
                    return score;
                }
            }
        }

        let mut list = MoveList::new();
        pseudo_moves(state, us, false, &mut list);
        let (mut inline, mut heap) = ([0i32; MoveList::CAPACITY], Vec::new());
        let scores = score_slots(&mut inline, &mut heap, list.len());
        self.score_moves(state, &list, scores, table_move, ply);

        let first_alpha = alpha;
        let mut best = -INFINITE;
        let mut best_move = Move::NULL;
        let mut legal = 0;
        for i in 0..list.len() {
            let m = Self::pick(&mut list, scores, i);
            if captures_royal(state, m) {
                return MATE - ply as i32 - 1;
            }
            let quiet = is_quiet(state, m);
            let undo = state.make(m);
            if in_check(state, us) {
                state.unmake(m, undo);
                continue;
            }
            legal += 1;
            let score = if legal == 1 {
                -self.negamax(state, depth - 1, -beta, -alpha, ply + 1, true)
            } else {
                let mut reduction = 0;
                if self.options.lmr && depth >= 3 && legal > 3 && quiet && !checked && scores[i] < 899_999 {
                    reduction = if legal > 8 && depth >= 6 { 2 } else { 1 };
                    if in_check(state, us.other()) {
                        reduction = 0;
                    }
                }
                let mut score = -self.negamax(state, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, true);
                if score > alpha && reduction > 0 && !self.stopped {
                    score = -self.negamax(state, depth - 1, -alpha - 1, -alpha, ply + 1, true);
                }
                if score > alpha && score < beta && !self.stopped {
                    score = -self.negamax(state, depth - 1, -beta, -alpha, ply + 1, true);
                }
                score
            };
            state.unmake(m, undo);
            if self.stopped {
                return 0;
            }
            if score > best {
                best = score;
                best_move = m;
                if score > alpha {
                    alpha = score;
                    if alpha >= beta {
                        if quiet {
                            self.reward_quiet(us, m, depth, ply);
                        }
                        break;
                    }
                }
            }
        }
        // A side with no legal move loses, with or without a check.
        if legal == 0 {
            return -MATE + ply as i32;
        }

        let bound = if best >= beta {
            Bound::Lower
        } else if best > first_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        let stored = if best >= MATE_BOUND {
            best + ply as i32
        } else if best <= -MATE_BOUND {
            best - ply as i32
        } else {
            best
        };
        let old = &mut self.table[slot];
        if old.bound == Bound::Empty || old.key != key || depth >= old.depth as i32 {
            *old = Entry { key, mv: best_move, score: stored, depth: depth as i16, bound };
        }
        best
    }

    /// Records a quiet move that was too good for the opponent, for the move order.
    fn reward_quiet(&mut self, us: Color, m: Move, depth: i32, ply: usize) {
        if self.killers[ply][0] != m {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = m;
        }
        let history = &mut self.history[us.index()][m.from as usize][m.to as usize];
        *history += depth * depth;
        if *history > 800_000 {
            for entry in self.history.iter_mut().flatten().flatten() {
                *entry /= 2;
            }
        }
    }

    /// The line of the table after the root move, for the result.
    fn principal_variation(&self, state: &mut State, first: Move, depth: u32) -> Vec<Move> {
        let mut pv = vec![first];
        let mut undos = vec![state.make(first)];
        while pv.len() < depth as usize {
            let key = table_key(state);
            let entry = self.table[key as usize & self.mask];
            if entry.bound == Bound::Empty || entry.key != key || Self::rout_score(state, 0).is_some() {
                break;
            }
            let us = state.turn();
            let mut list = MoveList::new();
            pseudo_moves(state, us, false, &mut list);
            if !list.iter().any(|&m| m == entry.mv) {
                break;
            }
            let undo = state.make(entry.mv);
            if in_check(state, us) {
                state.unmake(entry.mv, undo);
                break;
            }
            pv.push(entry.mv);
            undos.push(undo);
        }
        for (&m, &undo) in pv.iter().zip(&undos).rev() {
            state.unmake(m, undo);
        }
        pv
    }
}

struct RootMove {
    mv: Move,
    /// The random bonus of the move in centipawns. It is 0 when the level has no noise.
    bonus: i32,
}

/// Searches for the best move of the side that has the move. Returns None if the side has no
/// legal move. The state is the same after the call.
///
/// `noise_cp` makes the search weaker: each root move gets a random bonus from 0 to `noise_cp`
/// centipawns, and the search selects the move with the best sum of score and bonus. The
/// bonus of a move comes from `seed` and from the move only.
pub fn search(
    state: &mut State,
    limits: &Limits,
    variant: EvalVariant,
    options: SearchOptions,
    noise_cp: i32,
    seed: u64,
) -> Option<SearchResult> {
    let us = state.turn();
    let mut searcher = Searcher::new(state, limits, variant, options);

    let mut list = MoveList::new();
    pseudo_moves(state, us, false, &mut list);
    let (mut inline, mut heap) = ([0i32; MoveList::CAPACITY], Vec::new());
    let scores = score_slots(&mut inline, &mut heap, list.len());
    searcher.score_moves(state, &list, scores, Move::NULL, 0);
    let mut roots: Vec<RootMove> = Vec::new();
    for i in 0..list.len() {
        let m = Searcher::pick(&mut list, scores, i);
        if captures_royal(state, m) {
            return Some(SearchResult { mv: m, score: MATE - 1, depth: 1, nodes: 0, pv: vec![m] });
        }
        let undo = state.make(m);
        let legal = !in_check(state, us);
        state.unmake(m, undo);
        if legal {
            let code = (m.from as u64) << 16
                | (m.to as u64) << 8
                | (m.special as u64) << 4
                | m.promo.map_or(7, |kind| kind as u64);
            let bonus = if noise_cp > 0 { (mix(seed, code) % (noise_cp as u64 + 1)) as i32 } else { 0 };
            roots.push(RootMove { mv: m, bonus });
        }
    }
    let first = roots.first()?.mv;
    let mut result = SearchResult { mv: first, score: 0, depth: 0, nodes: 0, pv: vec![first] };

    for depth in 1..=limits.max_depth.max(1) as i32 {
        // The best sum of score and bonus in this iteration.
        let mut best_sum = -INFINITE;
        let mut best_index = 0;
        let mut best_score = 0;
        for (index, root) in roots.iter().enumerate() {
            let undo = state.make(root.mv);
            // The move is better than the best move if its score is more than this number.
            let floor = if index == 0 { -INFINITE } else { best_sum - root.bonus };
            let mut score = -INFINITE;
            if index > 0 {
                score = -searcher.negamax(state, depth - 1, -floor - 1, -floor, 1, true);
            }
            if (index == 0 || score > floor) && !searcher.stopped {
                score = -searcher.negamax(state, depth - 1, -INFINITE, -floor, 1, true);
            }
            state.unmake(root.mv, undo);
            if searcher.stopped {
                break;
            }
            if score > floor {
                best_sum = score + root.bonus;
                best_index = index;
                best_score = score;
            }
        }
        // An iteration that stopped early still gives a result if it finished its first move,
        // because the first move is the best move of the iteration before.
        if best_sum > -INFINITE {
            result.mv = roots[best_index].mv;
            result.score = best_score;
            result.depth = depth as u32;
            let best = roots.remove(best_index);
            roots.insert(0, best);
        }
        let forced_win = result.score >= MATE_BOUND && MATE - result.score <= depth;
        if searcher.stopped || roots.len() == 1 || forced_win {
            break;
        }
    }
    result.nodes = searcher.nodes;
    result.pv = searcher.principal_variation(state, result.mv, result.depth.max(1));
    Some(result)
}
