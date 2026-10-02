// Permanent upgrades that crowns buy. To add an upgrade, add one entry to `defs`.
import { addUnit } from './army';
import type { Meta, Run } from './types';

export interface UpgradeDef {
  name: string;
  text: string;
  /** The crown cost of each level. The number of costs is the maximum level. */
  costs: readonly number[];
  /** Changes a new run. */
  startRun?(run: Run, level: number): void;
  /** Returns the factor that the upgrade applies to shop prices. */
  priceFactor?(level: number): number;
  /** In a battle, the player can select an enemy piece to see its moves. */
  scout?: true;
}

const defs = {
  pawn: {
    name: 'Militia',
    text: 'You start each run with one more pawn for each level.',
    costs: [3, 5, 8],
    startRun(run, level) {
      for (let i = 0; i < level; i++) addUnit(run, 'p');
    },
  },
  gold: {
    name: 'Treasury',
    text: 'You start each run with 5 more gold for each level.',
    costs: [2, 4, 6],
    startRun(run, level) {
      run.gold += 5 * level;
    },
  },
  bishop: {
    name: 'Chaplain',
    text: 'You start each run with a bishop.',
    costs: [6],
    startRun(run) {
      addUnit(run, 'b');
    },
  },
  haggle: {
    name: 'Haggler',
    text: 'Shop prices decrease by 10% for each level.',
    costs: [5, 8],
    priceFactor: (level) => 1 - 0.1 * level,
  },
  scout: {
    name: 'Scout',
    text: 'In a battle, select an enemy piece to see the squares that it can move to.',
    costs: [4],
    scout: true,
  },
} satisfies Record<string, UpgradeDef>;

/** The upgrades screen has a set slot for each upgrade. These are the limits of that screen. */
export const UPGRADE_SLOTS = 16;
export const UPGRADE_NAME_MAX = 13;

export type UpgradeId = keyof typeof defs;
export const UPGRADES: Record<UpgradeId, UpgradeDef> = defs;
export const UPGRADE_IDS = Object.keys(defs) as UpgradeId[];
export const isUpgradeId = (value: string): value is UpgradeId => Object.hasOwn(defs, value);

// The upgrades that the player has, with their levels.
export function ownedUpgrades(meta: Meta): { def: UpgradeDef; level: number }[] {
  return UPGRADE_IDS.flatMap((id) => {
    const level = meta.upgrades[id] ?? 0;
    return level > 0 ? [{ def: UPGRADES[id], level }] : [];
  });
}

export const canScout = (meta: Meta): boolean => ownedUpgrades(meta).some(({ def }) => def.scout);

// Returns the crown cost of the next level, or null at the maximum level.
export const nextCost = (meta: Meta, id: UpgradeId): number | null => UPGRADES[id].costs[meta.upgrades[id] ?? 0] ?? null;

// Returns false if the player cannot buy the next level.
export function buyUpgrade(meta: Meta, id: UpgradeId): boolean {
  const cost = nextCost(meta, id);
  if (cost === null || meta.crowns < cost) return false;
  meta.crowns -= cost;
  meta.upgrades[id] = (meta.upgrades[id] ?? 0) + 1;
  return true;
}
