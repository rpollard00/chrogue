//! The search: iterative deepening, negamax with alpha-beta and principal variation search,
//! a transposition table, move ordering, and a quiescence search.
//!
//! The scores of the ends of a battle are those of `outcome`:
//!
//! - A side with no legal move loses, with or without a check.
//! - A side with only its king loses (rout). If each side has only its king, the battle is a draw.
//! - The battle is a draw when the clock gets to `CLOCK_LIMIT`.
//!
//! Each node of the search, the quiescence search too, tests these ends before it looks at
//! the depth. The quiescence search does not stand pat while the side to move is in check, and
//! it tests for a side with no legal move when the side has few men.
//!
//! A move out of check can give check. Thus a line has a limit of the half moves that a check
//! adds (`CHECK_EXTENSION_PLIES`), and a line of the quiescence search has a limit of quiet
//! moves out of check (`QUIET_EVASION_LINE`).
//!
//! The game has no rule for a repeated position, thus the search has none.
//!
//! A search with a node limit and no time limit is deterministic. The search always completes
//! depth 1 for each root move; the node limit applies after that.

use std::time::{Duration, Instant};

use crate::eval::{CLOCK_FADE_START, EvalVariant, Evaluator};
use crate::movegen::{evasion_moves, evasion_squares, in_check, may_give_check, pseudo_moves};
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
    /// The largest number of nodes after depth 1. The search always completes depth 1 for each
    /// root move, thus a search can use more nodes when depth 1 needs more.
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

/// The flaws of a search: the ways that it plays worse on purpose. Each random number comes
/// from the seed of the search, thus the same seed gives the same flaws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flaws {
    /// Each root move gets a random bonus from 0 to this number of centipawns. The search
    /// selects the move with the best sum of score and bonus. The bonus of a move comes from
    /// the seed and from the move only.
    pub noise_cp: i32,
    /// The chance in percent, from 0 to 100, that the search does not see a root move. Each
    /// legal move of the root has this chance, and the search selects from the moves that it
    /// sees. If it sees no move, it sees each move. The search always sees a legal capture of
    /// the royal king.
    pub overlook: u32,
    /// The chance in percent, from 0 to 100, that the search is careless. A search is careless
    /// for all its root moves or for none. A careless search gives each root move the score of
    /// the position right after the move. That score is the score of an end of the battle (a
    /// rout, bare kings, or the clock), or the evaluation. A careless search does not look at
    /// the reply of the opponent. It does one iteration, and the depth limit and the node
    /// limit do not apply. The noise and the overlook apply.
    pub careless: u32,
}

impl Flaws {
    /// No flaw.
    pub const NONE: Flaws = Flaws { noise_cp: 0, overlook: 0, careless: 0 };

    /// Noise and no other flaw.
    pub const fn noise(noise_cp: i32) -> Flaws {
        Flaws { noise_cp, ..Flaws::NONE }
    }
}

/// The salts for the random numbers of `Flaws::overlook` and `Flaws::careless`. The code of a
/// root move is less than `1 << 24`, thus a salt is never the code of a move.
const OVERLOOK_SALT: u64 = 1 << 32;
const CARELESS_SALT: u64 = 2 << 32;

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
    /// The formation term of the evaluation. It is always on in the game. The self-play tool
    /// can set it off to measure it.
    pub formation: bool,
}

impl SearchOptions {
    /// All the parts that can be off are off.
    pub const NONE: SearchOptions = SearchOptions { null_move: false, lmr: false, threats: false, formation: false };
}

/// The quiescence search tests for a side with no legal move (a loss) when the side to move
/// has this number of men or fewer. With more men, a side with no move is very rare, and the
/// test would cost time at each node.
const FEW_MEN: u32 = 3;

/// In check, the quiescence search searches each capture and promotion, and this number of
/// legal quiet moves. It searches more quiet moves only while each move so far is a loss, thus
/// it still finds a mate. All the quiet moves after each check of a line of captures would make
/// the tree too large.
const QUIET_EVASIONS: u32 = 2;

/// The most quiet moves out of check in one line of the quiescence search. A quiet move out of
/// check can give check, thus without this limit a line of such moves has no end. After the limit,
/// a side in check is searched as a side that is not in check.
const QUIET_EVASION_LINE: u32 = 6;

/// A check makes the search one half move longer only in the first `2 * depth + this number`
/// half moves of a line, where `depth` is the depth of the iteration. A move out of check can
/// give check, thus without this limit a line of checks has no end.
const CHECK_EXTENSION_PLIES: usize = 8;

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
    /// A check makes the search longer only at a ply less than this number.
    extension_end: usize,
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

