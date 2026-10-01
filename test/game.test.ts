import { expect, test } from 'bun:test';
import { VALUE, legalMoves } from '../src/engine';
import type { Move } from '../src/engine';
import {
  CONSCRIPT_ID, FLOORS, RELIC_IDS, TRAIT_IDS,
  buyOffer, buyUpgrade, createBattle, finishRun, generateEnemy, newRun, parseMeta, parseRun,
  playMove, priceOf, rollDraft, rollShop, settleBattle, takeDraft, takeOffer,
} from '../src/game';
import type { Battle, Enemy, Meta, MoveReport, Run } from '../src/game';
import { sq } from './helpers';

const emptyMeta = (): Meta => ({ crowns: 0, best: 0, runs: 0, upgrades: {} });

// A run against an enemy that the test selects. The army is: Ke1, Ra1, Ng1, and pawns on c2, d2, e2, f2.
function runAgainst(pieces: Enemy['pieces'], change: (run: Run) => void = () => {}): Run {
  const run = newRun(emptyMeta());
  run.enemy = { pieces, traits: [] };
  change(run);
  return run;
}

function play(battle: Battle, run: Run, from: string, to: string): MoveReport {
  const move = legalMoves(battle.state).find((m: Move) => m.from === sq(from) && m.to === sq(to));
  if (!move) throw new Error(`${from}-${to} is not a legal move`);
  return playMove(battle, run, move);
}

const KING = { type: 'k', square: sq('e8') } as const;

test('each floor makes an enemy army that uses the budget', () => {
  FLOORS.forEach((spec, i) => {
    for (let n = 0; n < 50; n++) {
      const { pieces, traits } = generateEnemy(i + 1);
      // The piece limits can leave up to 4 points of the budget.
      const total = pieces.reduce((sum, p) => sum + VALUE[p.type], 0);
      expect(total).toBeLessThanOrEqual(spec.budget);
      expect(total).toBeGreaterThanOrEqual(spec.budget - 4);
      expect(new Set(pieces.map((p) => p.square)).size).toBe(pieces.length);
      expect(traits).toHaveLength(spec.traits);
      expect(traits.every((id) => TRAIT_IDS.includes(id))).toBe(true);
    }
  });
});

test('upgrades change a new run', () => {
  const run = newRun({ ...emptyMeta(), upgrades: { pawn: 3, bishop: 1, gold: 2 } });
  expect(run.army).toHaveLength(11);
  expect(new Set(run.army.map((u) => u.home)).size).toBe(11);
  expect(run.army.filter((u) => u.type === 'b')).toHaveLength(1);
  expect(run.gold).toBe(10);
});

test('buyUpgrade takes crowns and stops at the maximum level', () => {
  const meta = { ...emptyMeta(), crowns: 10 };
  expect(buyUpgrade(meta, 'bishop')).toBe(true);
  expect(meta).toMatchObject({ crowns: 4, upgrades: { bishop: 1 } });
  expect(buyUpgrade(meta, 'bishop')).toBe(false);
  expect(buyUpgrade(meta, 'pawn')).toBe(true);
  expect(buyUpgrade(meta, 'pawn')).toBe(false);
  expect(meta.crowns).toBe(1);
});

test('a win gives gold, keeps the army, and opens the camp', () => {
  const run = runAgainst([KING, { type: 'p', square: sq('a2') }]);
  const battle = createBattle(run);
  play(battle, run, 'a1', 'a2');
  expect(battle.result).toMatchObject({ winner: 'w', reason: 'rout', reward: { captures: 1, clear: 4, bonuses: [] } });
  expect(settleBattle(run, battle)).toBe('camp');
  expect(run).toMatchObject({ floor: 2, gold: 5, phase: 'camp' });
  expect(run.army).toHaveLength(7);
  expect(run.draft).toHaveLength(3);
});

test('Bounty and Interest add gold', () => {
  const run = runAgainst([KING, { type: 'r', square: sq('a2') }], (r) => {
    r.relics = ['bounty', 'interest'];
    r.gold = 20;
  });
  const battle = createBattle(run);
  expect(play(battle, run, 'a1', 'a2')).toEqual({ capture: { square: sq('a2'), gold: 7.5 }, relics: ['bounty', 'interest'] });
  // Captures: 5 * 1.5 = 7.5, which rounds to 8. Interest: floor((20 + 8 + 4) / 5) = 6.
  expect(battle.result?.reward).toEqual({ captures: 8, clear: 4, bonuses: [{ id: 'interest', label: 'Interest', gold: 6 }] });
});

test('a captured unit leaves the army, and Second Wind returns the first one', () => {
  const enemy: Enemy['pieces'] = [KING, { type: 'r', square: sq('a8') }, { type: 'r', square: sq('h8') }];
  const lose = (run: Run) => {
    const battle = createBattle(run);
    play(battle, run, 'e2', 'e3');
    play(battle, run, 'a8', 'a1');
    // The rook on a1 gives check, thus the king moves.
    play(battle, run, 'e1', 'e2');
    play(battle, run, 'a1', 'g1');
    return battle;
  };
  const plain = runAgainst(enemy);
  expect(lose(plain)).toMatchObject({ lost: [2, 3], rescued: [] });

  const run = runAgainst(enemy, (r) => (r.relics = ['secondWind']));
  const battle = lose(run);
  expect(battle).toMatchObject({ lost: [3], rescued: [2] });
  battle.result = { winner: null, reason: 'clock', reward: { captures: 0, clear: 0, bonuses: [] } };
  expect(settleBattle(run, battle)).toBe('camp');
  expect(run.army.map((u) => u.id)).toEqual([1, 2, 4, 5, 6, 7]);
  expect(run.draft).toBeNull();
});

