// The parity test of the game layer: the TypeScript game in src/game/ against the Rust core.
//
// Usage: bun core/gametest/parity.ts [--seed N] [--battles N]
//
// 1. Content: the relics, upgrades, floors, prices, and constants of `hello` against the
//    TypeScript definitions.
// 2. Rules with no randomness: new runs for each set of upgrade levels, shop prices, and
//    scripted battles. The script selects each move from the legal moves with its own random
//    numbers and plays it in the two implementations: in TypeScript with createBattle, playMove,
//    and settleBattle; in the core with `move` for White and `debug_enemy_move` for Black.
//    After each move it compares the board, the gold from captures, the lost and rescued units,
//    the relics that had an effect, and the legal moves. At the end it compares the result,
//    the reward, and the settled run.
//
// The exit code is 1 if a value differs, or if the battles do not cover each result.
import { VALUE, inCheck, kingSquare, legalMoves, makeMove, movesFrom, outcome, unmakeMove } from '../../src/engine';
import type { Move, PieceId, PieceType, State } from '../../src/engine';
import {
  ARMY_MAX, CONSCRIPT_ID, FLOORS, PIECE_NAME, RELICS, RELIC_IDS, REROLL_COST, UPGRADES, UPGRADE_IDS, UPGRADE_NAME_MAX,
  UPGRADE_SLOTS, addUnit, basePrice, createBattle, finishRun, newRun, playMove, priceOf, rollDraft, settleBattle,
} from '../../src/game';
import type { Battle, Meta, Offer, RecruitType, Run, UpgradeId } from '../../src/game';
import { Core, makeRng, option } from './core';
import type { Json, Rng } from './core';

const SEED = option('seed', 1);
const BATTLES = option('battles', 400);
const MAX_PLIES = 400;

let failures = 0;
const failed = (where: string, what: string, ts: unknown, core: unknown): void => {
  failures++;
  if (failures <= 30) console.log(`DIFF ${where}: ${what}\n  ts:   ${JSON.stringify(ts)}\n  core: ${JSON.stringify(core)}`);
};
/** JSON with sorted keys, thus the order of the fields does not count. */
const canon = (value: unknown): string => JSON.stringify(value, (_, v) =>
  v && typeof v === 'object' && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : 1))) : v);
const same = (where: string, what: string, ts: unknown, core: unknown): boolean => {
  if (canon(ts) === canon(core)) return true;
  failed(where, what, ts, core);
  return false;
};

const RECRUITS: RecruitType[] = ['p', 'n', 'b', 'r', 'q'];
const TRAIT_IDS = RELIC_IDS.filter((id) => RELICS[id].foeText);
const emptyMeta = (): Meta => ({ crowns: 0, best: 0, runs: 0, upgrades: {} });

// ---- 1. Content ----