/// True if the move captures the royal king of the other side: the king on the lowest square,
/// which is the king that `in_check` tests.
///
/// This is possible only at the root, when the search starts from a state where the side that
/// does not have the move is in check. A legal capture of the king is a win for the capturer
/// with the score of a mate. Inside the search, the move before each node is legal, thus the
/// side that does not have the move is not in check, and no move attacks its royal king. A king
/// cannot be the victim of an en passant capture (`RulesError::KingMakesEnPassant`), thus each
/// capture of the king goes to its square.
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
        if !options.formation {
            eval.forget_formation();
        }
        Searcher {
            eval,
            options,
            max_nodes: limits.max_nodes,
            extension_end: 0,
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

    /// The score if the battle has ended, for the side that has the move: a rout, bare kings,
    /// or the clock. The order is that of `outcome`: at the limit of the clock, a side with no
    /// legal move loses, and the other positions are a draw. The search finds "no legal move"
    /// before the limit in its own move loops.
    #[inline(always)]
    fn end_score(state: &mut State, ply: usize) -> Option<i32> {
        if let Some(score) = Self::rout_score(state, ply) {
            return Some(score);
        }
        if state.clock >= CLOCK_LIMIT {
            return Some(if has_legal_move(state) { 0 } else { -MATE + ply as i32 });
        }
        None
    }

    /// The score of a root move at a glance, for the side that made it: a node with its count
    /// and the ends of the battle, and then the evaluation. The state is the position right
    /// after the move.
    fn glance(&mut self, state: &mut State) -> i32 {
        self.nodes += 1;
        -Self::end_score(state, 1).unwrap_or_else(|| self.eval.evaluate(state))
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
                    None if m.special == Special::EnPassant => {
                        state.piece_at(state.ep_victim()).map_or(0, |piece| self.eval.value(them, piece.kind))
                    }
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

    /// The quiescence search: a node with its count and the ends of the battle. `may_check` is
    /// false if the move before cannot have given check (`movegen::may_give_check`). `evasions`
    /// is the number of quiet moves out of check that the line can still have.
    fn quiesce(&mut self, state: &mut State, alpha: i32, beta: i32, ply: usize, may_check: bool, evasions: u32) -> i32 {
        if !self.enter() {
            return 0;
        }
        if let Some(score) = Self::end_score(state, ply) {
            return score;
        }
        let checked = evasions > 0 && may_check && in_check(state, state.turn());
        self.quiesce_moves(state, alpha, beta, ply, checked, evasions)
    }

    /// The moves of the quiescence search, after the node is counted and is not an end.
    ///
    /// A side that is not in check can stand pat, and searches its captures and promotions.
    /// A side in check cannot stand pat: it searches its moves (`QUIET_EVASIONS`), and with no
    /// legal move it loses. With `evasions` at 0, the caller gives `checked` as false for a side
    /// in check. A side with few men that is not in check also loses if it has no
    /// legal move.
    fn quiesce_moves(
        &mut self,
        state: &mut State,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        checked: bool,
        evasions: u32,
    ) -> i32 {
        if ply >= MAX_PLY {
            return self.eval.evaluate(state);
        }
        let us = state.turn();
        let mut best = -INFINITE;
        if !checked {
            if state.men(us).count_ones() <= FEW_MEN && !has_legal_move(state) {
                return -MATE + ply as i32;
            }
            let stand = self.eval.evaluate(state);
            if stand >= beta {
                return stand;
            }
            alpha = alpha.max(stand);
            best = stand;
        }
        let mut list = MoveList::new();
        if checked {
            // Most moves leave the king in check. Only these moves can be legal.
            evasion_moves(state, us, evasion_squares(state, us), &mut list);
        } else {
            pseudo_moves(state, us, true, &mut list);
        }
        let (mut inline, mut heap) = ([0i32; MoveList::CAPACITY], Vec::new());
        let scores = score_slots(&mut inline, &mut heap, list.len());
        self.score_moves(state, &list, scores, Move::NULL, MAX_PLY);
        let mut quiet_evasions = 0;
        for i in 0..list.len() {
            let m = Self::pick(&mut list, scores, i);
            // The captures come first. A side that has a move that is not a loss searches only
            // some quiet moves; thus a side with no legal move is still found.
            let quiet = checked && is_quiet(state, m);
            if quiet && quiet_evasions >= QUIET_EVASIONS && best > -MATE_BOUND {
                continue;
            }
            let undo = state.make(m);
            if in_check(state, us) {
                state.unmake(m, undo);
                continue;
            }
            quiet_evasions += quiet as u32;
            let may_check = may_give_check(state, m, &undo);
            let score = -self.quiesce(state, -beta, -alpha, ply + 1, may_check, evasions - quiet as u32);
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
        // In check with no legal move: the side loses.
        if best == -INFINITE { -MATE + ply as i32 } else { best }
    }

    /// `null`: a null move is permitted. `may_check`: false if the move before cannot have given
    /// check (`movegen::may_give_check`).
    #[allow(clippy::too_many_arguments)]
    fn negamax(
        &mut self,
        state: &mut State,
        mut depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        null: bool,
        may_check: bool,
    ) -> i32 {
        if !self.enter() {
            return 0;
        }
        // The ends of the battle come before the depth: a node at the horizon can be an end.
        if let Some(score) = Self::end_score(state, ply) {
            return score;
        }
        let us = state.turn();
        let checked = may_check && in_check(state, us);
        // A check makes the search one half move longer, thus the quiescence search starts
        // with a king in check only after the limit of these extensions.
        if checked && ply < self.extension_end {
            depth += 1;
        }
        if depth <= 0 {
            return self.quiesce_moves(state, alpha, beta, ply, checked, QUIET_EVASION_LINE);
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
                let score = -self.negamax(state, depth - 3 - depth / 4, -beta, -beta + 1, ply + 1, false, true);
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
            let quiet = is_quiet(state, m);
            let undo = state.make(m);
            if in_check(state, us) {
                state.unmake(m, undo);
                continue;
            }
            legal += 1;
            let may_check = may_give_check(state, m, &undo);
            let score = if legal == 1 {
                -self.negamax(state, depth - 1, -beta, -alpha, ply + 1, true, may_check)
            } else {
                let mut reduction = 0;
                if self.options.lmr && depth >= 3 && legal > 3 && quiet && !checked && scores[i] < 899_999 {
                    reduction = if legal > 8 && depth >= 6 { 2 } else { 1 };
                    if may_check && in_check(state, us.other()) {
                        reduction = 0;
                    }
                }
                let mut score =
                    -self.negamax(state, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, true, may_check);
                if score > alpha && reduction > 0 && !self.stopped {
                    score = -self.negamax(state, depth - 1, -alpha - 1, -alpha, ply + 1, true, may_check);
                }
                if score > alpha && score < beta && !self.stopped {
                    score = -self.negamax(state, depth - 1, -beta, -alpha, ply + 1, true, may_check);
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
/// `flaws` makes the search weaker (see `Flaws`). Each random number of the flaws comes from
/// `seed`. With `Flaws::NONE`, each seed gives the same result.
pub fn search(
    state: &mut State,
    limits: &Limits,
    variant: EvalVariant,
    options: SearchOptions,
    flaws: Flaws,
    seed: u64,
) -> Option<SearchResult> {
    let us = state.turn();
    let mut searcher = Searcher::new(state, limits, variant, options);
    let overlook_seed = mix(seed, OVERLOOK_SALT);
    let careless = mix(seed, CARELESS_SALT) % 100 < flaws.careless as u64;

    let mut list = MoveList::new();
    pseudo_moves(state, us, false, &mut list);
    let (mut inline, mut heap) = ([0i32; MoveList::CAPACITY], Vec::new());
    let scores = score_slots(&mut inline, &mut heap, list.len());
    searcher.score_moves(state, &list, scores, Move::NULL, 0);
    let mut roots: Vec<RootMove> = Vec::new();
    let mut overlooked: Vec<RootMove> = Vec::new();
    for i in 0..list.len() {
        let m = Searcher::pick(&mut list, scores, i);
        let undo = state.make(m);
        let legal = !in_check(state, us);
        state.unmake(m, undo);
        // A legal capture of the royal king wins at once. A capture of the king that leaves
        // the own king in check is not legal, as each such move.
        if legal && captures_royal(state, m) {
            return Some(SearchResult { mv: m, score: MATE - 1, depth: 1, nodes: 0, pv: vec![m] });
        }
        if legal {
            let code = (m.from as u64) << 16
                | (m.to as u64) << 8
                | (m.special as u64) << 4
                | m.promo.map_or(7, |kind| kind as u64);
            let bonus = if flaws.noise_cp > 0 { (mix(seed, code) % (flaws.noise_cp as u64 + 1)) as i32 } else { 0 };
            let root = RootMove { mv: m, bonus };
            if mix(overlook_seed, code) % 100 < flaws.overlook as u64 {
                overlooked.push(root);
            } else {
                roots.push(root);
            }
        }
    }
    // A search that overlooks each legal move sees all of them.
    if roots.is_empty() {
        roots = overlooked;
    }
    let first = roots.first()?.mv;
    let mut result = SearchResult { mv: first, score: 0, depth: 0, nodes: 0, pv: vec![first] };

    // A careless search does one iteration.
    let max_depth = if careless { 1 } else { limits.max_depth.max(1) as i32 };
    for depth in 1..=max_depth {
        // Depth 1 always completes for each root move, thus the result is never a move that
        // the search did not look at. The node limit applies from depth 2.
        searcher.max_nodes = if depth == 1 { u64::MAX } else { limits.max_nodes };
        searcher.extension_end = 2 * depth as usize + CHECK_EXTENSION_PLIES;
        // The best sum of score and bonus in this iteration.
        let mut best_sum = -INFINITE;
        let mut best_index = 0;
        let mut best_score = 0;
        for (index, root) in roots.iter().enumerate() {
            let undo = state.make(root.mv);
            // The move is better than the best move if its score is more than this number.
            let floor = if index == 0 { -INFINITE } else { best_sum - root.bonus };
            let mut score = -INFINITE;
            if careless {
                score = searcher.glance(state);
            } else {
                if index > 0 {
                    score = -searcher.negamax(state, depth - 1, -floor - 1, -floor, 1, true, true);
                }
                if (index == 0 || score > floor) && !searcher.stopped {
                    score = -searcher.negamax(state, depth - 1, -INFINITE, -floor, 1, true, true);
                }
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