test('Conscription adds a pawn that does not join the army', () => {
  const run = runAgainst([KING, { type: 'p', square: sq('a7') }], (r) => (r.relics = ['conscription']));
  const battle = createBattle(run);
  const conscript = battle.state.board.filter((p) => p?.id === CONSCRIPT_ID);
  expect(conscript).toHaveLength(1);
  expect(battle.state.board[sq('a2')]).toMatchObject({ id: CONSCRIPT_ID, type: 'p', color: 'w' });
  expect(run.army).toHaveLength(7);
});

test('a relic gives its movement rule to the battle', () => {
  const run = runAgainst([KING, { type: 'p', square: sq('a7') }], (r) => (r.relics = ['kingKnight']));
  run.enemy.traits = ['forcedMarch'];
  expect(createBattle(run).state.rules).toEqual({ w: { kingKnight: true }, b: { forcedMarch: true } });
});

test('a promoted pawn stays promoted after the battle', () => {
  const run = runAgainst([KING, { type: 'p', square: sq('h7') }, { type: 'p', square: sq('a7') }]);
  run.army = run.army.filter((u) => u.type !== 'p' || u.home === sq('e2'));
  const battle = createBattle(run);
  const pawn = battle.state.board[sq('e2')];
  battle.state.board[sq('e2')] = null;
  battle.state.board[sq('c7')] = pawn;
  const promote = legalMoves(battle.state).find((m) => m.to === sq('c8') && m.promo === 'q');
  if (!promote) throw new Error('No promotion move');
  playMove(battle, run, promote);
  battle.result = { winner: null, reason: 'clock', reward: { captures: 0, clear: 0, bonuses: [] } };
  settleBattle(run, battle);
  expect(run.army.map((u) => u.type).sort()).toEqual(['k', 'n', 'q', 'r']);
});

test('the last floor ends the run with a win', () => {
  const run = runAgainst([KING, { type: 'p', square: sq('a2') }], (r) => (r.floor = FLOORS.length));
  const battle = createBattle(run);
  play(battle, run, 'a1', 'a2');
  expect(settleBattle(run, battle)).toBe('won');
  const meta = emptyMeta();
  expect(finishRun(meta, run, true)).toEqual({ won: true, cleared: 8, bonus: 5, crowns: 13 });
  expect(meta).toMatchObject({ crowns: 13, best: 8, runs: 1 });
});

test('offers change the run, and a blocked offer does nothing', () => {
  const run = newRun(emptyMeta());
  expect(takeOffer({ kind: 'gold', amount: 12 }, run)).toBe(true);
  expect(takeOffer({ kind: 'relic', id: 'bounty' }, run)).toBe(true);
  expect(takeOffer({ kind: 'relic', id: 'bounty' }, run)).toBe(false);
  expect(run).toMatchObject({ gold: 12, relics: ['bounty'] });
  while (run.army.length < 16) expect(takeOffer({ kind: 'piece', type: 'p' }, run)).toBe(true);
  expect(takeOffer({ kind: 'piece', type: 'n' }, run)).toBe(false);
  expect(new Set(run.army.map((u) => u.home)).size).toBe(16);
});

test('the draft and the shop do not offer a relic that the run has', () => {
  const run = newRun(emptyMeta());
  run.relics = RELIC_IDS.slice(1);
  for (let n = 0; n < 50; n++) {
    for (const offer of [...rollDraft(run), ...rollShop(run)]) {
      if (offer.kind === 'relic') expect(offer.id).toBe(RELIC_IDS[0]);
    }
  }
});

test('the shop takes gold, and Haggler decreases the price', () => {
  const run = newRun(emptyMeta());
  const offer = { kind: 'piece', type: 'r' } as const;
  run.shop = [offer];
  const meta = { ...emptyMeta(), upgrades: { haggle: 2 } };
  expect(priceOf(offer, emptyMeta())).toBe(20);
  expect(priceOf(offer, meta)).toBe(16);
  run.gold = 15;
  expect(buyOffer(run, meta, offer)).toBe(false);
  run.gold = 16;
  expect(buyOffer(run, meta, offer)).toBe(true);
  expect(run).toMatchObject({ gold: 0, shop: [] });
  expect(run.army).toHaveLength(8);
});

test('takeDraft takes one reward and closes the draft', () => {
  const run = newRun(emptyMeta());
  run.draft = [{ kind: 'gold', amount: 12 }, { kind: 'piece', type: 'n' }];
  expect(takeDraft(run, run.draft[0])).toBe(true);
  expect(run).toMatchObject({ gold: 12, draft: null });
});

test('saved data survives a round trip, and unknown ids are removed', () => {
  const run = newRun(emptyMeta());
  run.relics = ['bounty'];
  run.shop = [{ kind: 'relic', id: 'interest' }, { kind: 'piece', type: 'q' }];
  const saved = JSON.parse(JSON.stringify(run));
  expect(parseRun(saved)).toEqual(run);

  saved.relics.push('removedRelic');
  saved.shop.push({ kind: 'relic', id: 'removedRelic' }, { kind: 'unknownKind' });
  expect(parseRun(saved)).toEqual(run);
  expect(parseRun({ ...saved, army: [] })).toBeNull();
  expect(parseRun(null)).toBeNull();

  expect(parseMeta({ crowns: 4, upgrades: { pawn: 2, removedUpgrade: 1 } }))
    .toEqual({ crowns: 4, best: 0, runs: 0, upgrades: { pawn: 2 } });
});
