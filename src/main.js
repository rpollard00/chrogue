import { createState, legalMoves, makeMove, outcome, inCheck, chooseMove, kingSquare, VALUE } from './engine.js';
import {
  RELICS, RELIC_BY_ID, FLOORS, UPGRADES, PIECE_NAME, ARMY_MAX, REROLL_COST,
  generateEnemy, rollDraft, rollShop, price, startingArmy, freeHome,
} from './content.js';

const META_KEY = 'chrogue.meta';
const RUN_KEY = 'chrogue.run';
const TEMP_ID = 'temp';
// The text selector U+FE0E stops the browser from drawing the pawn as an emoji.
const GLYPH = Object.fromEntries(Object.entries({ k: '♚', q: '♛', r: '♜', b: '♝', n: '♞', p: '♟' })
  .map(([type, glyph]) => [type, glyph + '︎']));

const app = document.getElementById('app');

function load(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key)) ?? fallback;
  } catch {
    return fallback;
  }
}

let meta = { crowns: 0, best: 0, runs: 0, upgrades: {}, ...load(META_KEY, {}) };
let run = load(RUN_KEY, null);
let battle = null;
let screen = 'title';
let campSelected = -1;
let summary = null;

const saveMeta = () => localStorage.setItem(META_KEY, JSON.stringify(meta));
const saveRun = () => (run ? localStorage.setItem(RUN_KEY, JSON.stringify(run)) : localStorage.removeItem(RUN_KEY));

function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (key === 'class') el.className = value;
    else if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
    else if (value === true) el.setAttribute(key, '');
    else if (value !== false && value != null) el.setAttribute(key, value);
  }
  el.append(...kids.flat().filter((kid) => kid != null && kid !== false));
  return el;
}

const button = (label, onclick, props = {}) => h('button', { type: 'button', onclick, ...props }, label);
const pieceEl = (type, color) => h('span', { class: `piece ${color}`, 'aria-hidden': 'true' }, GLYPH[type]);
const squareName = (s) => 'abcdefgh'[s & 7] + ((s >> 3) + 1);
const hasRelic = (id) => run.relics.includes(id);

function go(next) {
  screen = next;
  render();
}

// --- Run ---

function startRun() {
  const army = startingArmy(meta.upgrades);
  run = {
    floor: 1,
    gold: 5 * (meta.upgrades.gold ?? 0),
    army,
    nextId: army.length + 1,
    relics: [],
    enemy: generateEnemy(1),
    phase: 'battle',
    draft: null,
    shop: [],
  };
  startBattle();
}

function endRun(won) {
  const cleared = won ? FLOORS.length : run.floor - 1;
  const crowns = cleared + (won ? 5 : 0);
  meta.crowns += crowns;
  meta.best = Math.max(meta.best, cleared);
  meta.runs++;
  saveMeta();
  summary = { won, cleared, crowns };
  run = null;
  battle = null;
  saveRun();
  go('over');
}

function addPiece(type) {
  const home = freeHome(run.army, type);
  if (run.army.length >= ARMY_MAX || home < 0) return false;
  run.army.push({ id: run.nextId++, type, home });
  return true;
}

function takeItem(item) {
  if (item.kind === 'piece') return addPiece(item.type);
  if (item.kind === 'relic') run.relics.push(item.id);
  else run.gold += item.amount;
  return true;
}

// --- Battle ---

function startBattle() {
  const pieces = run.army.map((a) => ({ id: a.id, type: a.type, color: 'w', square: a.home }));
  if (hasRelic('conscription')) {
    const free = [...Array(24).keys()].slice(8).find((s) => !pieces.some((p) => p.square === s));
    if (free != null) pieces.push({ id: TEMP_ID, type: 'p', color: 'w', square: free });
  }
  run.enemy.pieces.forEach((e, i) => pieces.push({ id: `e${i}`, type: e.type, color: 'b', square: e.square }));
  const flags = (ids) => Object.fromEntries(ids.map((id) => [id, true]));
  battle = {
    state: createState(pieces, { w: flags(run.relics), b: flags(run.enemy.traits) }),
    selected: -1,
    targets: [],
    promotion: null,
    last: null,
    busy: false,
    lost: [],
    saved: null,
    gold: 0,
    taken: { w: [], b: [] },
    result: null,
  };
  run.phase = 'battle';
  saveRun();
  go('battle');
}

