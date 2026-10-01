// Relic definitions. To add a relic, add one entry to `defs`.
import type { PieceSetup, RuleSet } from '../engine';

/** The points where a relic can change a battle. Add a hook here when a new relic needs one. */
export interface RelicHooks {
  /** Changes the pieces before a battle starts. */
  setupBattle?(pieces: PieceSetup[]): void;
  /** Returns the gold for one capture. `gold` is the amount before this relic. */
  captureGold?(gold: number): number;
  /** Returns true if a captured unit returns to the army after the battle. */
  rescueUnit?(battle: { rescued: number }): boolean;
  /** Returns the extra gold after a win. `gold` is the gold of the run plus the other rewards. */
  victoryGold?(gold: number): number;
}

export interface RelicDef {
  name: string;
  text: string;
  /** The text when the enemy has the relic. A relic with this text can be a boss trait. */
  foeText?: string;
  /** Movement rules that the relic adds for its side. */
  rules?: RuleSet;
  hooks?: RelicHooks;
}

/** The id of the pawn that Conscription adds. It is not a unit of the army. */
export const CONSCRIPT_ID = 'conscript';

const defs = {
  forcedMarch: {
    name: 'Forced March',
    text: 'Your pawns can always move two squares forward.',
    foeText: 'Enemy pawns can always move two squares forward.',
    rules: { forcedMarch: true },
  },
  backpedal: {
    name: 'Tactical Retreat',
    text: 'Your pawns can move one square backward to an empty square.',
    foeText: 'Enemy pawns can move one square backward to an empty square.',
    rules: { backpedal: true },
  },
  earlyPromo: {
    name: 'Field Promotion',
    text: 'Your pawns promote one rank earlier.',
    foeText: 'Enemy pawns promote one rank earlier.',
    rules: { earlyPromo: true },
  },
  kingKnight: {
    name: 'Royal Steed',
    text: 'Your king can also move as a knight.',
    foeText: 'The enemy king can also move as a knight.',
    rules: { kingKnight: true },
  },
  longLeap: {
    name: 'Long Leap',
    text: 'Your knights can also jump three squares in one direction and one square to the side.',
    foeText: 'Enemy knights can also jump three squares in one direction and one square to the side.',
    rules: { longLeap: true },
  },
  sidestep: {
    name: 'Sidestep',
    text: 'Your bishops can move one square up, down, left, or right to an empty square.',
    foeText: 'Enemy bishops can move one square up, down, left, or right to an empty square.',
    rules: { sidestep: true },
  },
  bounty: {
    name: 'Bounty',
    text: 'You get 50% more gold for each capture.',
    hooks: { captureGold: (gold) => gold * 1.5 },
  },
  secondWind: {
    name: 'Second Wind',
    text: 'The first piece that you lose in each battle returns after the battle.',
    hooks: { rescueUnit: (battle) => battle.rescued === 0 },
  },
  conscription: {
    name: 'Conscription',
    text: 'You start each battle with one more pawn. The pawn leaves after the battle.',
    hooks: {
      setupBattle(pieces) {
        // The pawn goes to the first free square of rank 2, or of rank 3 if rank 2 is full.
        for (let square = 8; square < 24; square++) {
          if (pieces.some((p) => p.square === square)) continue;
          pieces.push({ id: CONSCRIPT_ID, type: 'p', color: 'w', square });
          return;
        }
      },
    },
  },
  interest: {
    name: 'Interest',
    text: 'After each battle that you win, you get 1 gold for each 5 gold that you have. The maximum is 6 gold.',
    hooks: { victoryGold: (gold) => Math.min(6, Math.floor(gold / 5)) },
  },
} satisfies Record<string, RelicDef>;

export type RelicId = keyof typeof defs;
export const RELICS: Record<RelicId, RelicDef> = defs;
export const RELIC_IDS = Object.keys(defs) as RelicId[];
export const TRAIT_IDS = RELIC_IDS.filter((id) => RELICS[id].foeText);
export const isRelicId = (value: unknown): value is RelicId => typeof value === 'string' && Object.hasOwn(defs, value);

// The movement rules that a list of relics gives to one side.
export const rulesFor = (ids: readonly RelicId[]): RuleSet => Object.assign({}, ...ids.map((id) => RELICS[id].rules));

// The relics of a list that have hooks, with their names.
export function hooksOf(ids: readonly RelicId[]): { id: RelicId; name: string; hooks: RelicHooks }[] {
  return ids.flatMap((id) => {
    const { name, hooks } = RELICS[id];
    return hooks ? [{ id, name, hooks }] : [];
  });
}