function checkContent(content: Json): void {
  same('content', 'relic ids', RELIC_IDS, content.relics.map((r: Json) => r.id));
  for (const [i, id] of RELIC_IDS.entries()) {
    const ts = RELICS[id], core = content.relics[i] ?? {};
    const flags = Object.entries(ts.rules ?? {}).filter(([, on]) => on).map(([flag]) => flag).sort();
    same(`relic ${id}`, 'fields', {
      name: ts.name, text: ts.text, foeText: ts.foeText ?? null, trait: Boolean(ts.foeText), flags, hooks: Object.keys(ts.hooks ?? {}),
    }, {
      name: core.name, text: core.text, foeText: core.foe_text, trait: core.trait, flags: [...(core.rule_flags ?? ['(no flags)'])].sort(), hooks: core.hooks,
    });
  }
  same('content', 'upgrade ids', UPGRADE_IDS, content.upgrades.map((u: Json) => u.id));
  for (const [i, id] of UPGRADE_IDS.entries()) {
    const { name, text, costs, ...hooks } = UPGRADES[id], core = content.upgrades[i] ?? {};
    same(`upgrade ${id}`, 'fields', { name, text, costs, hooks: Object.keys(hooks) }, { name: core.name, text: core.text, costs: core.costs, hooks: core.hooks });
  }
  same('content', 'floor count', FLOORS.length, content.floors.length);
  FLOORS.forEach((f, i) => {
    const core = content.floors[i] ?? {};
    same(`floor ${i + 1}`, 'fields', { name: f.name, budget: f.budget, traits: f.traits, boss: Boolean(f.boss) },
      { name: core.name, budget: core.budget, traits: core.traits, boss: core.boss });
  });
  // The gold reward of the draft: the TypeScript game has it only in rollDraft.
  for (let floor = 1; floor <= FLOORS.length; floor++) {
    const run = newRun(emptyMeta());
    run.floor = floor;
    let gold: number | null = null;
    for (let n = 0; n < 500 && gold === null; n++) {
      const offer = rollDraft(run).find((o) => o.kind === 'gold');
      if (offer?.kind === 'gold') gold = offer.amount;
    }
    same(`floor ${floor}`, 'draft gold', gold, content.floors[floor - 1]?.draft_gold);
  }
  const types: PieceType[] = ['k', 'q', 'r', 'b', 'n', 'p'];
  same('content', 'pieces',
    types.map((t) => ({ kind: t, name: PIECE_NAME[t], value: VALUE[t], price: t === 'k' ? null : basePrice({ kind: 'piece', type: t }) })),
    content.pieces.map((p: Json) => ({ kind: p.kind, name: p.name, value: p.value, price: p.price })));
  const won = finishRun(emptyMeta(), newRun(emptyMeta()), true);
  same('content', 'constants', {
    relic_price: basePrice({ kind: 'relic', id: RELIC_IDS[0] }), gold_price: basePrice({ kind: 'gold', amount: 5 }),
    reroll_cost: REROLL_COST, win_crowns: won.bonus, army_max: ARMY_MAX, upgrade_slots: UPGRADE_SLOTS,
    upgrade_name_max: UPGRADE_NAME_MAX, traits_max: Math.max(...FLOORS.map((f) => f.traits)),
  }, {
    relic_price: content.relic_price, gold_price: 0,
    reroll_cost: content.reroll_cost, win_crowns: content.win_crowns, army_max: content.army_max, upgrade_slots: content.upgrade_slots,
    upgrade_name_max: content.upgrade_name_max, traits_max: content.traits_max,
  });
}

// ---- 2. New runs and prices ----

const coreRun = async (core: Core): Promise<Json> => (await core.ok('view', { run: true })).data.run;
const runFields = (run: Run | Json) => ({
  floor: run.floor, gold: run.gold, army: run.army, nextId: run.nextId, relics: run.relics, phase: run.phase,
});

async function setUpgrades(core: Core, levels: Partial<Record<UpgradeId, number>>): Promise<void> {
  for (const id of UPGRADE_IDS) await core.ok('debug_set_upgrade', { upgrade: id, level: levels[id] ?? 0 });
}

async function checkNewRuns(core: Core): Promise<number> {
  let n = 0;
  for (let pawn = 0; pawn <= 3; pawn++) {
    for (let gold = 0; gold <= 3; gold++) {
      for (let bishop = 0; bishop <= 1; bishop++) {
        const upgrades = { pawn, gold, bishop };
        await setUpgrades(core, upgrades);
        await core.ok('new_run');
        const ts = newRun({ ...emptyMeta(), upgrades });
        same(`new run ${JSON.stringify(upgrades)}`, 'run', runFields(ts), runFields(await coreRun(core)));
        await core.ok('to_title');
        n++;
      }
    }
  }
  await setUpgrades(core, {});
  return n;
}

/** Moves the core to the camp: a battle that White wins with one capture. */
async function reachCamp(core: Core): Promise<void> {
  const view = (await core.ok('view')).view;
  if (view.screen === 'title' || view.screen === 'over') await core.ok('new_run');
  await core.ok('debug_set_floor', { floor: 1 });
  await core.ok('debug_set_army', { units: [{ kind: 'k', home: 4 }, { kind: 'r', home: 0 }] });
  await core.ok('debug_set_enemy', { pieces: [{ kind: 'k', square: 60 }, { kind: 'p', square: 8 }], traits: [] });
  await core.ok('move', { from: 0, to: 8 });
  await core.ok('continue');
}

async function checkPrices(core: Core): Promise<number> {
  await reachCamp(core);
  const offers: Offer[] = [...RECRUITS.map((type): Offer => ({ kind: 'piece', type })), { kind: 'relic', id: 'bounty' }, { kind: 'gold', amount: 9 }];
  await core.ok('debug_set_shop', { offers });
  let n = 0;
  for (let haggle = 0; haggle <= 2; haggle++) {
    const reply = await core.ok('debug_set_upgrade', { upgrade: 'haggle', level: haggle });
    const meta = { ...emptyMeta(), upgrades: { haggle } };
    same(`prices haggle ${haggle}`, 'shop prices', offers.map((o) => priceOf(o, meta)), reply.view.shop.offers.map((o: Json) => o.price));
    n += offers.length;
  }
  await core.ok('debug_set_upgrade', { upgrade: 'haggle', level: 0 });
  await core.ok('to_title');
  return n;
}