function clickSquare(s) {
  const { state } = battle;
  if (battle.busy || battle.result || battle.promotion || state.turn !== 'w') return;
  const moves = battle.targets.filter((m) => m.to === s);
  if (moves.length > 1) {
    battle.promotion = moves;
  } else if (moves.length === 1) {
    return playMove(moves[0]);
  } else if (state.board[s]?.color === 'w' && s !== battle.selected) {
    battle.selected = s;
    battle.targets = legalMoves(state).filter((m) => m.from === s);
  } else {
    battle.selected = -1;
    battle.targets = [];
  }
  render();
}

function playMove(move) {
  const { state } = battle;
  const mover = state.turn;
  const { captured } = makeMove(state, move);
  Object.assign(battle, { last: move, selected: -1, targets: [], promotion: null });
  if (captured) {
    battle.taken[mover].push(captured.type);
    if (mover === 'w') {
      battle.gold += VALUE[captured.type] * (hasRelic('bounty') ? 1.5 : 1);
    } else if (hasRelic('secondWind') && battle.saved == null && captured.id !== TEMP_ID) {
      battle.saved = captured.id;
    } else {
      battle.lost.push(captured.id);
    }
  }
  const result = outcome(state);
  if (result) {
    finishBattle(result);
  } else if (state.turn === 'b') {
    battle.busy = true;
    // The delay lets the browser show the move of the player before the search starts.
    setTimeout(() => {
      const spec = FLOORS[run.floor - 1];
      const reply = chooseMove(state, { depth: spec.depth, noise: spec.noise });
      battle.busy = false;
      playMove(reply);
    }, 350);
  }
  render();
}

function finishBattle(result) {
  const won = result.winner === 'w';
  const reward = { captures: Math.round(battle.gold), clear: 0, interest: 0 };
  if (result.winner !== 'b') {
    if (won) reward.clear = 3 + run.floor;
    if (won && hasRelic('interest')) {
      reward.interest = Math.min(6, Math.floor((run.gold + reward.captures + reward.clear) / 5));
    }
  }
  battle.result = { ...result, reward };
}

function leaveBattle() {
  const { result, state } = battle;
  if (result.winner === 'b') return endRun(false);
  const { reward } = result;
  run.gold += reward.captures + reward.clear + reward.interest;
  run.army = run.army.filter((a) => !battle.lost.includes(a.id));
  for (const p of state.board) {
    const unit = p && run.army.find((a) => a.id === p.id);
    if (unit) unit.type = p.type;
  }
  if (run.floor === FLOORS.length && result.winner === 'w') return endRun(true);
  run.floor++;
  run.enemy = generateEnemy(run.floor);
  run.draft = result.winner === 'w' ? rollDraft(run.floor, run.relics) : null;
  run.shop = rollShop(run.floor, run.relics);
  run.phase = 'camp';
  battle = null;
  campSelected = -1;
  saveRun();
  go('camp');
}

// --- Views ---

function itemCard(item, action) {
  const full = item.kind === 'piece' && run.army.length >= ARMY_MAX;
  let icon, name, text;
  if (item.kind === 'piece') {
    icon = pieceEl(item.type, 'w');
    name = PIECE_NAME[item.type];
    text = full ? 'Your army is full.' : `Add this ${name.toLowerCase()} to your army.`;
  } else if (item.kind === 'relic') {
    icon = h('span', { class: 'icon', 'aria-hidden': 'true' }, '✦');
    ({ name, text } = RELIC_BY_ID[item.id]);
  } else {
    icon = h('span', { class: 'icon gold', 'aria-hidden': 'true' }, '●');
    name = `${item.amount} gold`;
    text = `Get ${item.amount} gold.`;
  }
  return h('div', { class: `card ${item.kind}` },
    icon, h('h3', {}, name), h('p', {}, text),
    button(action.label, action.run, { disabled: full || action.disabled }));
}

