import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createState, legalMoves, makeMove, unmakeMove, outcome, chooseMove, inCheck } from '../src/engine.js';
import { generateEnemy, startingArmy, FLOORS } from '../src/content.js';

const sq = (name) => 'abcdefgh'.indexOf(name[0]) + 8 * (Number(name[1]) - 1);

// Reads the piece field of a FEN string. A pawn off its start rank counts as moved.
function fromFen(fen, turn = 'w', mods) {
  const pieces = [];
  fen.split('/').forEach((row, i) => {
    let f = 0;
    for (const ch of row) {
      if (ch >= '1' && ch <= '8') {
        f += Number(ch);
        continue;
      }
      const color = ch === ch.toUpperCase() ? 'w' : 'b', type = ch.toLowerCase(), r = 7 - i;
      const moved = type === 'p' && r !== (color === 'w' ? 1 : 6);
      pieces.push({ id: pieces.length, type, color, square: r * 8 + f++, moved });
    }
  });
  const state = createState(pieces, mods);
  state.turn = turn;
  return state;
}

function perft(state, depth) {
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
const targets = (state, from) => legalMoves(state).filter((m) => m.from === sq(from)).map((m) => m.to).sort();
const squares = (...names) => names.map(sq).sort();

test('perft from the start position', () => {
  assert.equal(perft(fromFen(START), 3), 8902);
  assert.equal(perft(fromFen(START), 4), 197281);
});

test('perft with castling, en passant, and promotion (Kiwipete)', () => {
  const fen = 'r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R';
  assert.equal(perft(fromFen(fen), 2), 2039);
  assert.equal(perft(fromFen(fen), 3), 97862);
});

test('unmakeMove restores the state', () => {
  const state = fromFen('r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R');
  const before = JSON.stringify(state);
  perft(state, 3);
  assert.equal(JSON.stringify(state), before);
});

test('forcedMarch lets a moved pawn move two squares', () => {
  const fen = '4k3/7p/8/8/8/4P3/8/4K3';
  assert.deepEqual(targets(fromFen(fen), 'e3'), squares('e4'));
  assert.deepEqual(targets(fromFen(fen, 'w', { w: { forcedMarch: true } }), 'e3'), squares('e4', 'e5'));
});

test('backpedal lets a pawn move backward to an empty square', () => {
  const state = fromFen('4k3/7p/8/8/8/4P3/8/4K3', 'w', { w: { backpedal: true } });
  assert.deepEqual(targets(state, 'e3'), squares('e2', 'e4'));
});

test('earlyPromo promotes on the seventh rank', () => {
  const state = fromFen('4k3/7p/4P3/8/8/8/8/4K3', 'w', { w: { earlyPromo: true } });
  const moves = legalMoves(state).filter((m) => m.from === sq('e6'));
  assert.equal(moves.length, 4);
  assert.ok(moves.every((m) => m.promo && m.to === sq('e7')));
});

test('kingKnight lets the king move and give check as a knight', () => {
  const mods = { w: { kingKnight: true } };
  assert.ok(targets(fromFen('4k3/7p/8/8/8/8/8/4K2P', 'w', mods), 'e1').includes(sq('f3')));
  assert.ok(inCheck(fromFen('4k3/7p/3K4/8/8/8/8/7P', 'b', mods), 'b'));
});

test('longLeap adds the long knight jump', () => {
  const state = fromFen('4k3/7p/8/8/8/8/8/N3K3', 'w', { w: { longLeap: true } });
  assert.deepEqual(targets(state, 'a1'), squares('b3', 'c2', 'b4', 'd2'));
});

test('sidestep moves a bishop one square without a capture', () => {
  const state = fromFen('4k3/8/8/8/8/p7/P7/B3K3', 'w', { w: { sidestep: true } });
  assert.ok(targets(state, 'a1').includes(sq('b1')));
  assert.ok(!targets(state, 'a1').includes(sq('a2')));
});

test('outcome finds checkmate, stalemate, and a lone king', () => {
  assert.deepEqual(outcome(fromFen('R5k1/5ppp/8/8/8/8/8/4K3', 'b')), { winner: 'w', reason: 'checkmate' });
  assert.deepEqual(outcome(fromFen('7k/5Q2/8/8/8/8/8/4K2p', 'b')), { winner: 'w', reason: 'stalemate' });
  assert.deepEqual(outcome(fromFen('7k/8/8/8/8/8/8/4K2P', 'b')), { winner: 'w', reason: 'rout' });
  assert.equal(outcome(fromFen(START)), null);
});

test('the AI captures a free queen and finds mate in one', () => {
  const capture = chooseMove(fromFen('4k3/8/8/3q4/8/4N3/PPP5/4K3'), { depth: 2 });
  assert.equal(capture.to, sq('d5'));
  const mate = chooseMove(fromFen('6k1/5ppp/8/8/8/8/1P6/R3K3'), { depth: 2 });
  assert.equal(mate.to, sq('a8'));
});

test('each floor makes an enemy army that uses the budget', () => {
  const value = { p: 1, n: 3, b: 3, r: 5, q: 9, k: 0 };
  FLOORS.forEach((spec, i) => {
    for (let n = 0; n < 50; n++) {
      const { pieces, traits } = generateEnemy(i + 1);
      // The piece limits can leave up to 4 points of the budget.
      const total = pieces.reduce((sum, p) => sum + value[p.type], 0);
      assert.ok(total <= spec.budget && total >= spec.budget - 4);
      assert.equal(new Set(pieces.map((p) => p.square)).size, pieces.length);
      assert.equal(traits.length, spec.traits);
    }
  });
});

test('the starting army uses different home squares', () => {
  const army = startingArmy({ pawn: 3, bishop: 1 });
  assert.equal(army.length, 11);
  assert.equal(new Set(army.map((a) => a.home)).size, 11);
});

test('the AI answers in a full position at depth 3 in less than 3 seconds', () => {
  const state = fromFen('r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R', 'b');
  const start = performance.now();
  assert.ok(chooseMove(state, { depth: 3 }));
  assert.ok(performance.now() - start < 3000);
});
