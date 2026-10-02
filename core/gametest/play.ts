// Plays whole runs through `chrogue-core --stdio`, from `new_run` to the end of the run, with a
// simple bot: the camp choices, the shop, new shop items, the arrangement of the army, and
// upgrades between runs. Each response must succeed and have a view of the expected screen.
// Then the script plays the same sessions again and compares the transcripts byte for byte.
//
// Usage: bun core/gametest/play.ts [--sessions N] [--runs N] [--seed N]
import { createHash } from 'node:crypto';
import { Core, makeRng, option } from './core';
import type { Json, Rng } from './core';

const SESSIONS = option('sessions', 6);
const RUNS = option('runs', 6);
const SEED = option('seed', 1);
const MAX_PLIES = 600;

let problems = 0;
function problem(text: string): void {
  problems++;
  if (problems <= 20) console.log(`PROBLEM: ${text}`);
}

const KEYS: Record<string, string[]> = {
  title: ['meta', 'floors', 'run', 'can_continue'],
  upgrades: ['meta', 'slots'],
  battle: ['floor', 'phase', 'turn', 'pieces', 'check', 'last', 'moves', 'scout', 'enemy_moves', 'taken', 'gold',
    'capture_gold', 'capture_gold_exact', 'lost', 'rescued', 'relics', 'traits', 'clock', 'result'],
  camp: ['floor', 'enemy', 'reward', 'shop', 'army', 'army_max', 'relics', 'gold', 'can_start'],
  over: ['meta', 'summary', 'rows'],
};

/** Checks that a view has the fields of its screen and that the fields agree. */
function checkView(view: Json, screen: string, where: string): void {
  if (view?.screen !== screen) return problem(`${where}: the screen is ${view?.screen}, not ${screen}`);
  for (const key of KEYS[screen]) if (!(key in view)) problem(`${where}: the ${screen} view has no "${key}"`);
  if (screen === 'battle') {
    const over = view.result !== null;
    if ((view.phase === 'over') !== over) problem(`${where}: phase ${view.phase} with result ${JSON.stringify(view.result)}`);
    if (!over && (view.phase === 'player') !== (view.turn === 'w')) problem(`${where}: phase ${view.phase} with turn ${view.turn}`);
    if (view.phase !== 'player' && view.moves.length) problem(`${where}: moves in the phase ${view.phase}`);
    if (view.phase === 'player' && !view.moves.length) problem(`${where}: no moves for the player`);
    if (!view.scout && view.enemy_moves.length) problem(`${where}: enemy moves with no Scout`);
    const squares = new Set(view.pieces.map((p: Json) => p.square));
    if (squares.size !== view.pieces.length) problem(`${where}: two pieces on one square`);
    if (JSON.stringify(view).length > 16384) problem(`${where}: the view has ${JSON.stringify(view).length} bytes`);
  }
  if (screen === 'camp') {
    const homes = new Set(view.army.map((u: Json) => u.home));
    if (homes.size !== view.army.length || view.army.some((u: Json) => u.home > 15)) problem(`${where}: bad homes`);
    if (view.can_start !== !(view.reward?.open ?? false)) problem(`${where}: can_start ${view.can_start} with reward ${JSON.stringify(view.reward)}`);
  }
  if (screen === 'upgrades' && view.slots.length !== 16) problem(`${where}: ${view.slots.length} slots`);
}

interface Tally {
  runs: number;
  won: number;
  lost: number;
  gaveUp: number;
  battles: number;
  moves: number;
  floors: number[];
  bought: number;
  rerolls: number;
  placed: number;
  rewards: number;
  skipped: number;
  upgrades: number;
  resumed: number;
  promotions: number;
}

async function expect(core: Core, cmd: string, args: Record<string, unknown>, screen: string): Promise<Json> {
  const reply = await core.ok(cmd, args);
  checkView(reply.view, screen, cmd);
  return reply;
}

/** Plays a battle to its result. White is the engine AI at `level`, or random moves for a weak bot. */
async function battle(core: Core, view: Json, rng: Rng, level: number | null, tally: Tally): Promise<Json> {
  tally.battles++;
  for (let ply = 0; view.phase !== 'over'; ply++) {
    if (ply >= MAX_PLIES) return null;
    let reply: Json;
    if (view.phase === 'enemy') {
      reply = await expect(core, 'enemy_move', {}, 'battle');
    } else if (level === null) {
      const m = rng.pick(view.moves as Json[]);
      reply = await expect(core, 'move', { from: m.from, to: m.to, promo: m.promo }, 'battle');
    } else {
      reply = await expect(core, 'debug_ai_move', { level }, 'battle');
    }
    if (reply.events.some((e: Json) => e.type === 'promote')) tally.promotions++;
    const first = reply.events[0];
    if (first?.type !== 'move') problem(`the first event of a move is ${first?.type}`);
    if (reply.view.phase === 'over' && !reply.events.some((e: Json) => e.type === 'result')) problem('a battle ends with no result event');
    view = reply.view;
    tally.moves++;
  }
  return view;
}

