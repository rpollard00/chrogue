// Chess rules, move generation, and the enemy AI.
// A square is an index from 0 (a1) to 63 (h8). White moves toward higher ranks.

export const VALUE = { p: 1, n: 3, b: 3, r: 5, q: 9, k: 0 };

const KNIGHT = [[1, 2], [2, 1], [2, -1], [1, -2], [-1, -2], [-2, -1], [-2, 1], [-1, 2]];
const CAMEL = [[1, 3], [3, 1], [3, -1], [1, -3], [-1, -3], [-3, -1], [-3, 1], [-1, 3]];
const KING = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];
const ORTHO = [[1, 0], [0, 1], [-1, 0], [0, -1]];
const DIAG = [[1, 1], [-1, 1], [-1, -1], [1, -1]];
const PROMOS = ['q', 'n', 'r', 'b'];

const other = (color) => (color === 'w' ? 'b' : 'w');

// pieces: [{ id, type, color, square }]
// mods: { w: {flag: true}, b: {flag: true} }. The flags are:
// forcedMarch, backpedal, earlyPromo, kingKnight, longLeap, sidestep.
export function createState(pieces, mods = {}) {
  const board = Array(64).fill(null);
  for (const p of pieces) {
    board[p.square] = { id: p.id, type: p.type, color: p.color, moved: p.moved ?? false };
  }
  return { board, turn: 'w', ep: -1, clock: 0, mods: { w: mods.w ?? {}, b: mods.b ?? {} } };
}

export function kingSquare(state, color) {
  const { board } = state;
  for (let s = 0; s < 64; s++) {
    const p = board[s];
    if (p && p.type === 'k' && p.color === color) return s;
  }
  return -1;
}

function leaperAt(board, f, r, offsets, by, typeA, typeB) {
  for (const [df, dr] of offsets) {
    const nf = f + df, nr = r + dr;
    if (nf < 0 || nf > 7 || nr < 0 || nr > 7) continue;
    const p = board[nr * 8 + nf];
    if (p && p.color === by && (p.type === typeA || p.type === typeB)) return true;
  }
  return false;
}

