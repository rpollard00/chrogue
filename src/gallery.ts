// The animation gallery, a page for development. Each demo shows a screen of the game with prepared data.
// Then the demo does the clicks that start the motion.
import type { PieceType, Square } from './engine';
import { generateEnemy } from './game';
import type { Meta, Offer, Run } from './game';
import type { App } from './ui/app';
import { button, h } from './ui/dom';
import { createApp } from './ui/shell';

const FIRST_STEP_MS = 800;
const STEP_MS = 450;

const sq = (name: string): Square => 'abcdefgh'.indexOf(name[0]) + 8 * (Number(name[1]) - 1);
const TYPE: Record<string, PieceType> = { K: 'k', Q: 'q', R: 'r', B: 'b', N: 'n' };

// Reads a list such as "Ke1 Ra1 c2". An item with no letter before the square is a pawn.
const pieces = (list: string): { type: PieceType; square: Square }[] =>
  list.split(' ').map((item) => (item.length === 3 ? { type: TYPE[item[0]], square: sq(item.slice(1)) } : { type: 'p', square: sq(item) }));

const meta = (): Meta => ({ crowns: 12, best: 3, runs: 4, upgrades: { gold: 2 } });

function run(army: string, enemy: string, rest: Partial<Run> = {}): Run {
  const units = pieces(army).map((piece, i) => ({ id: i + 1, type: piece.type, home: piece.square }));
  return {
    floor: 3, gold: 24, army: units, nextId: units.length + 1, relics: [],
    enemy: { pieces: pieces(enemy), traits: [] }, phase: 'battle', draft: null, shop: [], ...rest,
  };
}

const ARMY = 'Ke1 Ra1 Ng1 c2 d2 e2 f2';
const DRAFT: Offer[] = [{ kind: 'piece', type: 'n' }, { kind: 'relic', id: 'interest' }, { kind: 'gold', amount: 16 }];
const SHOP: Offer[] = [
  { kind: 'piece', type: 'b' }, { kind: 'piece', type: 'p' }, { kind: 'relic', id: 'secondWind' }, { kind: 'relic', id: 'sidestep' },
];
const camp = (rest: Partial<Run> = {}): Run => run(ARMY, 'Ke8 d7 e7', { gold: 60, phase: 'camp', relics: ['bounty'], shop: SHOP, ...rest });

/** Finds the element that one step of a demo clicks. */
type Step = (stage: HTMLElement) => HTMLElement | null | undefined;

const square = (name: string): Step => (stage) => stage.querySelector<HTMLElement>(`.board button[aria-label^="${name}"]`);
const press = (label: string): Step => (stage) =>
  [...stage.querySelectorAll('button')].find((el) => el.getAttribute('aria-label') === label || el.textContent?.startsWith(label));
const card = (name: string): Step => (stage) =>
  [...stage.querySelectorAll('.card')].find((el) => el.querySelector('h3')?.textContent === name)?.querySelector<HTMLElement>('.act');

interface Demo {
  group: string;
  name: string;
  note: string;
  /** Shows the first screen of the demo. */
  start(app: App): void;
  /** The clicks that come after the first screen. */
  steps?: Step[];
  /** True if the demo shows the banner at the start of a battle. */
  banner?: true;
}

