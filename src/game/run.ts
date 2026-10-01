// The life of a run: its start, the camp actions, and its end.
import { baseArmy } from './army';
import { FLOORS, generateEnemy } from './floors';
import { basePrice, rollDraft, rollShop, takeOffer } from './offers';
import type { Meta, Offer, Run, RunSummary } from './types';
import { ownedUpgrades } from './upgrades';

export const REROLL_COST = 3;
const WIN_CROWNS = 5;

export function newRun(meta: Meta): Run {
  const army = baseArmy();
  const run: Run = {
    floor: 1, gold: 0, army, nextId: army.length + 1, relics: [],
    enemy: generateEnemy(1), phase: 'battle', draft: null, shop: [],
  };
  for (const { def, level } of ownedUpgrades(meta)) def.startRun?.(run, level);
  return run;
}

// Moves the run to the camp before its next floor.
export function enterCamp(run: Run, withDraft: boolean): void {
  run.floor++;
  run.enemy = generateEnemy(run.floor);
  run.draft = withDraft ? rollDraft(run) : null;
  run.shop = rollShop(run);
  run.phase = 'camp';
}

export function takeDraft(run: Run, offer: Offer): boolean {
  if (!run.draft?.includes(offer) || !takeOffer(offer, run)) return false;
  run.draft = null;
  return true;
}

export function skipDraft(run: Run): void {
  run.draft = null;
}

export function priceOf(offer: Offer, meta: Meta): number {
  const factor = ownedUpgrades(meta).reduce((f, { def, level }) => f * (def.priceFactor?.(level) ?? 1), 1);
  return Math.max(1, Math.round(basePrice(offer) * factor));
}

export function buyOffer(run: Run, meta: Meta, offer: Offer): boolean {
  const cost = priceOf(offer, meta);
  if (!run.shop.includes(offer) || run.gold < cost || !takeOffer(offer, run)) return false;
  run.gold -= cost;
  run.shop = run.shop.filter((other) => other !== offer);
  return true;
}

export function rerollShop(run: Run): boolean {
  if (run.gold < REROLL_COST) return false;
  run.gold -= REROLL_COST;
  run.shop = rollShop(run);
  return true;
}

// Adds the result of a run to the permanent data.
export function finishRun(meta: Meta, run: Run, won: boolean): RunSummary {
  const cleared = won ? FLOORS.length : run.floor - 1;
  const bonus = won ? WIN_CROWNS : 0, crowns = cleared + bonus;
  const newBest = cleared > meta.best;
  meta.crowns += crowns;
  meta.best = Math.max(meta.best, cleared);
  meta.runs++;
  return { won, cleared, bonus, crowns, newBest };
}
