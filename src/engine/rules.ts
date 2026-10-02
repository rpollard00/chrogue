// Chess rules: move generation, make and unmake, and the result of a battle.
import { other } from './types';
import type { Color, Move, Outcome, Piece, PieceSetup, RuleSet, Square, State, Undo } from './types';

type Offsets = readonly (readonly [number, number])[];
type Board = State['board'];

const KNIGHT: Offsets = [[1, 2], [2, 1], [2, -1], [1, -2], [-1, -2], [-2, -1], [-2, 1], [-1, 2]];
const CAMEL: Offsets = [[1, 3], [3, 1], [3, -1], [1, -3], [-1, -3], [-3, -1], [-3, 1], [-1, 3]];
const KING: Offsets = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];
const ORTHO: Offsets = [[1, 0], [0, 1], [-1, 0], [0, -1]];
const DIAG: Offsets = [[1, 1], [-1, 1], [-1, -1], [1, -1]];
const PROMOS = ['q', 'n', 'r', 'b'] as const;

export function createState(pieces: PieceSetup[], rules: Partial<Record<Color, RuleSet>> = {}): State {
  const board: Board = Array(64).fill(null);
  for (const p of pieces) {
    board[p.square] = { id: p.id, type: p.type, color: p.color, moved: p.moved ?? false };
  }
  return { board, turn: 'w', ep: -1, clock: 0, rules: { w: rules.w ?? {}, b: rules.b ?? {} } };
}

function pieceAt(board: Board, s: Square): Piece {
  const p = board[s];
  if (!p) throw new Error(`No piece on square ${s}`);
  return p;
}

export function kingSquare(state: State, color: Color): Square {
  const { board } = state;
  for (let s = 0; s < 64; s++) {
    const p = board[s];
    if (p && p.type === 'k' && p.color === color) return s;
  }
  return -1;
}

function leaperAt(board: Board, f: number, r: number, offsets: Offsets, by: Color, typeA: string, typeB: string) {
  for (const [df, dr] of offsets) {
    const nf = f + df, nr = r + dr;
    if (nf < 0 || nf > 7 || nr < 0 || nr > 7) continue;
    const p = board[nr * 8 + nf];
    if (p && p.color === by && (p.type === typeA || p.type === typeB)) return true;
  }
  return false;
}

function sliderAt(board: Board, f: number, r: number, dirs: Offsets, by: Color, type: string) {
  for (const [df, dr] of dirs) {
    let nf = f + df, nr = r + dr;
    while (nf >= 0 && nf <= 7 && nr >= 0 && nr <= 7) {
      const p = board[nr * 8 + nf];
      if (p) {
        if (p.color === by && (p.type === type || p.type === 'q')) return true;
        break;
      }
      nf += df;
      nr += dr;
    }
  }
  return false;
}

export function isAttacked(state: State, s: Square, by: Color): boolean {
  const { board } = state, rules = state.rules[by];
  const f = s & 7, r = s >> 3;
  const pr = r - (by === 'w' ? 1 : -1);
  if (pr >= 0 && pr <= 7) {
    for (const pf of [f - 1, f + 1]) {
      if (pf < 0 || pf > 7) continue;
      const p = board[pr * 8 + pf];
      if (p && p.color === by && p.type === 'p') return true;
    }
  }
  if (leaperAt(board, f, r, KNIGHT, by, 'n', rules.kingKnight ? 'k' : 'n')) return true;
  if (rules.longLeap && leaperAt(board, f, r, CAMEL, by, 'n', 'n')) return true;
  if (leaperAt(board, f, r, KING, by, 'k', 'k')) return true;
  return sliderAt(board, f, r, ORTHO, by, 'r') || sliderAt(board, f, r, DIAG, by, 'b');
}

export function inCheck(state: State, color: Color): boolean {
  const k = kingSquare(state, color);
  return k >= 0 && isAttacked(state, k, other(color));
}

