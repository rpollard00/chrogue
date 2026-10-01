// One battle of a run. This module connects the chess engine to the run and its relics.
import { VALUE, chooseMove, createState, makeMove, outcome } from '../engine';
import type { Color, Move, Outcome, PieceSetup, PieceType, State } from '../engine';
import { FLOORS, floorOf } from './floors';
import { hooksOf, rulesFor } from './relics';
import { enterCamp } from './run';
import type { Run } from './types';

export interface BattleReward {
  captures: number;
  clear: number;
  /** Extra gold from relics. label is the relic name. */
  bonuses: { label: string; gold: number }[];
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
  for (const { name, hooks } of hooksOf(run.relics)) {
    const gold = hooks.victoryGold?.(run.gold + totalGold(reward)) ?? 0;
    if (gold > 0) reward.bonuses.push({ label: name, gold });
  }
  return reward;
}

export const totalGold = (reward: BattleReward): number =>
  reward.captures + reward.clear + reward.bonuses.reduce((sum, bonus) => sum + bonus.gold, 0);

// Plays a move for the side to move and records its effect on the run.
export function playMove(battle: Battle, run: Run, move: Move): void {
  const { state } = battle;
  const mover = state.turn;
  const { captured } = makeMove(state, move);
  if (captured) {
    battle.taken[mover].push(captured.type);
    const relics = hooksOf(run.relics);
    const unit = run.army.find((u) => u.id === captured.id);
    if (mover === 'w') {
      battle.gold += relics.reduce((gold, { hooks }) => hooks.captureGold?.(gold) ?? gold, VALUE[captured.type]);
    } else if (unit) {
      const rescued = relics.some(({ hooks }) => hooks.rescueUnit?.({ rescued: battle.rescued.length }));
      (rescued ? battle.rescued : battle.lost).push(unit.id);
    }
  }
  const result = outcome(state);
  if (result) battle.result = { ...result, reward: rewardFor(run, battle, result) };
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
