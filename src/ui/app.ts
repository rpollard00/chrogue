// The contract between the screens and the application shell in main.ts.
import type { Move, Square } from '../engine';
import type { Battle, Meta, RelicId, Run, RunSummary, UpgradeId } from '../game';

/** A battle and the selection state of its board. */
export interface BattleView {
  battle: Battle;
  selected: Square;
  /** The legal moves of the selected piece. */
  targets: Move[];
  /** The promotion moves that wait for a choice of piece. */
  promotion: Move[] | null;
  last: Move | null;
  /** True while the enemy selects a move. */
  busy: boolean;
}

/** The cause of the last change of the camp screen. The camp selects its motion from it. */
export type CampCue =
  | { kind: 'enter' }
  // units and relics are the ids that the action added. rolled is true when the shop has new items.
  | { kind: 'acted'; goldBefore: number; units: number[]; relics: RelicId[]; rolled: boolean };

/** The current screen and the data that only this screen uses. */
export type Screen =
  | { name: 'title' }
  | { name: 'help' }
  // bought is the upgrade that the player bought last on this screen.
  | { name: 'upgrades'; bought: UpgradeId | null }
  | { name: 'battle'; run: Run; view: BattleView }
  | { name: 'camp'; run: Run; selected: Square; cue: CampCue | null }
  | { name: 'over'; summary: RunSummary };

export type ScreenOf<N extends Screen['name']> = Extract<Screen, { name: N }>;

export interface App {
  readonly meta: Meta;
  /** The run in progress, or null. */
  readonly run: Run | null;
  readonly screen: Screen;
  show(screen: Screen): void;
  render(): void;
  startRun(): void;
  startBattle(run: Run): void;
  openCamp(run: Run): void;
  endRun(run: Run, won: boolean): void;
  saveRun(): void;
  saveMeta(): void;
}