function addLeaps(
  board: Board, moves: Move[], from: Square, color: Color, offsets: Offsets, capturesOnly: boolean, quietOnly = false,
) {
  const f = from & 7, r = from >> 3;
  for (const [df, dr] of offsets) {
    const nf = f + df, nr = r + dr;
    if (nf < 0 || nf > 7 || nr < 0 || nr > 7) continue;
    const to = nr * 8 + nf, target = board[to];
    if (target ? target.color !== color && !quietOnly : !capturesOnly) moves.push({ from, to });
  }
}

function addSlides(board: Board, moves: Move[], from: Square, color: Color, dirs: Offsets, capturesOnly: boolean) {
  const f = from & 7, r = from >> 3;
  for (const [df, dr] of dirs) {
    let nf = f + df, nr = r + dr;
    while (nf >= 0 && nf <= 7 && nr >= 0 && nr <= 7) {
      const to = nr * 8 + nf, target = board[to];
      if (target) {
        if (target.color !== color) moves.push({ from, to });
        break;
      }
      if (!capturesOnly) moves.push({ from, to });
      nf += df;
      nr += dr;
    }
  }
}

function addPawnMove(moves: Move[], from: Square, to: Square, promo: boolean, extra: Partial<Move> = {}) {
  if (promo) for (const type of PROMOS) moves.push({ from, to, promo: type, ...extra });
  else moves.push({ from, to, ...extra });
}

function addPawnMoves(state: State, moves: Move[], from: Square, p: Piece, capturesOnly: boolean) {
  const { board } = state, rules = state.rules[p.color];
  const white = p.color === 'w', dir = white ? 1 : -1;
  const f = from & 7, r = from >> 3;
  const promoRank = white ? (rules.earlyPromo ? 6 : 7) : (rules.earlyPromo ? 1 : 0);
  const isPromo = (rank: number) => (white ? rank >= promoRank : rank <= promoRank);
  const r1 = r + dir;
  if (r1 >= 0 && r1 <= 7) {
    const one = r1 * 8 + f;
    if (!board[one]) {
      if (!capturesOnly || isPromo(r1)) addPawnMove(moves, from, one, isPromo(r1));
      const r2 = r1 + dir;
      if ((!p.moved || rules.forcedMarch) && r2 >= 0 && r2 <= 7 && !board[r2 * 8 + f]) {
        const promo = isPromo(r2);
        if (!capturesOnly || promo) addPawnMove(moves, from, r2 * 8 + f, promo, promo ? {} : { ep: one });
      }
    }
    for (const nf of [f - 1, f + 1]) {
      if (nf < 0 || nf > 7) continue;
      const to = r1 * 8 + nf, target = board[to];
      if (target) {
        if (target.color !== p.color) addPawnMove(moves, from, to, isPromo(r1));
      } else if (to === state.ep) {
        const victim = board[to - 8 * dir];
        if (victim && victim.type === 'p' && victim.color !== p.color) {
          addPawnMove(moves, from, to, isPromo(r1), { epCapture: true });
        }
      }
    }
  }
  const rb = r - dir;
  if (rules.backpedal && !capturesOnly && rb >= 0 && rb <= 7 && !board[rb * 8 + f]) {
    moves.push({ from, to: rb * 8 + f, back: true });
  }
}

function addCastles(state: State, moves: Move[], from: Square, king: Piece) {
  const { board } = state, color = king.color, foe = other(color);
  const home = color === 'w' ? 4 : 60;
  if (king.moved || from !== home || isAttacked(state, from, foe)) return;
  const rookReady = (s: Square) => {
    const rook = board[s];
    return rook && rook.type === 'r' && rook.color === color && !rook.moved;
  };
  if (rookReady(from + 3) && !board[from + 1] && !board[from + 2] && !isAttacked(state, from + 1, foe)) {
    moves.push({ from, to: from + 2, castle: [from + 3, from + 1] });
  }
  if (rookReady(from - 4) && !board[from - 1] && !board[from - 2] && !board[from - 3]
    && !isAttacked(state, from - 1, foe)) {
    moves.push({ from, to: from - 2, castle: [from - 4, from - 1] });
  }
}