// ---- 3. Battles ----

const coreId = (id: PieceId): number =>
  typeof id === 'number' ? id : id === CONSCRIPT_ID ? 20000 : 30000 + Number(String(id).slice(1));

function tsBoard(battle: Battle): Json[] {
  const pieces: Json[] = [];
  battle.state.board.forEach((p, square) => {
    if (p) pieces.push({ id: coreId(p.id), kind: p.type, color: p.color, square });
  });
  return pieces;
}

// A function, thus TypeScript does not narrow battle.result in the loop of playBattle.
const resultOf = (battle: Battle) => battle.result && { winner: battle.result.winner, reason: battle.result.reason };

const moveKey = (m: { from: number; to: number; promo?: string | null }): string => `${m.from}-${m.to}${m.promo ?? ''}`;

/** A random battle setup: floor, gold, army, relics, traits, and enemy. */
function randomSetup(rng: Rng): Run {
  const run = newRun(emptyMeta());
  run.floor = 1 + rng.int(FLOORS.length);
  run.gold = rng.int(60);
  for (let n = rng.int(10); n > 0; n--) addUnit(run, rng.pick(RECRUITS));
  // Some battles start with a smaller army, thus the enemy wins some battles.
  if (rng.chance(0.25)) run.army = run.army.filter((u) => u.type === 'k' || rng.chance(0.4));
  // A new home for some units: the camp can put any unit on any home square.
  if (rng.chance(0.3)) {
    const homes = rng.shuffle([...Array(16).keys()]);
    run.army.forEach((u, i) => (u.home = homes[i]));
  }
  run.relics = RELIC_IDS.filter(() => rng.chance(0.3));
  const traits = rng.shuffle(TRAIT_IDS).slice(0, rng.int(3));
  // The enemy: a king and some pieces on ranks 5 to 8. A black pawn is not on rank 8.
  const squares = rng.shuffle([...Array(32).keys()].map((i) => 32 + i));
  const king = squares.pop() ?? 60;
  const pieces: Run['enemy']['pieces'] = [{ type: 'k', square: king }];
  const count = rng.chance(0.2) ? rng.int(3) : 2 + rng.int(12);
  for (const square of squares) {
    if (pieces.length > count) break;
    const type = rng.pick(RECRUITS);
    if (type === 'p' && square >= 56) continue;
    pieces.push({ type, square });
  }
  run.enemy = { pieces, traits };
  return run;
}

/**
 * How the script selects the moves of a battle. Each style makes a different end more likely:
 * captures make a rout, a mate in one makes a checkmate, quiet officer moves make a draw by the
 * clock, and pawn pushes make promotions. Each style plays a castle when it can, half of the time.
 */
const STYLES = ['captures', 'mates', 'quiet', 'pawns'] as const;
type Style = (typeof STYLES)[number];

function mates(state: State, m: Move): boolean {
  const undo = makeMove(state, m);
  const end = outcome(state);
  unmakeMove(state, m, undo);
  return end?.reason === 'checkmate';
}

// A pawn step toward the first rank of its side: the backward step of backpedal.
const isBack = (state: State, m: Move): boolean =>
  state.board[m.from]?.type === 'p' && m.to - m.from === (state.turn === 'w' ? -8 : 8);

function chooseMove(state: State, legal: Move[], style: Style, rng: Rng): Move {
  const castles = legal.filter((m) => m.castle);
  if (castles.length && rng.chance(0.5)) return rng.pick(castles);
  const captures = legal.filter((m) => state.board[m.to] || m.epCapture);
  const pawn = (m: Move) => state.board[m.from]?.type === 'p';
  switch (style) {
    case 'captures':
      return captures.length && rng.chance(0.5) ? rng.pick(captures) : rng.pick(legal);
    case 'mates': {
      const mate = legal.find((m) => mates(state, m));
      if (mate) return mate;
      return captures.length && rng.chance(0.3) ? rng.pick(captures) : rng.pick(legal);
    }
    case 'quiet': {
      const quiet = legal.filter((m) => !pawn(m) && !captures.includes(m));
      return quiet.length && rng.chance(0.97) ? rng.pick(quiet) : rng.pick(legal);
    }
    case 'pawns': {
      const promos = legal.filter((m) => m.promo);
      if (promos.length) return rng.pick(promos);
      const pushes = legal.filter((m) => pawn(m) && !isBack(state, m));
      return pushes.length && rng.chance(0.7) ? rng.pick(pushes) : rng.pick(legal);
    }
  }
}

