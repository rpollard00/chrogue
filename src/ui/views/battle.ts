import { inCheck, kingSquare, legalMoves } from '../../engine';
import type { Color, Move, Square } from '../../engine';
import { FLOORS, PIECE_NAME, enemyMove, floorOf, playMove, settleBattle } from '../../game';
import type { BattleResult } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h, pieceEl, squareName } from '../dom';
import { relicList } from '../widgets';

type BattleScreen = ScreenOf<'battle'>;

// The delay lets the browser show the move of the player before the search starts.
const ENEMY_DELAY_MS = 350;

function clickSquare(app: App, screen: BattleScreen, s: Square): void {
  const { view } = screen, { state } = view.battle;
  if (view.busy || view.battle.result || view.promotion || state.turn !== 'w') return;
  const moves = view.targets.filter((m) => m.to === s);
  if (moves.length > 1) {
    view.promotion = moves;
  } else if (moves.length === 1) {
    return commit(app, screen, moves[0]);
  } else if (state.board[s]?.color === 'w' && s !== view.selected) {
    view.selected = s;
    view.targets = legalMoves(state).filter((m) => m.from === s);
  } else {
    view.selected = -1;
    view.targets = [];
  }
  app.render();
}

function commit(app: App, screen: BattleScreen, move: Move): void {
  const { run, view } = screen;
  playMove(view.battle, run, move);
  Object.assign(view, { last: move, selected: -1, targets: [], promotion: null });
  if (!view.battle.result && view.battle.state.turn === 'b') {
    view.busy = true;
    setTimeout(() => {
      // The player can leave the battle while the enemy waits.
      if (app.screen !== screen) return;
      view.busy = false;
      commit(app, screen, enemyMove(view.battle, run));
    }, ENEMY_DELAY_MS);
  }
  app.render();
}

function leave(app: App, screen: BattleScreen): void {
  const { run, view } = screen;
  const next = settleBattle(run, view.battle);
  if (next === 'camp') app.openCamp(run);
  else app.endRun(run, next === 'won');
}

const WIN_TEXT: Record<Color, Record<'checkmate' | 'rout' | 'stalemate', string>> = {
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
};
const DRAW_TEXT: Record<'clock' | 'bare', string> = {
  clock: '50 moves passed with no capture and no pawn advance. The battle is a draw.',
  bare: 'Only the kings remain. The battle is a draw.',
};

function viewResult(app: App, screen: BattleScreen, result: BattleResult): HTMLElement {
  const { reward } = result, lines: string[] = [];
  if (result.winner !== 'b') {
    lines.push(`Gold from captures: ${reward.captures}`);
    if (reward.clear) lines.push(`Gold for the win: ${reward.clear}`);
    for (const bonus of reward.bonuses) lines.push(`Gold from ${bonus.label}: ${bonus.gold}`);
    const rescued = screen.view.battle.rescued.length;
    if (rescued) lines.push(`Pieces that return to your army: ${rescued}`);
  }
  return h('div', { class: 'result', role: 'status' },
    h('h2', {}, result.winner === 'w' ? 'Victory' : result.winner === 'b' ? 'Defeat' : 'Draw'),
    h('p', {}, result.winner === null ? DRAW_TEXT[result.reason] : WIN_TEXT[result.winner][result.reason]),
    lines.map((line) => h('p', { class: 'dim' }, line)),
    button('Continue', () => leave(app, screen), { class: 'primary' }));
}

function viewBoard(app: App, screen: BattleScreen): HTMLElement {
  const { view } = screen, { state } = view.battle;
  const check = inCheck(state, state.turn) ? kingSquare(state, state.turn) : -1;
  const board = h('div', { class: 'board' });
  for (let r = 7; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f, p = state.board[s];
      const targets = view.targets.filter((m) => m.to === s);
      const classes = ['square', (f + r) % 2 ? 'light' : 'dark'];
      if (s === view.selected) classes.push('selected');
      if (view.last && (s === view.last.from || s === view.last.to)) classes.push('last');
      if (s === check) classes.push('check');
      if (targets.length) classes.push(p || targets.some((m) => m.epCapture) ? 'capture' : 'target');
      const label = squareName(s) + (p ? `, ${p.color === 'w' ? 'white' : 'black'} ${PIECE_NAME[p.type].toLowerCase()}` : '');
      board.append(h('button',
        { type: 'button', class: classes.join(' '), 'aria-label': label, onclick: () => clickSquare(app, screen, s) },
        p && pieceEl(p.type, p.color),
        f === 0 && h('span', { class: 'rank', 'aria-hidden': 'true' }, String(r + 1)),
        r === 0 && h('span', { class: 'file', 'aria-hidden': 'true' }, 'abcdefgh'[f])));
    }
  }
  return board;
}

export function viewBattle(app: App, screen: BattleScreen): HTMLElement {
  const { run, view } = screen, { battle } = view, spec = floorOf(run);
  let status = 'Your move.';
  if (view.promotion) status = 'Select a piece for the promotion.';
  else if (view.busy) status = 'The enemy thinks.';
  else if (inCheck(battle.state, 'w')) status = 'Your king is in check.';
  const taken = (color: Color, by: Color) => h('div', { class: 'taken' },
    battle.taken[by].length ? battle.taken[by].map((type) => pieceEl(type, color)) : h('span', { class: 'dim' }, 'None'));
  const promotion = view.promotion && h('div', { class: 'promotion' },
    view.promotion.map((m) => m.promo && h('button',
      { type: 'button', 'aria-label': PIECE_NAME[m.promo], onclick: () => commit(app, screen, m) },
      pieceEl(m.promo, 'w'))));
  const giveUp = () => {
    if (confirm('The run will end. Give up?')) app.endRun(run, false);
  };
  return h('main', { class: 'battle' },
    h('div', { class: 'board-wrap' }, viewBoard(app, screen), battle.result && viewResult(app, screen, battle.result)),
    h('aside', { class: 'side' },
      h('p', { class: 'dim' }, `Floor ${run.floor} of ${FLOORS.length}${spec.boss ? ' · Boss' : ''}`),
      h('h2', {}, spec.name),
      !battle.result && h('p', { class: 'status', role: 'status' }, status),
      promotion,
      h('h3', {}, 'Enemy traits'), relicList(run.enemy.traits, 'enemy'),
      h('h3', {}, 'Your relics'), relicList(run.relics, 'player'),
      h('h3', {}, 'Pieces that you captured'), taken('b', 'w'),
      h('h3', {}, 'Pieces that you lost'), taken('w', 'b'),
      h('p', {}, `Gold: ${run.gold} (+${Math.round(battle.gold)} from captures)`),
      !battle.result && button('Give up', giveUp)));
}