function sliderAt(board, f, r, dirs, by, type) {
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

export function isAttacked(state, s, by) {
  const { board } = state, mods = state.mods[by];
  const f = s & 7, r = s >> 3;
  const pr = r - (by === 'w' ? 1 : -1);
  if (pr >= 0 && pr <= 7) {
    for (const pf of [f - 1, f + 1]) {
      if (pf < 0 || pf > 7) continue;
      const p = board[pr * 8 + pf];
      if (p && p.color === by && p.type === 'p') return true;
    }
  }
  if (leaperAt(board, f, r, KNIGHT, by, 'n', mods.kingKnight ? 'k' : 'n')) return true;
  if (mods.longLeap && leaperAt(board, f, r, CAMEL, by, 'n', 'n')) return true;
  if (leaperAt(board, f, r, KING, by, 'k', 'k')) return true;
  return sliderAt(board, f, r, ORTHO, by, 'r') || sliderAt(board, f, r, DIAG, by, 'b');
}

export function inCheck(state, color) {
  const k = kingSquare(state, color);
  return k >= 0 && isAttacked(state, k, other(color));
}

function addLeaps(board, moves, from, color, offsets, capturesOnly, quietOnly) {
  const f = from & 7, r = from >> 3;
  for (const [df, dr] of offsets) {
    const nf = f + df, nr = r + dr;
    if (nf < 0 || nf > 7 || nr < 0 || nr > 7) continue;
    const to = nr * 8 + nf, target = board[to];
    if (target ? target.color !== color && !quietOnly : !capturesOnly) moves.push({ from, to });
  }
}

function addSlides(board, moves, from, color, dirs, capturesOnly) {
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

function addPawnMove(moves, from, to, promo, extra) {
  if (promo) for (const type of PROMOS) moves.push({ from, to, promo: type, ...extra });
  else moves.push({ from, to, ...extra });
}

function addPawnMoves(state, moves, from, p, capturesOnly) {
  const { board } = state, mods = state.mods[p.color];
  const white = p.color === 'w', dir = white ? 1 : -1;
  const f = from & 7, r = from >> 3;
  const promoRank = white ? (mods.earlyPromo ? 6 : 7) : (mods.earlyPromo ? 1 : 0);
  const isPromo = (rank) => (white ? rank >= promoRank : rank <= promoRank);
  const r1 = r + dir;
  if (r1 >= 0 && r1 <= 7) {
    const one = r1 * 8 + f;
    if (!board[one]) {
      if (!capturesOnly || isPromo(r1)) addPawnMove(moves, from, one, isPromo(r1));
      const r2 = r1 + dir;
      if ((!p.moved || mods.forcedMarch) && r2 >= 0 && r2 <= 7 && !board[r2 * 8 + f]) {
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
  if (mods.backpedal && !capturesOnly && rb >= 0 && rb <= 7 && !board[rb * 8 + f]) {
    moves.push({ from, to: rb * 8 + f, back: true });
  }
}

function addCastles(state, moves, from, king) {
  const { board } = state, color = king.color, foe = other(color);
  const home = color === 'w' ? 4 : 60;
  if (king.moved || from !== home || isAttacked(state, from, foe)) return;
  const rookReady = (s) => {
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
function pseudoMoves(state, color, capturesOnly) {
  const { board } = state, mods = state.mods[color], moves = [];
  for (let from = 0; from < 64; from++) {
    const p = board[from];
    if (!p || p.color !== color) continue;
    switch (p.type) {
      case 'p':
        addPawnMoves(state, moves, from, p, capturesOnly);
        break;
      case 'n':
        addLeaps(board, moves, from, color, KNIGHT, capturesOnly);
        if (mods.longLeap) addLeaps(board, moves, from, color, CAMEL, capturesOnly);
        break;
      case 'b':
        addSlides(board, moves, from, color, DIAG, capturesOnly);
        if (mods.sidestep && !capturesOnly) addLeaps(board, moves, from, color, ORTHO, false, true);
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
        if (mods.kingKnight) addLeaps(board, moves, from, color, KNIGHT, capturesOnly);
        if (!capturesOnly) addCastles(state, moves, from, p);
        break;
    }
  }
  return moves;
}

// Returns the data that unmakeMove needs. undo.captured is the captured piece or null.
export function makeMove(state, m) {
  const b = state.board, p = b[m.from];
  const capSq = m.epCapture ? m.to + (p.color === 'w' ? -8 : 8) : m.to;
  const undo = { captured: b[capSq], capSq, ep: state.ep, clock: state.clock, moved: p.moved, type: p.type };
  b[capSq] = null;
  b[m.from] = null;
  b[m.to] = p;
  if (m.promo) p.type = m.promo;
  p.moved = true;
  if (m.castle) {
    const rook = b[m.castle[0]];
    b[m.castle[0]] = null;
    b[m.castle[1]] = rook;
    rook.moved = true;
  }
  state.ep = m.ep ?? -1;
  state.clock = undo.captured || (undo.type === 'p' && !m.back) ? 0 : state.clock + 1;
  state.turn = other(p.color);
  return undo;
}

export function unmakeMove(state, m, undo) {
  const b = state.board, p = b[m.to];
  if (m.castle) {
    const rook = b[m.castle[1]];
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

export function legalMoves(state) {
  const color = state.turn;
  return pseudoMoves(state, color, false).filter((m) => {
    const undo = makeMove(state, m);
    const ok = !inCheck(state, color);
    unmakeMove(state, m, undo);
    return ok;
  });
}

// Returns null while the battle continues.
// A side loses when it has no legal move or when only its king remains.
export function outcome(state) {
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

// --- AI ---

const CP = { p: 100, n: 320, b: 330, r: 500, q: 900, k: 0 };
const MATE = 100000;
const BARE = 50000;

// Score in centipawns for the side to move.
function evaluate(state) {
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
    if (p.type !== 'k') white ? whiteMen++ : blackMen++;
    score += white ? v : -v;
  }
  if (!whiteMen) score -= BARE;
  if (!blackMen) score += BARE;
  return state.turn === 'w' ? score : -score;
}

function ordered(state, moves) {
  const { board } = state;
  for (const m of moves) {
    const victim = board[m.to];
    m.order = (victim ? CP[victim.type] * 10 - CP[board[m.from].type] : 0) + (m.promo ? CP[m.promo] : 0);
  }
  return moves.sort((a, b) => b.order - a.order);
}

function quiesce(state, alpha, beta, depth) {
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

function search(state, depth, alpha, beta, ply) {
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

// noise is a random bonus in centipawns for each root move. It makes the AI weaker.
export function chooseMove(state, { depth = 2, noise = 0 } = {}) {
  let best = null, bestScore = -Infinity;
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
