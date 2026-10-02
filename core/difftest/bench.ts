// Measures the perft speed of the TypeScript engine, for a comparison with the Rust tool `perft`.
//
// Usage: bun core/difftest/bench.ts [DEPTH] [FEN_PIECE_FIELD]
import { legalMoves, makeMove, unmakeMove } from '../../src/engine';
import type { State } from '../../src/engine';
import { fromFen } from '../../test/helpers';

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

const depth = Number(process.argv[2] ?? 5);
const fen = process.argv[3] ?? 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR';
const state = fromFen(fen);
// The first run lets the JIT compile the engine.
perft(state, Math.min(depth, 3));
const start = performance.now();
const nodes = perft(state, depth);
const seconds = (performance.now() - start) / 1000;
console.log(`depth ${depth}: ${nodes} nodes in ${seconds.toFixed(3)} s, ${(nodes / seconds / 1e6).toFixed(1)} million nodes per second`);