// Moves that obey piece movement but can leave the king in check.
export function pseudoMoves(state: State, color: Color, capturesOnly: boolean): Move[] {
  const { board } = state, rules = state.rules[color], moves: Move[] = [];
  for (let from = 0; from < 64; from++) {
    const p = board[from];
    if (!p || p.color !== color) continue;
    switch (p.type) {
      case 'p':
        addPawnMoves(state, moves, from, p, capturesOnly);
        break;
      case 'n':
        addLeaps(board, moves, from, color, KNIGHT, capturesOnly);
        if (rules.longLeap) addLeaps(board, moves, from, color, CAMEL, capturesOnly);
        break;
      case 'b':
        addSlides(board, moves, from, color, DIAG, capturesOnly);
        if (rules.sidestep && !capturesOnly) addLeaps(board, moves, from, color, ORTHO, false, true);
        break;
      case 'r':
        addSlides(board, moves, from, color, ORTHO, capturesOnly);
        break;
      case 'q':
        addSlides(board, moves, from, color, ORTHO, capturesOnly);
        addSlides(board, moves, from, color, DIAG, capturesOnly);
        break;
      case 'k':
        addLeaps(board, moves, from, color, KING, capturesOnly);
        if (rules.kingKnight) addLeaps(board, moves, from, color, KNIGHT, capturesOnly);
        if (!capturesOnly) addCastles(state, moves, from, p);
        break;
    }
  }
  return moves;
}

export function makeMove(state: State, m: Move): Undo {
  const b = state.board, p = pieceAt(b, m.from);
  const capSq = m.epCapture ? m.to + (p.color === 'w' ? -8 : 8) : m.to;
  const undo: Undo = { captured: b[capSq], capSq, ep: state.ep, clock: state.clock, moved: p.moved, type: p.type };
  b[capSq] = null;
  b[m.from] = null;
  b[m.to] = p;
  if (m.promo) p.type = m.promo;
  p.moved = true;
  if (m.castle) {
    const rook = pieceAt(b, m.castle[0]);
    b[m.castle[0]] = null;
    b[m.castle[1]] = rook;
    rook.moved = true;
  }
  state.ep = m.ep ?? -1;
  state.clock = undo.captured || (undo.type === 'p' && !m.back) ? 0 : state.clock + 1;
  state.turn = other(p.color);
  return undo;
}

export function unmakeMove(state: State, m: Move, undo: Undo): void {
  const b = state.board, p = pieceAt(b, m.to);
  if (m.castle) {
    const rook = pieceAt(b, m.castle[1]);
    b[m.castle[1]] = null;
    b[m.castle[0]] = rook;
    rook.moved = false;
  }
  b[m.to] = null;
  b[m.from] = p;
  p.type = undo.type;
  p.moved = undo.moved;
  b[undo.capSq] = undo.captured;
  state.ep = undo.ep;
  state.clock = undo.clock;
  state.turn = p.color;
}

export function legalMoves(state: State): Move[] {
  const color = state.turn;
  return pseudoMoves(state, color, false).filter((m) => {
    const undo = makeMove(state, m);
    const ok = !inCheck(state, color);
    unmakeMove(state, m, undo);
    return ok;
  });
}

// The legal moves of the piece on a square, as if its side has the move.
// A side that does not have the move cannot capture en passant.
export function movesFrom(state: State, from: Square): Move[] {
  const p = state.board[from];
  if (!p) return [];
  const { turn, ep } = state;
  state.turn = p.color;
  if (p.color !== turn) state.ep = -1;
  const moves = legalMoves(state).filter((m) => m.from === from);
  state.turn = turn;
  state.ep = ep;
  return moves;
}

// Returns null while the battle continues.
// A side loses when it has no legal move or when only its king remains.
export function outcome(state: State): Outcome | null {
  const men = { w: 0, b: 0 };
  for (const p of state.board) if (p && p.type !== 'k') men[p.color]++;
  if (!men.w && !men.b) return { winner: null, reason: 'bare' };
  if (!men.b) return { winner: 'w', reason: 'rout' };
  if (!men.w) return { winner: 'b', reason: 'rout' };
  if (!legalMoves(state).length) {
    return { winner: other(state.turn), reason: inCheck(state, state.turn) ? 'checkmate' : 'stalemate' };
  }
  if (state.clock >= 100) return { winner: null, reason: 'clock' };
  return null;
}