const DEMOS: Demo[] = [
  {
    group: 'Battle', name: 'Battle start', note: 'The banner with the floor and the name of the enemy.', banner: true,
    start: (app) => app.startBattle(run(ARMY, 'Ke8', { enemy: generateEnemy(3) })),
  },
  {
    group: 'Battle', name: 'Boss battle start', note: 'The banner of a boss floor.', banner: true,
    start: (app) => app.startBattle(run(ARMY, 'Ke8', { floor: 4, enemy: generateEnemy(4) })),
  },
  {
    group: 'Battle', name: 'Move and capture',
    note: 'The rook moves and captures. The gold shows on the square and in the side panel. Bounty flashes. Then the enemy moves.',
    start: (app) => app.startBattle(run('Ke1 Ra1 Ng1 c2 d2 e2', 'Ke8 a7 d7 Ng8', { relics: ['bounty'] })),
    steps: [square('a1'), square('a7')],
  },
  {
    group: 'Battle', name: 'Check', note: 'The square of the king in check pulses.',
    start: (app) => app.startBattle(run('Ke1 Ra1 c2', 'Ke8 d7 e7 h6')),
    steps: [square('a1'), square('a8')],
  },
  {
    group: 'Battle', name: 'Lost piece', note: 'The enemy king captures the rook. Second Wind flashes.',
    start: (app) => app.startBattle(run('Ke1 Ra8 c2 d2', 'Kh8 g7 h7', { relics: ['secondWind'] })),
    steps: [square('a8'), square('g8')],
  },
  {
    group: 'Battle', name: 'Promotion', note: 'The pawn becomes a queen.',
    start: (app) => app.startBattle(run('Ke1 a7', 'Kh6 h5')),
    steps: [square('a7'), square('a8'), press('Queen')],
  },
  {
    group: 'Battle result', name: 'Victory', note: 'Checkmate. The gold rows count up, and Interest flashes.',
    start: (app) => app.startBattle(run('Ke1 Ra1 c2', 'Kh8 g7 h7', { gold: 27, relics: ['bounty', 'interest'] })),
    steps: [square('a1'), square('a8')],
  },
  {
    group: 'Battle result', name: 'Defeat', note: 'The enemy king captures the last piece of the army.',
    start: (app) => app.startBattle(run('Ke1 Ra8', 'Kh8 g7 h7')),
    steps: [square('a8'), square('g8')],
  },
  {
    group: 'Battle result', name: 'Draw', note: 'Only the kings remain.',
    start: (app) => app.startBattle(run('Ke1', 'Ka8 e2')),
    steps: [square('e1'), square('e2')],
  },
  {
    group: 'Camp', name: 'Camp arrival', note: 'The reward cards and the shop cards come into view.',
    start: (app) => app.openCamp(camp({ draft: DRAFT })),
  },
  {
    group: 'Camp', name: 'Take a gold reward', note: 'The gold counts up.',
    start: (app) => app.openCamp(camp({ draft: DRAFT })),
    steps: [card('16 gold')],
  },
  {
    group: 'Camp', name: 'Buy a piece', note: 'The gold counts down. The bishop shows on its home square.',
    start: (app) => app.openCamp(camp()),
    steps: [card('Bishop')],
  },
  {
    group: 'Camp', name: 'Buy a relic', note: 'The gold counts down. The relic flashes in the list.',
    start: (app) => app.openCamp(camp()),
    steps: [card('Second Wind')],
  },
  {
    group: 'Camp', name: 'New shop items', note: 'The shop cards turn.',
    start: (app) => app.openCamp(camp()),
    steps: [press('Get new items')],
  },
  {
    group: 'Between runs', name: 'Upgrade', note: 'The card flashes, and the crowns count down.',
    start: (app) => app.show({ name: 'upgrades', bought: null }),
    steps: [card('Militia')],
  },
  {
    group: 'Between runs', name: 'Run won', note: 'The Black King falls. The crown rows count up.',
    start: (app) => app.endRun(run(ARMY, 'Ke8', { floor: 8 }), true),
  },
  {
    group: 'Between runs', name: 'Run lost', note: 'Your king falls. The run is a new best.',
    start: (app) => app.endRun(run(ARMY, 'Ke8', { floor: 6 }), false),
  },
];

const root = document.getElementById('gallery');
if (!root) throw new Error('The page has no #gallery element');

const title = h('h2'), note = h('p', { class: 'dim' });
const stage = h('div', { class: 'gallery-stage' });
const links = new Map<Demo, HTMLElement>();
let current = DEMOS.find((demo) => `#${encodeURIComponent(demo.name)}` === location.hash) ?? DEMOS[0];
let timers: ReturnType<typeof setTimeout>[] = [];

// Shows a demo from its start. Each time, the demo gets a new shell that saves nothing.
function play(demo: Demo): void {
  timers.forEach(clearTimeout);
  current = demo;
  history.replaceState(null, '', `#${encodeURIComponent(demo.name)}`);
  title.textContent = demo.name;
  note.textContent = demo.note;
  for (const [other, link] of links) link.setAttribute('aria-current', String(other === demo));
  stage.classList.toggle('skip-intro', !demo.banner);
  demo.start(createApp(stage, { meta: meta(), run: null, saveMeta() {}, saveRun() {} }));
  timers = (demo.steps ?? []).map((step, i) => setTimeout(() => step(stage)?.click(), FIRST_STEP_MS + i * STEP_MS));
}

const nav = h('nav', { class: 'gallery-nav' }, h('h1', {}, 'Animation gallery'));
for (const demo of DEMOS) {
  if (demo.group !== [...links.keys()].at(-1)?.group) nav.append(h('h3', {}, demo.group));
  const link = button(demo.name, () => play(demo));
  links.set(demo, link);
  nav.append(link);
}

root.append(h('div', { class: 'gallery' },
  nav,
  h('section', { class: 'gallery-main' },
    h('header', { class: 'gallery-head' }, h('div', {}, title, note), button('Replay', () => play(current), { class: 'primary' })),
    stage)));
play(current);
