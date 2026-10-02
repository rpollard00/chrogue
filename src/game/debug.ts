// Changes that the debug menu makes to the saved data. The game does not use them.
import { generateEnemy } from './floors';
import type { RelicId } from './relics';
import type { Meta, Run } from './types';
import { UPGRADES } from './upgrades';
import type { UpgradeId } from './upgrades';

// Sets the level of an upgrade at no cost. The level stays between 0 and the maximum level.
export function setUpgradeLevel(meta: Meta, id: UpgradeId, level: number): void {
  const next = Math.max(0, Math.min(level, UPGRADES[id].costs.length));
  if (next) meta.upgrades[id] = next;
  else delete meta.upgrades[id];
}

// Moves the run to a floor and makes the enemy of that floor.
export function setFloor(run: Run, floor: number): void {
  run.floor = floor;
  run.enemy = generateEnemy(floor);
}

// Returns false if the unit is the king or is not in the army.
export function removeUnit(run: Run, id: number): boolean {
  const unit = run.army.find((u) => u.id === id);
  if (!unit || unit.type === 'k') return false;
  run.army = run.army.filter((u) => u !== unit);
  return true;
}

// Adds a relic to a list of the run, or removes it: the relics of the player, or the traits of the enemy.
export function setRelic(list: RelicId[], id: RelicId, on: boolean): void {
  const at = list.indexOf(id);
  if (on && at < 0) list.push(id);
  else if (!on && at >= 0) list.splice(at, 1);
}