function relicList(ids, field, empty) {
  if (!ids.length) return h('p', { class: 'dim' }, empty);
  return h('ul', { class: 'relics' }, ids.map((id) =>
    h('li', {}, h('strong', {}, RELIC_BY_ID[id].name), ' ', RELIC_BY_ID[id][field])));
}

function viewTitle() {
  return h('main', { class: 'panel title' },
    h('h1', {}, 'Chrogue'),
    h('p', { class: 'tagline' }, 'Eight battles. One army. Every piece you lose stays lost.'),
    h('div', { class: 'menu' },
      run && button(`Continue run (floor ${run.floor})`, () => (run.phase === 'camp' ? go('camp') : startBattle()),
        { class: 'primary' }),
      button('New run', () => {
        if (!run || confirm('Your current run will end. Start a new run?')) startRun();
      }, { class: run ? '' : 'primary' }),
      button('Upgrades', () => go('upgrades')),
      button('How to play', () => go('help'))),
    h('p', { class: 'dim' }, `Crowns: ${meta.crowns} · Best: ${meta.best} of ${FLOORS.length} floors · Runs: ${meta.runs}`));
}

function viewHelp() {
  const rules = [
    'You play White. Your army stays with you from one battle to the next battle.',
    'A piece that the enemy captures is gone for the rest of the run.',
    'A pawn that you promote stays promoted.',
    'You win a battle when you checkmate the enemy king.',
    'You also win when the enemy king is alone, or when the enemy has no legal move.',
    'The same conditions apply to you. If you lose a battle, the run ends.',
    'If 50 moves pass with no capture and no pawn advance, the battle is a draw. You continue without a reward.',
    'Castling and en passant follow the usual chess rules.',
    'After each battle, you select a reward, buy pieces and relics, and arrange your first two ranks.',
    `Clear ${FLOORS.length} floors to win the run. Each run gives crowns. Crowns buy permanent upgrades.`,
  ];
  return h('main', { class: 'panel' },
    h('h2', {}, 'How to play'),
    h('ul', { class: 'rules' }, rules.map((rule) => h('li', {}, rule))),
    h('h2', {}, 'Relics'),
    relicList(RELICS.map((r) => r.id), 'text'),
    button('Back', () => go('title')));
}

function viewUpgrades() {
  const cards = UPGRADES.map((up) => {
    const level = meta.upgrades[up.id] ?? 0, cost = up.costs[level];
    const buy = () => {
      meta.crowns -= cost;
      meta.upgrades[up.id] = level + 1;
      saveMeta();
      render();
    };
    return h('div', { class: 'card' },
      h('h3', {}, up.name), h('p', {}, up.text),
      h('p', { class: 'dim' }, `Level ${level} of ${up.costs.length}`),
      cost == null
        ? button('Maximum level', null, { disabled: true })
        : button(`Buy for ${cost} crowns`, buy, { disabled: meta.crowns < cost }));
  });
  return h('main', { class: 'panel' },
    h('h2', {}, 'Upgrades'),
    h('p', {}, `You have ${meta.crowns} crowns. Upgrades apply to each new run.`),
    h('div', { class: 'cards' }, cards),
    button('Back', () => go('title')));
}

const RESULT_TEXT = {
  w: {
    checkmate: 'Checkmate. You won the battle.',
    rout: 'The enemy king is alone. You won the battle.',
    stalemate: 'The enemy has no legal move. You won the battle.',
  },
  b: {
    checkmate: 'Checkmate. The run ends.',
    rout: 'Your king is alone. The run ends.',
    stalemate: 'You have no legal move. The run ends.',
  },
  draw: {
    clock: '50 moves passed with no capture and no pawn advance. The battle is a draw.',
    bare: 'Only the kings remain. The battle is a draw.',
  },
};

