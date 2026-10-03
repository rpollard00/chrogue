import { inCheck, kingSquare, movesFrom } from '../../engine';
import type { Color, Move, PieceId, PieceType, Square } from '../../engine';
import { FLOORS, PIECE_NAME, canScout, enemyMove, floorOf, playMove } from '../../game';
import type { BattleResult, MoveReport } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, glyph, h, pieceEl, squareName } from '../dom';
import { burst, counter, removeWhenDone, replay, tally } from '../effects';
import type { TallyRow } from '../effects';
import { enemyName, flashRelics, plaque, purse, relicList, stash } from '../widgets';
import type { PieceWell } from '../widgets';

type BattleScreen = ScreenOf<'battle'>;

// The delay lets the browser show the move of the player before the search starts.
const ENEMY_DELAY_MS = 350;

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
  clock: '50 moves passed with no capture. The battle is a draw.',
  bare: 'Only the kings remain. The battle is a draw.',
};

function viewResult(screen: BattleScreen, result: BattleResult, leave: () => void): HTMLElement {
  const { reward, winner } = result, rows: TallyRow[] = [];
  let rescued = 0;
  if (winner !== 'b') {
    rows.push({ label: 'Gold from captures', value: reward.captures });
    if (reward.clear) rows.push({ label: 'Gold for the win', value: reward.clear });
    for (const bonus of reward.bonuses) rows.push({ label: `Gold from ${bonus.label}`, value: bonus.gold });
    rescued = screen.view.battle.rescued.length;
  }
  const kind = winner === 'w' ? 'victory' : winner === 'b' ? 'defeat' : 'draw';
  return h('div', { class: `result ${kind}`, role: 'status' },
    h('h2', {}, winner === 'w' ? 'Victory' : winner === 'b' ? 'Defeat' : 'Draw', winner === 'w' && burst()),
    h('p', {}, winner === null ? DRAW_TEXT[result.reason] : WIN_TEXT[winner][result.reason]),
    rows.length > 0 && tally(rows, 'Total gold'),
    rescued > 0 && h('p', { class: 'dim' }, `Pieces that return to your army: ${rescued}`),
    button('Continue', leave, { class: 'primary' }));
}

// The position of a square in the piece layer. Each unit is the width of one square.
const placeAt = (s: Square): string => `${(s & 7) * 100}% ${(7 - (s >> 3)) * 100}%`;

