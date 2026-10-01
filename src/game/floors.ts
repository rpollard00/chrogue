// The floors of a run and the enemy army of each floor.
import { VALUE } from '../engine';
import type { AiLevel, Square } from '../engine';
import { pickWeighted, shuffle } from './random';
import { TRAIT_IDS } from './relics';
import type { Enemy, RecruitType, Run } from './types';

export interface Floor {
  name: string;
  /** The total piece value of the enemy army. */
  budget: number;
  ai: AiLevel;
  /** The number of boss traits. */
  traits: number;
  boss?: true;
}

export const FLOORS: readonly Floor[] = [
  { name: 'Border Patrol', budget: 5, ai: { depth: 1, noise: 60 }, traits: 0 },
  { name: 'Scouts', budget: 9, ai: { depth: 1, noise: 40 }, traits: 0 },
  { name: 'Garrison', budget: 13, ai: { depth: 2, noise: 40 }, traits: 0 },
  { name: 'The Warden', budget: 18, ai: { depth: 2, noise: 20 }, traits: 1, boss: true },
  { name: 'Cavalry', budget: 23, ai: { depth: 2, noise: 20 }, traits: 0 },
  { name: 'Royal Guard', budget: 28, ai: { depth: 2, noise: 10 }, traits: 0 },
  { name: 'Vanguard', budget: 33, ai: { depth: 3, noise: 10 }, traits: 0 },
  { name: 'The Black King', budget: 39, ai: { depth: 3, noise: 0 }, traits: 2, boss: true },
];

export const floorOf = (run: Run): Floor => FLOORS[run.floor - 1];

const TYPES: RecruitType[] = ['p', 'n', 'b', 'r', 'q'];
const WEIGHT: Record<RecruitType, number> = { p: 4, n: 2, b: 2, r: 1.5, q: 1 };

export function generateEnemy(floor: number): Enemy {
  const spec = FLOORS[floor - 1];
  const caps: Record<RecruitType, number> = { p: 8, n: 2, b: 2, r: 2, q: floor >= 5 ? 1 : 0 };
  const counts: Record<RecruitType, number> = { p: 0, n: 0, b: 0, r: 0, q: 0 };
  let budget = spec.budget;
  for (;;) {
    const pool = TYPES.filter((t) => counts[t] < caps[t] && VALUE[t] <= budget).map((t) => ({ item: t, weight: WEIGHT[t] }));
    const [type] = pickWeighted(pool, 1);
    if (!type) break;
    counts[type]++;
    budget -= VALUE[type];
  }
  const squares: Record<RecruitType, Square[]> = {
    r: shuffle([56, 63]), n: shuffle([57, 62]), b: shuffle([58, 61]), q: [59],
    p: [52, 51, 53, 50, 54, 49, 55, 48],
  };
  const pieces: Enemy['pieces'] = [{ type: 'k', square: 60 }];
  for (const type of TYPES) {
    for (let i = 0; i < counts[type]; i++) pieces.push({ type, square: squares[type][i] });
  }
  return { pieces, traits: shuffle(TRAIT_IDS).slice(0, spec.traits) };
}