interface Stats {
  battles: number;
  moves: number;
  captures: number;
  promotions: number;
  castles: number;
  enPassant: number;
  backward: number;
  rescued: number;
  conscripts: number;
  bonuses: number;
  unfinished: number;
  stuck: number;
  floor8Draws: number;
  results: Record<string, number>;
  next: Record<string, number>;
}

async function setUpCore(core: Core, run: Run): Promise<Json> {
  const view = (await core.ok('view')).view;
  if (view.screen === 'title' || view.screen === 'over') await core.ok('new_run');
  if (view.screen === 'camp') {
    if (view.reward?.open) await core.ok('skip_reward');
    await core.ok('start_battle');
  }
  await core.ok('debug_set_floor', { floor: run.floor });
  await core.ok('debug_set_gold', { gold: run.gold });
  await core.ok('debug_set_army', { units: run.army.map((u) => ({ id: u.id, kind: u.type, home: u.home })) });
  // The relics of the core keep their order, thus the script removes all of them first.
  for (const id of RELIC_IDS) await core.ok('debug_set_relic', { relic: id, on: false });
  for (const id of run.relics) await core.ok('debug_set_relic', { relic: id, on: true });
  const pieces = run.enemy.pieces.map((p) => ({ kind: p.type, square: p.square }));
  return (await core.ok('debug_set_enemy', { pieces, traits: run.enemy.traits })).view;
}

async function playBattle(core: Core, rng: Rng, n: number, meta: Meta, stats: Stats): Promise<void> {
  const run = randomSetup(rng);
  const where = `battle ${n}`;
  let view = await setUpCore(core, run);
  // The core makes the next id from the army; the TypeScript run keeps its own. The settled runs compare both.
  run.nextId = Math.max(...run.army.map((u) => u.id)) + 1;
  const battle = createBattle(run);
  const style = rng.pick(STYLES);
  if (!same(where, 'start board', tsBoard(battle), view.pieces)) return;
  if (battle.state.board.some((p) => p?.id === CONSCRIPT_ID)) stats.conscripts++;
  stats.battles++;

  for (let ply = 0; !battle.result; ply++) {
    if (ply >= MAX_PLIES) {
      stats.unfinished++;
      await core.ok('to_title');
      return;
    }
    const legal = legalMoves(battle.state);
    const white = battle.state.turn === 'w';
    if (white) {
      if (!same(where, `legal moves at ply ${ply}`, legal.map(moveKey).sort(), view.moves.map(moveKey).sort())) return;
      // The session has Scout, thus the view has the moves of each enemy piece (movesFrom).
      const scouted = battle.state.board.flatMap((p, square) => (p?.color === 'b' ? movesFrom(battle.state, square) : []));
      if (!same(where, `scout moves at ply ${ply}`, scouted.map(moveKey).sort(), view.enemy_moves.map(moveKey).sort())) return;
    }
    if (!legal.length) {
      // The side to move has no move, but the battle has no result: the TypeScript game checks
      // the result only after a move, thus a battle can start with no move. The core does the same.
      same(where, 'no move at the start', { phase: 'player', moves: [], result: null }, { phase: view.phase, moves: view.moves, result: view.result });
      stats.stuck++;
      await core.ok('to_title');
      return;
    }
    const move = chooseMove(battle.state, legal, style, rng);
    const wasBack = isBack(battle.state, move);
    const report = playMove(battle, run, move);
    const reply = await core.ok(white ? 'move' : 'debug_enemy_move', { from: move.from, to: move.to, promo: move.promo });
    view = reply.view;
    stats.moves++;
    if (report.capture) stats.captures++;
    if (move.promo) stats.promotions++;
    if (move.castle) stats.castles++;
    if (move.epCapture) stats.enPassant++;
    if (wasBack) stats.backward++;

    const events: Json[] = reply.events;
    const capture = events.find((e) => e.type === 'capture');
    const relics = events.find((e) => e.type === 'relic')?.ids ?? [];
    const check = inCheck(battle.state, battle.state.turn) ? kingSquare(battle.state, battle.state.turn) : null;
    const ok = same(where, `move ${ply} ${moveKey(move)}`, {
      board: tsBoard(battle),
      turn: battle.state.turn,
      capture: report.capture,
      relics: report.relics,
      gold: battle.gold,
      lost: battle.lost,
      rescued: battle.rescued,
      taken: battle.taken,
      check,
      clock: battle.state.clock,
      result: resultOf(battle),
    }, {
      board: view.pieces,
      turn: view.turn,
      capture: capture ? { square: capture.square, gold: capture.gold } : null,
      relics,
      gold: view.capture_gold_exact,
      lost: view.lost,
      rescued: view.rescued,
      taken: view.taken,
      check: view.check,
      clock: view.clock,
      result: view.result && { winner: view.result.winner, reason: view.result.reason },
    });
    if (!ok) return;
  }

  const result = battle.result;
  stats.results[`${result.winner ?? 'draw'}:${result.reason}`] = (stats.results[`${result.winner ?? 'draw'}:${result.reason}`] ?? 0) + 1;
  stats.rescued += battle.rescued.length;
  stats.bonuses += result.reward.bonuses.length;
  const coreResult = view.result;
  same(where, 'reward', {
    reward: result.reward,
    rescued: result.winner === 'b' ? 0 : battle.rescued.length,
  }, {
    reward: { captures: coreResult.reward.captures, clear: coreResult.reward.clear, bonuses: coreResult.reward.bonuses },
    rescued: coreResult.rescued,
  });

  const floor = run.floor;
  const next = settleBattle(run, battle);
  // A draw on the last floor: the two games stay on the last floor.
  if (result.winner === null && floor === FLOORS.length) stats.floor8Draws++;
  stats.next[next] = (stats.next[next] ?? 0) + 1;
  same(where, 'next', next, coreResult.next);
  const reply = await core.ok('continue');
  if (next === 'camp') {
    const settled = await coreRun(core);
    same(where, 'settled run', {
      ...runFields(run), draft: run.draft !== null,
    }, {
      ...runFields(settled), draft: settled.draft !== null,
    });
    same(where, 'camp view', { screen: 'camp', gold: run.gold, reward: run.draft !== null }, { screen: reply.view.screen, gold: reply.view.gold, reward: reply.view.reward !== null });
  } else {
    const summary = finishRun(meta, run, next === 'won');
    const end = reply.events.find((e: Json) => e.type === 'run_end');
    same(where, 'run end', { summary, run: runFields(run), meta }, {
      summary: { won: end.won, cleared: end.cleared, bonus: end.bonus, crowns: end.crowns, newBest: end.new_best },
      run: runFields(end.run),
      meta: { ...reply.view.meta, upgrades: {} },
    });
  }
}

