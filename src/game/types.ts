// The saved data of the roguelite layer. These types have no behavior.
import type { PieceType, Square } from '../engine';
import type { RelicId } from './relics';
import type { UpgradeId } from './upgrades';

/** A piece type that the player can add to the army. */
export type RecruitType = Exclude<PieceType, 'k'>;

/** A piece of the player army. home is its start square on the first two ranks. */
export interface Unit {
  id: number;
  type: PieceType;
  home: Square;
}

export interface Enemy {
  pieces: { type: PieceType; square: Square }[];
  traits: RelicId[];
}

/** One item of a reward draft or of the shop. To add a kind, also add it to OFFER_KINDS in offers.ts. */
export type Offer =
  | { kind: 'piece'; type: RecruitType }
  | { kind: 'relic'; id: RelicId }
  | { kind: 'gold'; amount: number };

export interface Run {
  /** The floor of the current or the next battle. The first floor is 1. */
  floor: number;
  gold: number;
  army: Unit[];
  nextId: number;
  relics: RelicId[];
  enemy: Enemy;
  phase: 'battle' | 'camp';
  /** The reward choices. null when the player has no reward to take. */
  draft: Offer[] | null;
  shop: Offer[];
}

/** The data that stays from one run to the next run. */
export interface Meta {
  crowns: number;
  best: number;
  runs: number;
  upgrades: Partial<Record<UpgradeId, number>>;
}

export interface RunSummary {
  won: boolean;
  cleared: number;
  /** The crowns for the win of the run. */
  bonus: number;
  /** All the crowns of the run. */
  crowns: number;
  /** True if the run cleared more floors than each run before it. */
  newBest: boolean;
}
