// The differential test: it compares the results of the Rust engine with the results of the
// TypeScript engine on the same positions and games.
//
// Usage: bun core/difftest/run.ts [--seed N] [--playouts N]
//
// The script plays random legal games with the TypeScript engine. It sends the positions and
// the games to the Rust tool `difftest`, and compares each answer with the TypeScript engine.
// The run fails if a move kind or a result occurs fewer times than its minimum in `MINIMUMS`.
// The same seed always gives the same games.
import { resolve } from 'node:path';
import {
  VALUE, createState, inCheck, isAttacked, legalMoves, makeMove, movesFrom, outcome, pseudoMoves, unmakeMove,
} from '../../src/engine';
import type { Color, Move, MoveRules, PieceSetup, PieceType, RuleSet, Square, State } from '../../src/engine';
import { baseArmy, freeHome } from '../../src/game/army';
import { FLOORS } from '../../src/game/floors';
import { CONSCRIPT_ID } from '../../src/game/relics';

// ---- Options ----

function option(name: string, fallback: number): number {
  const i = process.argv.indexOf(`--${name}`);
  if (i < 0) return fallback;
  const value = Number(process.argv[i + 1]);
  if (!Number.isInteger(value) || value < 0) throw new Error(`--${name} must be a whole number`);
  return value;
}

const SEED = option('seed', 1);
const PLAYOUTS = option('playouts', 768);
/** The minimum number of positions. */
const MIN_POSITIONS = 3000;
/** The minimum number of positions with a perft of depth 3. */
const MIN_DEEP = 300;
/** The minimum number of positions for each rule combination of each side. */
const MIN_PER_COMBO = 20;
/** The number of games from the start that gives en passant captures with a promotion. */
const EP_PROMO_PLAYOUTS = 64;

// ---- Random numbers ----