/** The camp: a reward, some purchases, new items, and a new arrangement. */
async function camp(core: Core, view: Json, rng: Rng, tally: Tally): Promise<void> {
  if (view.reward?.open) {
    const offers: Json[] = view.reward.offers;
    const order = ['piece', 'relic', 'gold'];
    const choice = offers.map((o, i) => ({ o, i })).filter(({ o }) => o.blocked === null)
      .sort((a, b) => order.indexOf(a.o.kind) - order.indexOf(b.o.kind))[0];
    if (choice && rng.chance(0.9)) {
      view = (await expect(core, 'take_reward', { index: choice.i }, 'camp')).view;
      if (view.reward.taken !== choice.i || view.reward.state !== 'taken') problem('the reward is not taken');
      tally.rewards++;
    } else {
      view = (await expect(core, 'skip_reward', {}, 'camp')).view;
      if (view.reward.state !== 'skipped') problem('the reward is not skipped');
      tally.skipped++;
    }
  }
  for (let n = 0; n < 4; n++) {
    const items: Json[] = view.shop.offers;
    const i = items.findIndex((o) => o.affordable && o.blocked === null);
    if (i >= 0) {
      const before = view.gold;
      const reply = await expect(core, 'buy', { index: i }, 'camp');
      const cue = reply.events.find((e: Json) => e.type === 'camp_action');
      if (!cue || cue.gold_before !== before || cue.gold !== before - items[i].price) problem(`the cue of buy is ${JSON.stringify(cue)}`);
      view = reply.view;
      tally.bought++;
    } else if (view.shop.can_reroll && view.gold >= 10 && rng.chance(0.5)) {
      const reply = await expect(core, 'reroll', {}, 'camp');
      if (!reply.events.some((e: Json) => e.type === 'camp_action' && e.rolled)) problem('reroll has no rolled cue');
      view = reply.view;
      tally.rerolls++;
    }
  }
  for (let n = rng.int(3); n > 0; n--) {
    const unit = rng.pick(view.army as Json[]);
    view = (await expect(core, 'place', { unit: unit.id, square: rng.int(16) }, 'camp')).view;
    tally.placed++;
  }
}

/** Plays one run from the title or the end screen. Returns the end screen. */
async function run(core: Core, rng: Rng, strong: boolean, tally: Tally): Promise<void> {
  let view = (await expect(core, 'new_run', {}, 'battle')).view;
  tally.runs++;
  for (;;) {
    // White gets stronger on each floor. A weak bot plays random moves.
    const level = strong ? Math.min(8, view.floor.number + 2) : null;
    const end = await battle(core, view, rng, level, tally);
    if (!end) {
      // The battle did not end: a pawn with Tactical Retreat can step back and forward forever.
      const reply = await expect(core, 'give_up', {}, 'over');
      tally.gaveUp++;
      tally.lost++;
      tally.floors.push(reply.view.summary.cleared);
      return;
    }
    const next = end.result.next;
    const reply = await expect(core, 'continue', {}, next === 'camp' ? 'camp' : 'over');
    if (next !== 'camp') {
      if (next === 'won') tally.won++;
      else tally.lost++;
      tally.floors.push(reply.view.summary.cleared);
      return;
    }
    view = reply.view;
    // Some visits leave for the title and continue the run, as a client that restarts does.
    if (rng.chance(0.1)) {
      await expect(core, 'to_title', {}, 'title');
      view = (await expect(core, 'continue_run', {}, 'camp')).view;
      tally.resumed++;
    }
    await camp(core, view, rng, tally);
    view = (await expect(core, 'start_battle', {}, 'battle')).view;
    if (rng.chance(0.05)) {
      await expect(core, 'to_title', {}, 'title');
      view = (await expect(core, 'continue_run', {}, 'battle')).view;
      tally.resumed++;
    }
  }
}

/** Buys each upgrade that the crowns pay for. */
async function upgrades(core: Core, tally: Tally): Promise<void> {
  let view = (await expect(core, 'open_upgrades', {}, 'upgrades')).view;
  for (;;) {
    const slot = view.slots.find((s: Json) => s?.affordable);
    if (!slot) break;
    const reply = await expect(core, 'buy_upgrade', { upgrade: slot.id }, 'upgrades');
    view = reply.view;
    tally.upgrades++;
  }
  await expect(core, 'back', {}, 'title');
}

async function session(index: number, tally: Tally): Promise<string[]> {
  const seed = SEED * 1000 + index;
  const core = new Core(['--no-save', '--debug', '--seed', String(seed)]);
  const rng = makeRng(seed);
  checkView((await core.ok('view')).view, 'title', 'view');
  for (let n = 0; n < RUNS; n++) {
    // Two runs of three have the strong bot.
    await run(core, rng, (index + n) % 3 !== 2, tally);
    await upgrades(core, tally);
  }
  const exit = await core.close();
  if (exit !== 0) problem(`session ${index}: exit code ${exit}`);
  return core.transcript;
}

const newTally = (): Tally => ({
  runs: 0, won: 0, lost: 0, gaveUp: 0, battles: 0, moves: 0, floors: [], bought: 0, rerolls: 0, placed: 0,
  rewards: 0, skipped: 0, upgrades: 0, resumed: 0, promotions: 0,
});

async function all(): Promise<{ tally: Tally; hash: string; lines: number; ms: number }> {
  const tally = newTally();
  const hash = createHash('sha256');
  let lines = 0;
  const start = performance.now();
  const transcripts = await Promise.all([...Array(SESSIONS).keys()].map((i) => session(i, tally)));
  for (const t of transcripts) {
    for (const line of t) hash.update(`${line}\n`);
    lines += t.length;
  }
  return { tally, hash: hash.digest('hex'), lines, ms: performance.now() - start };
}

const first = await all();
const { tally } = first;
console.log(`first pass: ${first.lines} responses in ${(first.ms / 1000).toFixed(1)} s, sha256 ${first.hash}`);
console.log(JSON.stringify({ ...tally, floors: tally.floors.join(' ') }));
const second = await all();
console.log(`second pass: ${second.lines} responses, sha256 ${second.hash}`);
if (second.hash !== first.hash) problem('the two passes have different transcripts');
if (tally.runs < 30) problem(`only ${tally.runs} runs`);
if (tally.won < 1) problem('no run was won');
if (tally.lost < 3) problem(`only ${tally.lost} runs were lost`);
console.log(problems ? `FAIL: ${problems} problems` : 'PASS');
process.exit(problems ? 1 : 0);
