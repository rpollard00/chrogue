import { expect, test } from 'bun:test';
import { chooseMove, inCheck, legalMoves, makeMove, movesFrom, outcome, unmakeMove } from '../src/engine';
import type { State } from '../src/engine';
import { fromFen, sq } from './helpers';

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

const START = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR';
const KIWIPETE = 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R';
const targets = (state: State, from: string) =>
  legalMoves(state).filter((m) => m.from === sq(from)).map((m) => m.to).sort((a, b) => a - b);
const squares = (...names: string[]) => names.map(sq).sort((a, b) => a - b);

test('perft from the start position', () => {
  expect(perft(fromFen(START), 3)).toBe(8902);
  expect(perft(fromFen(START), 4)).toBe(197281);
});

test('perft with castling, en passant, and promotion (Kiwipete)', () => {
  expect(perft(fromFen(KIWIPETE), 2)).toBe(2039);
  expect(perft(fromFen(KIWIPETE), 3)).toBe(97862);
});

test('unmakeMove restores the state', () => {
  const state = fromFen(KIWIPETE);
  const before = JSON.stringify(state);
  perft(state, 3);
  expect(JSON.stringify(state)).toBe(before);
});

test('forcedMarch lets a moved pawn move two squares', () => {
  const fen = '4k3/7p/8/8/8/4P3/8/4K3';
  expect(targets(fromFen(fen), 'e3')).toEqual(squares('e4'));
  expect(targets(fromFen(fen, 'w', { w: { forcedMarch: true } }), 'e3')).toEqual(squares('e4', 'e5'));
});

test('backpedal lets a pawn move backward to an empty square', () => {
  const state = fromFen('4k3/7p/8/8/8/4P3/8/4K3', 'w', { w: { backpedal: true } });
  expect(targets(state, 'e3')).toEqual(squares('e2', 'e4'));
});

test('earlyPromo promotes on the seventh rank', () => {
  const state = fromFen('4k3/7p/4P3/8/8/8/8/4K3', 'w', { w: { earlyPromo: true } });
  const moves = legalMoves(state).filter((m) => m.from === sq('e6'));
  expect(moves).toHaveLength(4);
  expect(moves.every((m) => m.promo && m.to === sq('e7'))).toBe(true);
});

test('kingKnight lets the king move and give check as a knight', () => {
  const rules = { w: { kingKnight: true } };
  expect(targets(fromFen('4k3/7p/8/8/8/8/8/4K2P', 'w', rules), 'e1')).toContain(sq('f3'));
  expect(inCheck(fromFen('4k3/7p/3K4/8/8/8/8/7P', 'b', rules), 'b')).toBe(true);
});

test('longLeap adds the long knight jump', () => {
  const state = fromFen('4k3/7p/8/8/8/8/8/N3K3', 'w', { w: { longLeap: true } });
  expect(targets(state, 'a1')).toEqual(squares('b3', 'c2', 'b4', 'd2'));
});

test('sidestep moves a bishop one square without a capture', () => {
  const state = fromFen('4k3/8/8/8/8/p7/P7/B3K3', 'w', { w: { sidestep: true } });
  expect(targets(state, 'a1')).toContain(sq('b1'));
  expect(targets(state, 'a1')).not.toContain(sq('a2'));
});

test('movesFrom gives the moves of a piece of the side that does not have the move', () => {
  const to = (state: State, from: string) => movesFrom(state, sq(from)).map((m) => m.to).sort((a, b) => a - b);
  const leap = fromFen('4k3/8/8/3n4/8/8/8/4K3', 'w', { b: { longLeap: true } });
  expect(to(leap, 'd5')).toHaveLength(15);
  expect(to(leap, 'd5')).toContain(sq('a4'));
  expect(to(leap, 'a1')).toEqual([]);

  // The rook on e7 is pinned to its king, thus it stays on the e-file.
  expect(to(fromFen('4k3/4r3/8/8/8/8/8/4RK2'), 'e7')).toEqual(squares('e6', 'e5', 'e4', 'e3', 'e2', 'e1'));

  const state = fromFen('4k3/8/8/3pP3/8/8/8/4K3');
  state.ep = sq('d6');
  expect(to(state, 'd5')).toEqual(squares('d4'));
  expect(state).toMatchObject({ turn: 'w', ep: sq('d6') });
  expect(to(state, 'e5')).toEqual(squares('d6', 'e6'));
});

test('outcome finds checkmate, stalemate, and a lone king', () => {
  expect(outcome(fromFen('R5k1/5ppp/8/8/8/8/8/4K3', 'b'))).toEqual({ winner: 'w', reason: 'checkmate' });
  expect(outcome(fromFen('7k/5Q2/8/8/8/8/8/4K2p', 'b'))).toEqual({ winner: 'w', reason: 'stalemate' });
  expect(outcome(fromFen('7k/8/8/8/8/8/8/4K2P', 'b'))).toEqual({ winner: 'w', reason: 'rout' });
  expect(outcome(fromFen(START))).toBeNull();
});

test('the AI captures a free queen and finds mate in one', () => {
  const level = { depth: 2, noise: 0 };
  expect(chooseMove(fromFen('4k3/8/8/3q4/8/4N3/PPP5/4K3'), level)?.to).toBe(sq('d5'));
  expect(chooseMove(fromFen('6k1/5ppp/8/8/8/8/1P6/R3K3'), level)?.to).toBe(sq('a8'));
});

test('the AI answers in a full position at depth 3 in less than 3 seconds', () => {
  const state = fromFen('r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b');
  const start = performance.now();
  expect(chooseMove(state, { depth: 3, noise: 0 })).not.toBeNull();
  expect(performance.now() - start).toBeLessThan(3000);
});