// mulberry32. The script does not use Math.random, thus a seed gives the same run each time.
function makeRng(seed: number) {
  let a = seed >>> 0;
  const next = (): number => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const int = (n: number): number => Math.floor(next() * n);
  const pick = <T>(list: readonly T[]): T => list[int(list.length)];
  const shuffle = <T>(list: readonly T[]): T[] => {
    const a = [...list];
    for (let i = a.length - 1; i > 0; i--) {
      const j = int(i + 1);
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  };
  return { next, int, pick, shuffle };
}
type Rng = ReturnType<typeof makeRng>;

// ---- Rules ----

const FLAGS: (keyof MoveRules)[] = ['forcedMarch', 'backpedal', 'earlyPromo', 'kingKnight', 'longLeap', 'sidestep'];
const COMBOS = 1 << FLAGS.length;
type RuleFlags = Record<Color, (keyof MoveRules)[]>;

const flagsOf = (combo: number): (keyof MoveRules)[] => FLAGS.filter((_, bit) => combo & (1 << bit));
const ruleSet = (flags: (keyof MoveRules)[]): RuleSet => Object.fromEntries(flags.map((flag) => [flag, true]));

// ---- Start positions ----

interface Start {
  name: string;
  pieces: PieceSetup[];
  turn: Color;
  clock: number;
}

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR';
const KIWIPETE_FEN = 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R';

// Reads the piece field of a FEN string. A pawn off its start rank counts as moved.
function fenPieces(fen: string): PieceSetup[] {
  const pieces: PieceSetup[] = [];
  fen.split('/').forEach((row, i) => {
    let f = 0;
    for (const ch of row) {
      if (ch >= '1' && ch <= '8') {
        f += Number(ch);
        continue;
      }
      const color: Color = ch === ch.toUpperCase() ? 'w' : 'b', type = ch.toLowerCase() as PieceType, r = 7 - i;
      const moved = type === 'p' && r !== (color === 'w' ? 1 : 6);
      pieces.push({ id: pieces.length, type, color, square: r * 8 + f++, moved });
    }
  });
  return pieces;
}

const RECRUITS = ['p', 'n', 'b', 'r', 'q'] as const;
const WEIGHT: Record<(typeof RECRUITS)[number], number> = { p: 4, n: 2, b: 2, r: 1.5, q: 1 };

// The enemy army of a floor. The steps are those of generateEnemy in src/game/floors.ts,
// but with the seeded random numbers of this script.
function enemyPieces(floor: number, rng: Rng): { type: PieceType; square: Square }[] {
  const caps = { p: 8, n: 2, b: 2, r: 2, q: floor >= 5 ? 1 : 0 };
  const counts = { p: 0, n: 0, b: 0, r: 0, q: 0 };
  let budget = FLOORS[floor - 1].budget;
  for (;;) {
    const pool = RECRUITS.filter((t) => counts[t] < caps[t] && VALUE[t] <= budget);
    if (!pool.length) break;
    let roll = rng.next() * pool.reduce((sum, t) => sum + WEIGHT[t], 0);
    const type = pool.find((t) => (roll -= WEIGHT[t]) < 0) ?? pool[pool.length - 1];
    counts[type]++;
    budget -= VALUE[type];
  }
  const squares = {
    r: rng.shuffle([56, 63]), n: rng.shuffle([57, 62]), b: rng.shuffle([58, 61]), q: [59],
    p: [52, 51, 53, 50, 54, 49, 55, 48],
  };
  const pieces: { type: PieceType; square: Square }[] = [{ type: 'k', square: 60 }];
  for (const type of RECRUITS) {
    for (let i = 0; i < counts[type]; i++) pieces.push({ type, square: squares[type][i] });
  }
  return pieces;
}

// An army of the player against the enemy army of a floor, as createBattle in src/game/battle.ts makes it.
function gameStart(rng: Rng): Start {
  const floor = 1 + rng.int(FLOORS.length);
  const army = baseArmy();
  const recruits = rng.int(2 + floor);
  for (let i = 0; i < recruits && army.length < 16; i++) {
    const type = rng.pick(['p', 'p', 'p', 'n', 'b', 'r', 'q'] as const);
    army.push({ id: army.length + 1, type, home: freeHome(army, type) });
  }
  // The player can arrange the first two ranks, thus a pawn can start on rank 1.
  for (let i = rng.int(4); i > 0; i--) {
    const unit = rng.pick(army), to = rng.int(16), occupant = army.find((u) => u.home === to);
    if (occupant) occupant.home = unit.home;
    unit.home = to;
  }
  const pieces: PieceSetup[] = army.map((unit) => ({ id: unit.id, type: unit.type, color: 'w', square: unit.home }));
  // The relic Conscription adds one pawn on the first free square of rank 2 or rank 3.
  if (rng.next() < 0.2) {
    for (let square = 8; square < 24; square++) {
      if (pieces.some((p) => p.square === square)) continue;
      pieces.push({ id: CONSCRIPT_ID, type: 'p', color: 'w', square });
      break;
    }
  }
  // The ids of the enemy pieces are those of createBattle: "e0", "e1", and so on.
  enemyPieces(floor, rng).forEach((e, i) => pieces.push({ id: `e${i}`, type: e.type, color: 'b', square: e.square }));
  return { name: `game floor ${floor}`, pieces, turn: 'w', clock: 0 };
}

// Random pieces on random squares. The side that does not have the move can be in check.
function scatterStart(rng: Rng): Start {
  const squares = rng.shuffle([...Array(64).keys()]);
  const pieces: PieceSetup[] = [];
  for (const color of ['w', 'b'] as const) {
    if (rng.next() < 0.9) pieces.push({ id: pieces.length, type: 'k', color, square: squares.pop()!, moved: rng.next() < 0.5 });
    for (let men = 1 + rng.int(9); men > 0; men--) {
      const type = rng.pick(['p', 'p', 'p', 'n', 'b', 'r', 'q'] as const);
      let square = squares.pop()!;
      // A pawn does not stand on its last rank.
      while (type === 'p' && (square >> 3) === (color === 'w' ? 7 : 0)) square = squares.pop()!;
      pieces.push({ id: pieces.length, type, color, square, moved: rng.next() < 0.5 });
    }
  }
  return { name: 'scatter', pieces, turn: rng.pick(['w', 'b']), clock: rng.int(20) };
}

// Positions where a side has no pawns, many promoted pieces, only a king, or no king.
const SPECIAL_FENS: [name: string, fen: string][] = [
  ['no pawns', 'r1b1k1nr/8/8/8/8/8/8/RN2K1BR'],
  ['white has no pawns', 'rnbqkbnr/pppppppp/8/8/8/8/8/RNBQKBNR'],
  ['black has no pawns', 'r3k2r/8/8/8/8/8/PPPPPPPP/R3K2R'],
  ['promotion race', '4k3/PPPP1PPP/8/8/8/8/pppp1ppp/4K3'],
  ['late promotion race', '4k3/8/PPP2PPP/8/8/ppp2ppp/8/4K3'],
  ['many queens', 'qqq1k1qq/8/8/8/8/8/8/QQQ1K1QQ'],
  ['many promoted pieces', 'qnrbkqqn/2p2p2/8/8/8/8/2P2P2/QNRBKQQN'],
  ['black has only a king', '4k3/8/8/8/8/8/PPPP4/R3K1N1'],
  ['white has only a king', 'rn2k3/pppp4/8/8/8/8/8/4K3'],
  ['two kings only', '4k3/8/8/8/8/8/8/4K3'],
  ['king and one man each', '4k3/7p/8/8/8/8/P7/4K3'],
  ['rook endgame', '4k2r/8/8/8/8/8/8/R3K3'],
  ['no kings', '8/pppp4/1n6/8/8/1N6/PPPP4/R7'],
  ['white has no king', 'rn2k3/pppp4/8/8/8/8/PPPP4/RN6'],
  ['two white kings', '4k3/pppp4/8/8/8/8/PPPP4/RK2K2R'],
  ['castles', 'r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R'],
  ['pawns only', '4k3/pppppppp/8/8/8/8/PPPPPPPP/4K3'],
  ['pawns face to face', '4k3/8/pppppppp/8/8/PPPPPPPP/8/4K3'],
  ['en passant for White', '4k3/pppppppp/8/PPPPPPPP/8/8/8/4K3'],
  ['en passant for Black', '4k3/8/8/8/pppppppp/8/PPPPPPPP/4K3'],
  ['knights and bishops', '1nb1kbn1/1nb2bn1/8/8/8/8/1NB2BN1/1NB1KBN1'],
];

// Positions where more than one end of the battle is true. `outcome` must do its checks in
// the same order in the two engines: bare, rout, no legal move, clock.
const ENDINGS: Start[] = [
  { name: 'checkmate at the clock limit', pieces: fenPieces('R5k1/5ppp/8/8/8/8/8/4K3'), turn: 'b', clock: 100 },
  { name: 'checkmate after the clock limit', pieces: fenPieces('6k1/5ppp/8/8/8/8/5PPP/r5K1'), turn: 'w', clock: 130 },
  { name: 'stalemate at the clock limit', pieces: fenPieces('k7/2Q5/1K6/8/8/7p/7P/8'), turn: 'b', clock: 100 },
  { name: 'stalemate after the clock limit', pieces: fenPieces('8/8/8/8/8/1k5p/2q4P/K7'), turn: 'w', clock: 140 },
  { name: 'rout at the clock limit', pieces: fenPieces('4k3/8/8/8/8/8/P7/4K3'), turn: 'b', clock: 100 },
  { name: 'rout and checkmate at the clock limit', pieces: fenPieces('R3k3/8/4K3/8/8/8/8/8'), turn: 'b', clock: 120 },
  { name: 'rout of White after the clock limit', pieces: fenPieces('4k3/p7/8/8/8/8/8/4K3'), turn: 'w', clock: 199 },
  { name: 'bare kings at the clock limit', pieces: fenPieces('4k3/8/8/8/8/8/8/4K3'), turn: 'w', clock: 100 },
  { name: 'clock limit with legal moves', pieces: fenPieces('r3k3/p7/8/8/8/8/P7/R3K3'), turn: 'w', clock: 100 },
  { name: 'bare kings after the clock limit', pieces: fenPieces('k7/8/1K6/8/8/8/8/8'), turn: 'b', clock: 150 },
];

// White pawns on rank 1 that have not moved, and black pawns on rank 3. A double step of a
// white pawn crosses rank 2. With earlyPromo for Black, a capture en passant there promotes.
function epPromoStart(): Start {
  const pieces = fenPieces('4k3/8/8/8/8/1p1p1p2/8/P1P1P1PK').map((p) => ({ ...p, moved: p.color === 'w' && p.type === 'p' ? false : p.moved }));
  return { name: 'en passant promotion', pieces, turn: 'w', clock: 0 };
}

function makeStart(index: number, rng: Rng): Start {
  const turn = rng.pick<Color>(['w', 'b']);
  switch (index % 8) {
    case 0:
      return { name: 'start', pieces: fenPieces(START_FEN), turn: 'w', clock: 0 };
    case 1:
      return { name: 'kiwipete', pieces: fenPieces(KIWIPETE_FEN), turn, clock: 0 };
    case 2:
    case 3:
    case 4:
      return gameStart(rng);
    case 5:
      return scatterStart(rng);
    default: {
      const [name, fen] = SPECIAL_FENS[rng.int(SPECIAL_FENS.length)];
      // A clock near the limit makes the result "clock" possible.
      return { name, pieces: fenPieces(fen), turn, clock: rng.next() < 0.3 ? 80 + rng.int(20) : 0 };
    }
  }
}

// ---- The data that goes to the Rust tool ----

type PlainState = Omit<State, 'rules'>;
const plain = (state: State): PlainState => ({ board: state.board, turn: state.turn, ep: state.ep, clock: state.clock });

function stateFrom(data: PlainState, flags: RuleFlags): State {
  return {
    board: data.board.map((p) => (p ? { ...p } : null)),
    turn: data.turn, ep: data.ep, clock: data.clock,
    rules: { w: ruleSet(flags.w), b: ruleSet(flags.b) },
  };
}

interface AnalyzeJob {
  op: 'analyze';
  label: string;
  state: PlainState;
  rules: RuleFlags;
  perft: number[];
}
interface PlayoutJob {
  op: 'playout';
  label: string;
  state: PlainState;
  rules: RuleFlags;
  moves: Move[];
  /** The state after each move, from the TypeScript engine. */
  expected: string[];
}
type Job = AnalyzeJob | PlayoutJob;

/** The answer of the Rust tool to an AnalyzeJob. */
interface RustAnalysis {
  moves: Move[];
  movesFrom: Record<string, Move[]>;
  pseudo: Record<Color, { all: Move[]; captures: Move[] }>;
  /** One character for each square: 1 if the color attacks the square. */
  attacked: Record<Color, string>;
  inCheck: Record<Color, boolean>;
  outcome: { winner: Color | null; reason: string } | null;
  perft: number[];
}
/** The answer of the Rust tool to a PlayoutJob. */
interface RustPlayout {
  states: PlainState[];
  /** The first move (from 1) after which the bitboards, the mailbox, and the key disagree. 0 is the start. */
  inconsistentAfter: number | null;
  /** The first move (from 1) whose take-back did not give back the state before the move. */
  notRestored: number | null;
}

// ---- Comparison ----

const moveKey = (m: Move): string =>
  `${m.from}>${m.to} promo=${m.promo ?? '-'} ep=${m.ep ?? '-'} epCapture=${m.epCapture ? 1 : 0}`
  + ` back=${m.back ? 1 : 0} castle=${m.castle ? m.castle.join('>') : '-'}`;
const moveKeys = (moves: Move[]): string[] => moves.map(moveKey).sort();

const stateKey = (state: PlainState): string =>
  state.board.map((p) => (p ? `${p.id}${p.type}${p.color}${p.moved ? '+' : ''}` : '.')).join(' ')
  + ` turn=${state.turn} ep=${state.ep} clock=${state.clock}`;

function diagram(state: PlainState): string {
  const rows: string[] = [];
  for (let r = 7; r >= 0; r--) {
    let row = `${r + 1} `;
    for (let f = 0; f < 8; f++) {
      const p = state.board[r * 8 + f];
      row += p ? (p.color === 'w' ? p.type.toUpperCase() : p.type) : '.';
    }
    rows.push(row);
  }
  return `${rows.join('\n')}\n  abcdefgh`;
}

let mismatches = 0;

function report(job: Job, what: string, detail: string): void {
  mismatches++;
  if (mismatches > 10) return;
  console.error(`\nMISMATCH in ${what} (${job.label})`);
  console.error(diagram(job.state));
  console.error(`rules: ${JSON.stringify(job.rules)}`);
  console.error(`state: ${JSON.stringify(job.state)}`);
  console.error(detail);
}

function compareMoves(job: Job, what: string, ts: Move[], rust: Move[]): void {
  const a = moveKeys(ts), b = moveKeys(rust);
  if (a.length === b.length && a.every((key, i) => key === b[i])) return;
  const onlyTs = a.filter((key) => !b.includes(key)), onlyRust = b.filter((key) => !a.includes(key));
  report(job, what, [
    `TypeScript has ${a.length} moves and Rust has ${b.length} moves.`,
    `Only TypeScript: ${onlyTs.join(' | ') || 'none'}`,
    `Only Rust: ${onlyRust.join(' | ') || 'none'}`,
    'A list that has the same move two times also gives this report.',
  ].join('\n'));
}

function compareValue(job: Job, what: string, ts: unknown, rust: unknown): void {
  const a = JSON.stringify(ts), b = JSON.stringify(rust);
  if (a !== b) report(job, what, `TypeScript: ${a}\nRust: ${b}`);
}

function perft(state: State, depth: number): number {
  if (depth === 0) return 1;
  let nodes = 0;
  for (const m of legalMoves(state)) {
    const undo = makeMove(state, m);
    nodes += perft(state, depth - 1);
    unmakeMove(state, m, undo);
  }
  return nodes;
}

const COLORS: Color[] = ['w', 'b'];
const seen = {
  moves: 0, ep: 0, epCapture: 0, epPromo: 0, back: 0, check: 0,
  promo: { q: 0, n: 0, r: 0, b: 0 },
  castle: { wK: 0, wQ: 0, bK: 0, bQ: 0 },
};
const outcomes: Record<string, number> = {};
let perftNodes = 0;

function compareAnalysis(job: AnalyzeJob, rust: RustAnalysis): void {
  const state = stateFrom(job.state, job.rules);
  const before = stateKey(state);

  const moves = legalMoves(state);
  compareMoves(job, 'legalMoves', moves, rust.moves);
  seen.moves += moves.length;
  for (const m of moves) {
    if (m.promo) seen.promo[m.promo]++;
    if (m.ep !== undefined) seen.ep++;
    if (m.epCapture) seen.epCapture++;
    if (m.epCapture && m.promo) seen.epPromo++;
    if (m.back) seen.back++;
    if (m.castle) seen.castle[`${m.from === 4 ? 'w' : 'b'}${m.to > m.from ? 'K' : 'Q'}`]++;
  }

  const occupied = state.board.flatMap((p, s) => (p ? [s] : []));
  compareValue(job, 'the squares of movesFrom', occupied.map(String), Object.keys(rust.movesFrom).sort((a, b) => +a - +b));
  for (const s of occupied) compareMoves(job, `movesFrom(${s})`, movesFrom(state, s), rust.movesFrom[s] ?? []);

  for (const color of COLORS) {
    compareMoves(job, `pseudoMoves(${color})`, pseudoMoves(state, color, false), rust.pseudo[color].all);
    compareMoves(job, `pseudoMoves(${color}, capturesOnly)`, pseudoMoves(state, color, true), rust.pseudo[color].captures);
    const attacked = state.board.map((_, s) => (isAttacked(state, s, color) ? '1' : '0')).join('');
    compareValue(job, `isAttacked by ${color}`, attacked, rust.attacked[color]);
    const check = inCheck(state, color);
    compareValue(job, `inCheck(${color})`, check, rust.inCheck[color]);
    if (check) seen.check++;
  }

  const result = outcome(state);
  const outcomeKey = (o: { winner: Color | null; reason: string } | null) => (o ? `${o.reason}, winner ${o.winner}` : 'none');
  compareValue(job, 'outcome', outcomeKey(result), outcomeKey(rust.outcome));
  const reason = result?.reason ?? 'none';
  outcomes[reason] = (outcomes[reason] ?? 0) + 1;

  const counts = job.perft.map((depth) => perft(state, depth));
  compareValue(job, `perft at depths ${job.perft.join(', ')}`, counts, rust.perft);
  perftNodes += counts.reduce((sum, n) => sum + n, 0);

  if (stateKey(state) !== before) throw new Error('The TypeScript engine changed the state during the analysis');
}

function comparePlayout(job: PlayoutJob, rust: RustPlayout): void {
  const states: string[] = rust.states.map(stateKey);
  if (states.length !== job.expected.length) {
    report(job, 'playout', `TypeScript made ${job.expected.length} moves and Rust made ${states.length} moves.`);
    return;
  }
  const i = states.findIndex((key, ply) => key !== job.expected[ply]);
  if (i >= 0) {
    report(job, `the state after move ${i + 1} of a playout`, [
      `The moves until the difference: ${JSON.stringify(job.moves.slice(0, i + 1))}`,
      `TypeScript: ${job.expected[i]}`,
      `Rust:       ${states[i]}`,
    ].join('\n'));
  }
  if (rust.notRestored !== null) {
    report(job, 'unmake', `The take-back of move ${rust.notRestored} did not give back the state before the move.`);
  }
  if (rust.inconsistentAfter !== null) {
    report(job, 'the bitboards and the key', `After move ${rust.inconsistentAfter}, the bitboards, the mailbox, and the Zobrist key of Rust disagree.`);
  }
}

// ---- Play the games ----

const rng = makeRng(SEED);
const jobs: Job[] = [];
const coverage: Record<Color, number[]> = { w: Array(COMBOS).fill(0), b: Array(COMBOS).fill(0) };
const startNames: Record<string, number> = {};
let positions = 0, deep = 0, plies = 0;

function addAnalysis(state: State, flags: RuleFlags, combos: Record<Color, number>, label: string): void {
  const withDepth3 = positions % 6 === 0;
  jobs.push({ op: 'analyze', label, state: structuredClone(plain(state)), rules: flags, perft: withDepth3 ? [1, 2, 3] : [1, 2] });
  positions++;
  if (withDepth3) deep++;
  for (const color of COLORS) coverage[color][combos[color]]++;
}

/** Plays one random game and adds the game and the sampled positions to the jobs. */
function playGame(start: Start, combos: Record<Color, number>, label: string, sampleAll: boolean): void {
  const flags: RuleFlags = { w: flagsOf(combos.w), b: flagsOf(combos.b) };
  const state = createState(start.pieces, { w: ruleSet(flags.w), b: ruleSet(flags.b) });
  state.turn = start.turn;
  state.clock = start.clock;
  const playout: PlayoutJob = { op: 'playout', label, state: structuredClone(plain(state)), rules: flags, moves: [], expected: [] };

  const maxPlies = sampleAll ? 8 + rng.int(24) : 40 + rng.int(120);
  const captureBias = rng.next() * 0.7;
  for (let ply = 0; ; ply++) {
    const ended = outcome(state) !== null || ply === maxPlies;
    if (ended || sampleAll || rng.next() < (ply === 0 ? 0.3 : 0.08)) addAnalysis(state, flags, combos, `${label}, ply ${ply}`);
    if (ended) break;
    const moves = legalMoves(state);
    // Captures and promotions make the positions with few pieces and with promoted pieces.
    const sharp = moves.filter((m) => m.promo || m.epCapture || state.board[m.to]);
    const move = sharp.length && rng.next() < captureBias ? rng.pick(sharp) : rng.pick(moves);
    makeMove(state, move);
    playout.moves.push(move);
    playout.expected.push(stateKey(state));
    plies++;
  }
  jobs.push(playout);
}

const countStart = (name: string) => {
  const key = name.replace(/ floor \d/, '');
  startNames[key] = (startNames[key] ?? 0) + 1;
};

// Each block of 64 playouts gives each combination to each side one time. The pairs change from block to block.
const whiteOrder = rng.shuffle([...Array(COMBOS).keys()]);
for (let i = 0; i < PLAYOUTS; i++) {
  const block = Math.floor(i / COMBOS);
  const combos = { w: whiteOrder[i % COMBOS], b: (i * 5 + block * 13 + 7) % COMBOS };
  // The start changes with the block, thus each combination gets each kind of start.
  const start = makeStart(i + block, rng);
  countStart(start.name);
  playGame(start, combos, `seed ${SEED}, playout ${i}, start "${start.name}"`, false);
}

// Games with earlyPromo for Black, with an analysis of each position.
const EARLY_PROMO = 1 << FLAGS.indexOf('earlyPromo');
for (let i = 0; i < EP_PROMO_PLAYOUTS; i++) {
  const start = epPromoStart();
  countStart(start.name);
  playGame(start, { w: rng.int(COMBOS), b: rng.int(COMBOS) | EARLY_PROMO }, `seed ${SEED}, en passant game ${i}`, true);
}

// Each prepared ending with ordinary rules, and with three random rule combinations.
for (const ending of ENDINGS) {
  for (let i = 0; i < 4; i++) {
    const combos = i === 0 ? { w: 0, b: 0 } : { w: rng.int(COMBOS), b: rng.int(COMBOS) };
    const flags: RuleFlags = { w: flagsOf(combos.w), b: flagsOf(combos.b) };
    const state = createState(ending.pieces, { w: ruleSet(flags.w), b: ruleSet(flags.b) });
    state.turn = ending.turn;
    state.clock = ending.clock;
    countStart('ending');
    addAnalysis(state, flags, combos, `seed ${SEED}, ending "${ending.name}", rules ${JSON.stringify(flags)}`);
  }
}

// ---- Run the Rust tool and compare ----

const core = resolve(import.meta.dir, '..');
const build = Bun.spawnSync(['cargo', 'build', '--release', '--quiet', '-p', 'chrogue-tools', '--bin', 'difftest'], {
  cwd: core, stdout: 'inherit', stderr: 'inherit',
});
if (build.exitCode !== 0) throw new Error('cargo build failed');

const input = jobs
  .map((job) => JSON.stringify(job.op === 'analyze'
    ? { op: job.op, state: job.state, rules: job.rules, perft: job.perft }
    : { op: job.op, state: job.state, rules: job.rules, moves: job.moves }))
  .join('\n');
const rustStart = performance.now();
const tool = Bun.spawn([resolve(core, 'target/release/difftest')], { stdin: new Blob([input, '\n']), stdout: 'pipe', stderr: 'inherit' });
const output = await new Response(tool.stdout).text();
if ((await tool.exited) !== 0) throw new Error('The Rust tool stopped with an error');
const rustMs = performance.now() - rustStart;
const answers = output.split('\n').filter((line) => line.length > 0);
if (answers.length !== jobs.length) throw new Error(`The Rust tool gave ${answers.length} answers for ${jobs.length} requests`);

const tsStart = performance.now();
jobs.forEach((job, i) => {
  const answer = JSON.parse(answers[i]);
  if (answer.error) report(job, 'the Rust tool', answer.error);
  else if (job.op === 'analyze') compareAnalysis(job, answer);
  else comparePlayout(job, answer);
});
const tsMs = performance.now() - tsStart;

// ---- Report ----

/** The minimum count of each kind of move and of each result in the compared positions. */
// Seeds 1, 2, and 3 give two times these counts or more.
const MINIMUMS = {
  epCapture: 100, epPromo: 80, back: 2000, check: 200,
  promo: { q: 500, n: 500, r: 500, b: 500 },
  castle: { wK: 15, wQ: 15, bK: 15, bQ: 15 },
  outcome: { checkmate: 15, stalemate: 3, rout: 200, bare: 6, clock: 6 },
};

function minimumChecks(): [name: string, count: number, minimum: number][] {
  const m = MINIMUMS;
  return [
    ['an en passant capture', seen.epCapture, m.epCapture],
    ['an en passant capture with a promotion', seen.epPromo, m.epPromo],
    ['a backward step', seen.back, m.back],
    ['a check', seen.check, m.check],
    ...Object.entries(m.promo).map(([kind, min]): [string, number, number] => [`a promotion to ${kind}`, seen.promo[kind as keyof typeof m.promo], min]),
    ...Object.entries(m.castle).map(([wing, min]): [string, number, number] => [`a castle ${wing}`, seen.castle[wing as keyof typeof m.castle], min]),
    ...Object.entries(m.outcome).map(([reason, min]): [string, number, number] => [`the result ${reason}`, outcomes[reason] ?? 0, min]),
  ];
}

const minCoverage = Math.min(...coverage.w, ...coverage.b);
console.log(`seed ${SEED}: ${PLAYOUTS} playouts with ${plies} moves, from these starts: ${JSON.stringify(startNames)}`);
console.log(`positions compared: ${positions} (perft depth 2 on each, depth 3 on ${deep}), ${perftNodes} perft nodes`);
console.log(`rule combinations: ${COMBOS} for each side, each on ${minCoverage} positions or more`);
console.log(`legal moves compared: ${JSON.stringify(seen)}`);
console.log(`outcomes: ${JSON.stringify(outcomes)}`);
console.log(`time: Rust ${Math.round(rustMs)} ms, TypeScript ${Math.round(tsMs)} ms`);
console.log(`mismatches: ${mismatches}`);

const problems: string[] = [];
if (mismatches) problems.push(`${mismatches} mismatches`);
for (const [name, count, minimum] of minimumChecks()) {
  if (count < minimum) problems.push(`${name} occurred ${count} times, the minimum is ${minimum}`);
}
if (positions < MIN_POSITIONS) problems.push(`only ${positions} positions, the minimum is ${MIN_POSITIONS}`);
if (deep < MIN_DEEP) problems.push(`only ${deep} positions with perft depth 3, the minimum is ${MIN_DEEP}`);
if (minCoverage < MIN_PER_COMBO) problems.push(`a rule combination has only ${minCoverage} positions, the minimum is ${MIN_PER_COMBO}`);
if (problems.length) {
  console.error(`FAILED: ${problems.join('; ')}`);
  process.exit(1);
}
console.log('OK: the two engines agree.');