// ---- Main ----

async function main(): Promise<void> {
  const core = new Core(['--no-save', '--debug', '--seed', String(SEED)]);
  const hello = await core.ok('hello');
  checkContent(hello.data.content);
  const contentFailures = failures;
  const runs = await checkNewRuns(core);
  const prices = await checkPrices(core);

  await core.ok('debug_set_upgrade', { upgrade: 'scout', level: 1 });
  const rng = makeRng(SEED);
  const meta = emptyMeta();
  const stats: Stats = {
    battles: 0, moves: 0, captures: 0, promotions: 0, castles: 0, enPassant: 0, backward: 0, rescued: 0, conscripts: 0,
    bonuses: 0, unfinished: 0, stuck: 0, floor8Draws: 0, results: {}, next: {},
  };
  for (let n = 0; n < BATTLES; n++) await playBattle(core, rng, n, meta, stats);
  const exit = await core.close();

  console.log(`content: ${RELIC_IDS.length} relics, ${UPGRADE_IDS.length} upgrades, ${FLOORS.length} floors, ${contentFailures} differences`);
  console.log(`new runs: ${runs} upgrade sets; shop prices: ${prices}`);
  console.log(`battles: ${JSON.stringify(stats)}`);
  console.log(`core exit code: ${exit}`);
  const reasons = ['w:checkmate', 'b:checkmate', 'w:rout', 'b:rout', 'draw:clock'];
  const missing = reasons.filter((r) => !stats.results[r]);
  if (missing.length) console.log(`The battles have no result of these kinds: ${missing.join(', ')}`);
  const finished = stats.battles - stats.unfinished - stats.stuck;
  if (finished < 200) console.log(`Only ${finished} battles have a result. The test needs 200.`);
  console.log(failures ? `FAIL: ${failures} differences` : 'PASS: no differences');
  process.exit(failures || missing.length || finished < 200 || exit !== 0 ? 1 : 0);
}

await main();
