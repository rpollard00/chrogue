// The player army and its home squares.
import type { PieceType, Square } from '../engine';
import type { RecruitType, Run, Unit } from './types';

export const ARMY_MAX = 16;
export const PIECE_NAME: Record<PieceType, string> = {
  k: 'King', q: 'Queen', r: 'Rook', b: 'Bishop', n: 'Knight', p: 'Pawn',
};

// Home squares on the first two ranks, from the center to the edge.
const BACK_HOMES: Square[] = [3, 2, 5, 1, 6, 0, 7, 4];
const FRONT_HOMES: Square[] = [12, 11, 13, 10, 14, 9, 15, 8];

export function baseArmy(): Unit[] {
  const army: Omit<Unit, 'id'>[] = [
    { type: 'k', home: 4 }, { type: 'r', home: 0 }, { type: 'n', home: 6 },
    { type: 'p', home: 10 }, { type: 'p', home: 11 }, { type: 'p', home: 12 }, { type: 'p', home: 13 },
  ];
  return army.map((unit, i) => ({ id: i + 1, ...unit }));
}

// Returns a free home square for a new piece, or -1 if the first two ranks are full.
export function freeHome(army: readonly Unit[], type: PieceType): Square {
  const order = type === 'p' ? [...FRONT_HOMES, ...BACK_HOMES] : [...BACK_HOMES, ...FRONT_HOMES];
  return order.find((s) => !army.some((unit) => unit.home === s)) ?? -1;
}

export const canAddUnit = (run: Run): boolean => run.army.length < ARMY_MAX;

// Returns false if the army is full.
export function addUnit(run: Run, type: RecruitType): boolean {
  if (!canAddUnit(run)) return false;
  run.army.push({ id: run.nextId++, type, home: freeHome(run.army, type) });
  return true;
}

// Moves the unit on one home square to a second home square. If the second square has a unit, the two units swap.
export function moveUnit(run: Run, from: Square, to: Square): void {
  const unit = run.army.find((u) => u.home === from);
  if (!unit) return;
  const occupant = run.army.find((u) => u.home === to);
  if (occupant) occupant.home = from;
  unit.home = to;
}
