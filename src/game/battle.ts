// One battle of a run. This module connects the chess engine to the run and its relics.
import { VALUE, chooseMove, createState, makeMove, outcome } from '../engine';
import type { Color, Move, Outcome, PieceSetup, PieceType, Square, State } from '../engine';
import { FLOORS, floorOf } from './floors';
import { hooksOf, rulesFor } from './relics';
import type { RelicId } from './relics';
import { enterCamp } from './run';
import type { Run } from './types';

export interface BattleReward {
  captures: number;
  clear: number;
  /** Extra gold from relics. label is the relic name. */
  bonuses: { id: RelicId; label: string; gold: number }[];
}

export type BattleResult = Outcome & { reward: BattleReward };

export interface Battle {
  state: State;
  /** The ids of the units that the enemy captured. */
  lost: number[];
  /** The ids of the captured units that return after the battle. */
  rescued: number[];
  /** The gold from captures. It can have a fraction until the battle ends. */
  gold: number;
  /** The piece types that each side captured. */
  taken: Record<Color, PieceType[]>;
  result: BattleResult | null;
}

/** The effects of one move on the run. */
export interface MoveReport {
  /** The square of the captured piece and the gold that the capture gave, or null. */
  capture: { square: Square; gold: number } | null;
  /** The relics of the player that had an effect. */
  relics: RelicId[];
}

export function createBattle(run: Run): Battle {
  const pieces: PieceSetup[] = run.army.map((unit) => ({ id: unit.id, type: unit.type, color: 'w', square: unit.home }));
  for (const { hooks } of hooksOf(run.relics)) hooks.setupBattle?.(pieces);
  run.enemy.pieces.forEach((e, i) => pieces.push({ id: `e${i}`, type: e.type, color: 'b', square: e.square }));
  return {
    state: createState(pieces, { w: rulesFor(run.relics), b: rulesFor(run.enemy.traits) }),
    lost: [], rescued: [], gold: 0, taken: { w: [], b: [] }, result: null,
  };
}

function rewardFor(run: Run, battle: Battle, result: Outcome): BattleReward {
  const reward: BattleReward = { captures: 0, clear: 0, bonuses: [] };
  if (result.winner === 'b') return reward;
  reward.captures = Math.round(battle.gold);
  if (result.winner === null) return reward;
  reward.clear = 3 + run.floor;
  for (const { id, name, hooks } of hooksOf(run.relics)) {
    const gold = hooks.victoryGold?.(run.gold + totalGold(reward)) ?? 0;
    if (gold > 0) reward.bonuses.push({ id, label: name, gold });
  }
  return reward;
}

export const totalGold = (reward: BattleReward): number =>
  reward.captures + reward.clear + reward.bonuses.reduce((sum, bonus) => sum + bonus.gold, 0);

// Plays a move for the side to move and records its effect on the run.
export function playMove(battle: Battle, run: Run, move: Move): MoveReport {
  const { state } = battle;
  const mover = state.turn;
  const { captured, capSq } = makeMove(state, move);
  const report: MoveReport = { capture: captured && { square: capSq, gold: 0 }, relics: [] };
  if (captured && report.capture) {
    battle.taken[mover].push(captured.type);
    const relics = hooksOf(run.relics);
    const unit = run.army.find((u) => u.id === captured.id);
    if (mover === 'w') {
      let gold = VALUE[captured.type];
      for (const { id, hooks } of relics) {
        const next = hooks.captureGold?.(gold) ?? gold;
        if (next !== gold) report.relics.push(id);
        gold = next;
      }
      report.capture.gold = gold;
      battle.gold += gold;
    } else if (unit) {
      const rescuer = relics.find(({ hooks }) => hooks.rescueUnit?.({ rescued: battle.rescued.length }));
      if (rescuer) report.relics.push(rescuer.id);
      (rescuer ? battle.rescued : battle.lost).push(unit.id);
    }
  }
  const result = outcome(state);
  if (result) {
    const reward = rewardFor(run, battle, result);
    battle.result = { ...result, reward };
    report.relics.push(...reward.bonuses.map((bonus) => bonus.id));
  }
  return report;
}

export function enemyMove(battle: Battle, run: Run): Move {
  const move = chooseMove(battle.state, floorOf(run).ai);
  if (!move) throw new Error('The enemy has no legal move');
  return move;
}

// Applies a completed battle to the run. Returns what comes next.
export function settleBattle(run: Run, battle: Battle): 'camp' | 'won' | 'lost' {
  const { result, state } = battle;
  if (!result) throw new Error('The battle has no result');
  if (result.winner === 'b') return 'lost';
  run.gold += totalGold(result.reward);
  run.army = run.army.filter((unit) => !battle.lost.includes(unit.id));
  // A unit keeps the type that it has on the board, thus a promoted pawn stays promoted.
  for (const p of state.board) {
    const unit = p && run.army.find((u) => u.id === p.id);
    if (unit) unit.type = p.type;
  }
  if (result.winner === 'w' && run.floor === FLOORS.length) return 'won';
  enterCamp(run, result.winner === 'w');
  return 'camp';
}