function viewResult() {
  const { winner, reason, reward } = battle.result;
  const lines = [];
  if (winner !== 'b') {
    lines.push(`Gold from captures: ${reward.captures}`);
    if (reward.clear) lines.push(`Gold for the win: ${reward.clear}`);
    if (reward.interest) lines.push(`Gold from Interest: ${reward.interest}`);
    if (battle.saved != null) lines.push('Second Wind returned one piece.');
  }
  return h('div', { class: 'result', role: 'status' },
    h('h2', {}, winner === 'w' ? 'Victory' : winner === 'b' ? 'Defeat' : 'Draw'),
    h('p', {}, RESULT_TEXT[winner ?? 'draw'][reason]),
    lines.map((line) => h('p', { class: 'dim' }, line)),
    button('Continue', leaveBattle, { class: 'primary' }));
}

function viewBoard() {
  const { state } = battle;
  const check = inCheck(state, state.turn) ? kingSquare(state, state.turn) : -1;
  const board = h('div', { class: 'board' });
  for (let r = 7; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f, p = state.board[s];
      const target = battle.targets.some((m) => m.to === s);
      const classes = ['square', (f + r) % 2 ? 'light' : 'dark'];
      if (s === battle.selected) classes.push('selected');
      if (battle.last && (s === battle.last.from || s === battle.last.to)) classes.push('last');
      if (s === check) classes.push('check');
      if (target) classes.push(p || battle.targets.some((m) => m.to === s && m.epCapture) ? 'capture' : 'target');
      const label = squareName(s) + (p ? `, ${p.color === 'w' ? 'white' : 'black'} ${PIECE_NAME[p.type].toLowerCase()}` : '');
      board.append(h('button', { type: 'button', class: classes.join(' '), 'aria-label': label, onclick: () => clickSquare(s) },
        p && pieceEl(p.type, p.color),
        f === 0 && h('span', { class: 'rank', 'aria-hidden': 'true' }, String(r + 1)),
        r === 0 && h('span', { class: 'file', 'aria-hidden': 'true' }, 'abcdefgh'[f])));
    }
  }
  return board;
}

function viewBattle() {
  const { state } = battle, spec = FLOORS[run.floor - 1];
  let status = 'Your move.';
  if (battle.promotion) status = 'Select a piece for the promotion.';
  else if (battle.busy) status = 'The enemy thinks.';
  else if (inCheck(state, 'w')) status = 'Your king is in check.';
  const taken = (color, by) => h('div', { class: 'taken' },
    battle.taken[by].length ? battle.taken[by].map((type) => pieceEl(type, color)) : h('span', { class: 'dim' }, 'None'));
  const promotion = battle.promotion && h('div', { class: 'promotion' },
    battle.promotion.map((m) => h('button', { type: 'button', 'aria-label': PIECE_NAME[m.promo], onclick: () => playMove(m) },
      pieceEl(m.promo, 'w'))));
  return h('main', { class: 'battle' },
    h('div', { class: 'board-wrap' }, viewBoard(), battle.result && viewResult()),
    h('aside', { class: 'side' },
      h('p', { class: 'dim' }, `Floor ${run.floor} of ${FLOORS.length}${spec.boss ? ' · Boss' : ''}`),
      h('h2', {}, spec.name),
      !battle.result && h('p', { class: 'status', role: 'status' }, status),
      promotion,
      h('h3', {}, 'Enemy traits'), relicList(run.enemy.traits, 'foe', 'None'),
      h('h3', {}, 'Your relics'), relicList(run.relics, 'text', 'None'),
      h('h3', {}, 'Pieces that you captured'), taken('b', 'w'),
      h('h3', {}, 'Pieces that you lost'), taken('w', 'b'),
      h('p', {}, `Gold: ${run.gold} (+${Math.round(battle.gold)} from captures)`),
      !battle.result && button('Give up', () => {
        if (confirm('The run will end. Give up?')) endRun(false);
      })));
}

function clickHome(s) {
  const unit = run.army.find((a) => a.home === s);
  if (campSelected < 0) {
    if (unit) campSelected = s;
  } else {
    const selected = run.army.find((a) => a.home === campSelected);
    if (unit) unit.home = campSelected;
    selected.home = s;
    campSelected = -1;
    saveRun();
  }
  render();
}

