// The enemy AI: a negamax search with alpha-beta pruning.
import { inCheck, legalMoves, makeMove, pseudoMoves, unmakeMove } from './rules';
import type { Move, PieceType, State } from './types';

export interface AiLevel {
  /** The search depth in half moves. */
  depth: number;
  /** A random bonus in centipawns for each root move. It makes the AI weaker. */
  noise: number;
}

const CP: Record<PieceType, number> = { p: 100, n: 320, b: 330, r: 500, q: 900, k: 0 };
const MATE = 100000;
const BARE = 50000;

// Score in centipawns for the side to move.
function evaluate(state: State): number {
  const { board } = state;
  let score = 0, whiteMen = 0, blackMen = 0;
  for (let s = 0; s < 64; s++) {
    const p = board[s];
    if (!p) continue;
    const f = s & 7, r = s >> 3, white = p.color === 'w';
    let v = CP[p.type];
    if (p.type === 'p') {
      const advance = white ? r : 7 - r;
      v += advance * advance * 2;
    } else if (p.type === 'n' || p.type === 'b') {
      v += (14 - Math.abs(2 * f - 7) - Math.abs(2 * r - 7)) * 2;
    }
    if (p.type !== 'k') {
      if (white) whiteMen++;
      else blackMen++;
    }
    score += white ? v : -v;
  }
  if (!whiteMen) score -= BARE;
  if (!blackMen) score += BARE;
  return state.turn === 'w' ? score : -score;
}

// Puts captures of valuable pieces and promotions first.
function ordered(state: State, moves: Move[]): Move[] {
  const { board } = state;
  const score = (m: Move) => {
    const victim = board[m.to], mover = board[m.from];
    return (victim && mover ? CP[victim.type] * 10 - CP[mover.type] : 0) + (m.promo ? CP[m.promo] : 0);
  };
  return moves.map((move) => ({ move, score: score(move) })).sort((a, b) => b.score - a.score).map((e) => e.move);
}

function quiesce(state: State, alpha: number, beta: number, depth: number): number {
  const stand = evaluate(state);
  if (depth === 0 || stand >= beta) return stand;
  if (stand > alpha) alpha = stand;
  const color = state.turn;
  for (const m of ordered(state, pseudoMoves(state, color, true))) {
    const undo = makeMove(state, m);
    if (inCheck(state, color)) {
      unmakeMove(state, m, undo);
      continue;
    }
    const score = -quiesce(state, -beta, -alpha, depth - 1);
    unmakeMove(state, m, undo);
    if (score > alpha) {
      alpha = score;
      if (alpha >= beta) break;
    }
  }
  return alpha;
}

function search(state: State, depth: number, alpha: number, beta: number, ply: number): number {
  if (depth <= 0) return quiesce(state, alpha, beta, 4);
  const color = state.turn;
  let any = false;
  for (const m of ordered(state, pseudoMoves(state, color, false))) {
    const undo = makeMove(state, m);
    if (inCheck(state, color)) {
      unmakeMove(state, m, undo);
      continue;
    }
    any = true;
    const score = -search(state, depth - 1, -beta, -alpha, ply + 1);
    unmakeMove(state, m, undo);
    if (score > alpha) {
      alpha = score;
      if (alpha >= beta) break;
    }
  }
  return any ? alpha : -MATE + ply;
}

// Returns null when the side to move has no legal move.
export function chooseMove(state: State, { depth, noise }: AiLevel): Move | null {
  let best: Move | null = null, bestScore = -Infinity;
  for (const m of ordered(state, legalMoves(state))) {
    const undo = makeMove(state, m);
    // A move that scores less than bestScore - noise cannot become the best move.
    const floor = best ? bestScore - noise : -Infinity;
    const score = -search(state, depth - 1, -Infinity, -floor, 1) + Math.random() * noise;
    unmakeMove(state, m, undo);
    if (score > bestScore) {
      best = m;
      bestScore = score;
    }
  }
  return best;
}