// Makes the battle screen one time. After that, each click and each move changes only the elements that are different.
export function viewBattle(app: App, screen: BattleScreen): HTMLElement {
  const { run, view } = screen, { battle } = view, { state } = battle, spec = floorOf(run);
  const scout = canScout(app.meta);

  const squares: HTMLElement[] = [];
  const board = h('div', { class: 'board' });
  for (let r = 7; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f;
      squares[s] = h('button',
        { type: 'button', class: `square ${(f + r) % 2 ? 'light' : 'dark'}`, onclick: () => clickSquare(s) },
        f === 0 && h('span', { class: 'rank', 'aria-hidden': 'true' }, String(r + 1)),
        r === 0 && h('span', { class: 'file', 'aria-hidden': 'true' }, 'abcdefgh'[f]));
      board.append(squares[s]);
    }
  }
  // The pieces are in a layer above the squares. Each piece keeps its element for the full battle.
  const pieceLayer = h('div', { class: 'pieces', 'aria-hidden': 'true' });
  const shown = new Map<PieceId, { el: HTMLElement; type: PieceType }>();
  board.append(pieceLayer);

  // The lamp is lit when the player can move.
  const status = h('p', { class: 'lamp', role: 'status' });
  let shownStatus = '';
  // The picker is on the board, on the square of the promotion.
  const promotion = h('div', { class: 'promotion', role: 'group', 'aria-label': 'Promotion', hidden: true });
  let shownPromotion: Move[] | null = null;
  board.append(promotion);
  // The pieces that each side captured.
  const trays: Record<Color, PieceWell> = {
    w: stash('b', 'Pieces that you captured', 'Captured'),
    b: stash('w', 'Pieces that you lost', 'Lost'),
  };
  const captureGold = h('strong', {}, '0');
  const captureNote = h('small', { hidden: true }, '+', captureGold, ' from captures');
  let shownGold = 0;
  const giveUp = button('Give up', () => {
    if (confirm('The run will end. Give up?')) app.endRun(run, false);
  }, { class: 'quiet' });
  // The banner repeats the text of the enemy plaque, thus a screen reader ignores it.
  const intro = h('div', { class: spec.boss ? 'intro boss' : 'intro', 'aria-hidden': 'true' },
    h('p', {}, `Floor ${run.floor} of ${FLOORS.length}${spec.boss ? ' · Boss' : ''}`), h('strong', {}, spec.name));
  const wrap = h('div', { class: 'board-wrap' }, board, intro);
  const relics = relicList(run.relics, 'player');

  function syncSquares(): void {
    const check = inCheck(state, state.turn) ? kingSquare(state, state.turn) : -1;
    // The marks of an enemy piece have a different color.
    board.classList.toggle('scouting', state.board[view.selected]?.color === 'b');
    squares.forEach((el, s) => {
      const p = state.board[s], targets = view.targets.filter((m) => m.to === s);
      const capture = targets.length > 0 && (p !== null || targets.some((m) => m.epCapture));
      el.classList.toggle('selected', s === view.selected);
      el.classList.toggle('last', view.last !== null && (s === view.last.from || s === view.last.to));
      el.classList.toggle('check', s === check);
      el.classList.toggle('capture', capture);
      el.classList.toggle('target', targets.length > 0 && !capture);
      el.setAttribute('aria-label',
        squareName(s) + (p ? `, ${p.color === 'w' ? 'white' : 'black'} ${PIECE_NAME[p.type].toLowerCase()}` : ''));
    });
  }

  function syncPieces(): void {
    const onBoard = new Set<PieceId>();
    state.board.forEach((p, s) => {
      if (!p) return;
      onBoard.add(p.id);
      let piece = shown.get(p.id);
      if (!piece) {
        piece = { el: pieceEl(p.type, p.color), type: p.type };
        shown.set(p.id, piece);
        pieceLayer.append(piece.el);
      } else if (piece.type !== p.type) {
        piece.el.textContent = glyph(p.type);
        piece.type = p.type;
        replay(piece.el, 'promoted');
      }
      piece.el.style.translate = placeAt(s);
    });
    for (const [id, { el }] of shown) {
      if (onBoard.has(id)) continue;
      el.classList.add('gone');
      removeWhenDone(el);
      shown.delete(id);
    }
  }

  function syncPromotion(): void {
    if (view.promotion === shownPromotion) return;
    shownPromotion = view.promotion;
    promotion.hidden = shownPromotion === null;
    if (!shownPromotion) return;
    const { to } = shownPromotion[0];
    promotion.style.left = `${(to & 7) * 12.5}%`;
    promotion.style.top = `${(7 - (to >> 3)) * 12.5}%`;
    promotion.replaceChildren(...shownPromotion.flatMap((m) => (m.promo ? [h('button',
      { type: 'button', 'aria-label': PIECE_NAME[m.promo], onclick: () => commit(m) },
      pieceEl(m.promo, 'w'))] : [])));
    promotion.querySelector('button')?.focus();
  }

  function syncSide(): void {
    let text = 'Your move.';
    if (view.promotion) text = 'Select the new piece.';
    else if (view.busy) text = 'The enemy thinks';
    else if (inCheck(state, 'w')) text = 'Your king is in check.';
    // A screen reader reads the status again when its content changes, thus the content changes only with the text.
    if (text !== shownStatus) {
      const dots = view.busy && h('span', { class: 'dots', 'aria-hidden': 'true' }, [1, 2, 3].map(() => h('i', {}, '.')));
      status.replaceChildren(text, ...(dots ? [dots] : []));
      shownStatus = text;
    }
    status.classList.toggle('lit', !view.busy && !battle.result && state.turn === 'w');
    status.hidden = giveUp.hidden = battle.result !== null;
    syncPromotion();
    trays.w.show(battle.taken.w);
    trays.b.show(battle.taken.b);
    const gold = Math.round(battle.gold);
    if (gold !== shownGold) captureGold.replaceChildren(counter({ from: shownGold, to: gold }));
    shownGold = gold;
    captureNote.hidden = gold <= 0;
  }

  // Shows the gold of a capture above the square of the captured piece.
  function showCapture({ capture }: MoveReport): void {
    if (!capture || capture.gold <= 0) return;
    const floater = h('span', { class: 'floater' }, `+${Number(capture.gold.toFixed(1))}`);
    floater.style.translate = placeAt(capture.square);
    pieceLayer.append(floater);
    removeWhenDone(floater);
  }

  function sync(): void {
    syncSquares();
    syncPieces();
    syncSide();
  }

  function clickSquare(s: Square): void {
    if (view.busy || battle.result || view.promotion || state.turn !== 'w') return;
    const own = state.board[view.selected]?.color === 'w', color = state.board[s]?.color;
    const moves = own ? view.targets.filter((m) => m.to === s) : [];
    if (moves.length > 1) {
      view.promotion = moves;
    } else if (moves.length === 1) {
      return commit(moves[0]);
    } else if ((color === 'w' || (color === 'b' && scout)) && s !== view.selected) {
      view.selected = s;
      view.targets = movesFrom(state, s);
    } else {
      view.selected = -1;
      view.targets = [];
    }
    sync();
  }

  function commit(move: Move): void {
    // The picker goes away, thus the keyboard focus goes to the square of the new piece.
    const fromPicker = promotion.contains(document.activeElement);
    const report = playMove(battle, run, move);
    Object.assign(view, { last: move, selected: -1, targets: [], promotion: null });
    if (battle.result) {
      wrap.append(viewResult(screen, battle.result, app.settleBattle(run, battle)));
    } else if (state.turn === 'b') {
      view.busy = true;
      setTimeout(() => {
        // The player can leave the battle while the enemy waits.
        if (app.screen !== screen) return;
        view.busy = false;
        commit(enemyMove(battle, run));
      }, ENEMY_DELAY_MS);
    }
    sync();
    if (fromPicker) squares[move.to].focus();
    showCapture(report);
    const checked = squares.find((el) => el.classList.contains('check'));
    if (checked) replay(checked, 'alarm');
    flashRelics(relics, report.relics);
  }

  sync();
  const foe = plaque('enemy', 'Enemy', enemyName(run), relicList(run.enemy.traits, 'enemy'), trays.b.el);
  const me = plaque('player', 'You',
    h('div', { class: 'id' }, h('strong', { class: 'name' }, 'You'), status),
    trays.w.el, purse(String(run.gold), captureNote), relics, giveUp);
  return h('main', { class: 'battle' }, foe, wrap, me);
}