function viewArmy() {
  const grid = h('div', { class: 'board homes' });
  for (let r = 1; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f, unit = run.army.find((a) => a.home === s);
      const classes = ['square', (f + r) % 2 ? 'light' : 'dark', s === campSelected ? 'selected' : ''];
      const label = squareName(s) + (unit ? `, ${PIECE_NAME[unit.type].toLowerCase()}` : '');
      grid.append(h('button', { type: 'button', class: classes.join(' '), 'aria-label': label, onclick: () => clickHome(s) },
        unit && pieceEl(unit.type, 'w')));
    }
  }
  return grid;
}

function viewCamp() {
  const spec = FLOORS[run.floor - 1], haggle = meta.upgrades.haggle ?? 0;
  const change = (fn) => () => {
    fn();
    saveRun();
    render();
  };
  const draft = run.draft && h('section', {},
    h('h2', {}, 'Select one reward'),
    h('div', { class: 'cards' }, run.draft.map((item) => itemCard(item, {
      label: 'Take',
      run: change(() => {
        if (takeItem(item)) run.draft = null;
      }),
    }))),
    button('Skip the reward', change(() => (run.draft = null))));
  const shop = h('section', {},
    h('h2', {}, 'Shop'),
    run.shop.length
      ? h('div', { class: 'cards' }, run.shop.map((item) => {
        const cost = price(item, haggle);
        return itemCard(item, {
          label: `Buy for ${cost} gold`,
          disabled: run.gold < cost || (item.kind === 'relic' && hasRelic(item.id)),
          run: change(() => {
            if (!takeItem(item)) return;
            run.gold -= cost;
            run.shop = run.shop.filter((other) => other !== item);
          }),
        });
      }))
      : h('p', { class: 'dim' }, 'The shop is empty.'),
    button(`Get new items for ${REROLL_COST} gold`, change(() => {
      run.gold -= REROLL_COST;
      run.shop = rollShop(run.floor, run.relics);
    }), { disabled: run.gold < REROLL_COST }));
  const enemy = [...run.enemy.pieces].sort((a, b) => VALUE[b.type] - VALUE[a.type] || (a.type === 'k' ? -1 : 1));
  return h('main', { class: 'panel camp' },
    h('header', {},
      h('h1', {}, 'Camp'),
      h('p', { class: 'gold-count' }, `Gold: ${run.gold}`)),
    draft,
    shop,
    h('section', {},
      h('h2', {}, `Your army (${run.army.length} of ${ARMY_MAX})`),
      h('p', { class: 'dim' }, 'To move a piece, select the piece and then select a square.'),
      viewArmy(),
      h('h3', {}, 'Your relics'), relicList(run.relics, 'text', 'None')),
    h('section', {},
      h('h2', {}, `Next: floor ${run.floor} of ${FLOORS.length}, ${spec.name}${spec.boss ? ' (boss)' : ''}`),
      h('div', { class: 'taken' }, enemy.map((p) => pieceEl(p.type, 'b'))),
      run.enemy.traits.length ? relicList(run.enemy.traits, 'foe') : null,
      button('Start the battle', startBattle, { class: 'primary', disabled: !!run.draft }),
      run.draft && h('p', { class: 'dim' }, 'Select or skip the reward before the battle.')));
}

function viewOver() {
  const { won, cleared, crowns } = summary;
  return h('main', { class: 'panel title' },
    h('h1', {}, won ? 'The Black King falls' : 'Your king fell'),
    h('p', {}, `You cleared ${cleared} of ${FLOORS.length} floors and got ${crowns} crowns.`),
    h('div', { class: 'menu' },
      button('New run', startRun, { class: 'primary' }),
      button('Upgrades', () => go('upgrades')),
      button('Title', () => go('title'))));
}

const VIEWS = { title: viewTitle, help: viewHelp, upgrades: viewUpgrades, battle: viewBattle, camp: viewCamp, over: viewOver };

function render() {
  app.replaceChildren(VIEWS[screen]());
}

render();
