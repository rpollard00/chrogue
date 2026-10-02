import { VALUE } from '../../engine';
import type { PieceType, Square } from '../../engine';
import {
  ARMY_MAX, PIECE_NAME, REROLL_COST,
  buyOffer, moveUnit, priceOf, rerollShop, skipDraft, takeDraft,
} from '../../game';
import type { Offer } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h, pieceEl, squareName } from '../dom';
import { counter } from '../effects';
import { actionButton, amount, enemyName, flashRelics, offerCard, pieceWell, plaque, purse, relicList, settledCard } from '../widgets';

type CampScreen = ScreenOf<'camp'>;

function clickHome(app: App, screen: CampScreen, s: Square): void {
  const { run } = screen;
  if (screen.selected < 0) {
    if (run.army.some((unit) => unit.home === s)) screen.selected = s;
  } else {
    moveUnit(run, screen.selected, s);
    screen.selected = -1;
    app.saveRun();
  }
  screen.cue = null;
  app.render();
}

function viewArmy(app: App, screen: CampScreen): HTMLElement {
  const added = screen.cue?.kind === 'acted' ? screen.cue.units : [];
  const grid = h('div', { class: 'board homes' });
  for (let r = 1; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f, unit = screen.run.army.find((u) => u.home === s);
      const classes = ['square', (f + r) % 2 ? 'light' : 'dark', s === screen.selected ? 'selected' : ''];
      const label = squareName(s) + (unit ? `, ${PIECE_NAME[unit.type].toLowerCase()}` : '');
      grid.append(h('button',
        { type: 'button', class: classes.join(' '), 'aria-label': label, onclick: () => clickHome(app, screen, s) },
        unit && h('span', { class: added.includes(unit.id) ? 'unit new' : 'unit' }, pieceEl(unit.type, 'w'))));
    }
  }
  return grid;
}

export function viewCamp(app: App, screen: CampScreen): HTMLElement {
  const { run } = screen, { meta } = app;
  const { cue } = screen, acted = cue?.kind === 'acted' ? cue : null;
  // Runs a camp action, then saves and shows the result. The cue tells the next view what the action changed.
  const act = (action: () => void, rolled = false) => () => {
    const goldBefore = run.gold, units = run.army.map((unit) => unit.id), relics = [...run.relics];
    action();
    screen.cue = {
      kind: 'acted', goldBefore, rolled,
      units: run.army.map((unit) => unit.id).filter((id) => !units.includes(id)),
      relics: run.relics.filter((id) => !relics.includes(id)),
    };
    app.saveRun();
    app.render();
  };
  const deal = cue?.kind === 'enter' ? 'cards deal' : 'cards';
  const relics = relicList(run.relics, 'player');
  if (acted) flashRelics(relics, acted.relics);
  // The reward shelf stays after the player selects, thus the table does not move.
  const { reward } = screen;
  const take = (offer: Offer) => act(() => {
    if (takeDraft(run, offer) && reward) reward.taken = offer;
  });
  // The shelf is on each visit, thus the table has one layout. A visit with no reward has an empty shelf.
  const draft = h('section', { class: 'shelf' },
    run.draft
      ? h('header', {}, h('h2', {}, 'Select one reward'), button('Skip the reward', act(() => skipDraft(run))))
      : h('header', {}, h('h2', {}, !reward ? 'Reward' : reward.taken ? 'Reward taken' : 'Reward skipped')),
    reward
      ? h('div', { class: deal },
        reward.offers.map((offer) => (run.draft
          ? offerCard(offer, run, { verb: 'Take', run: take(offer) })
          : settledCard(offer, offer === reward.taken))))
      : h('p', { class: 'dim' }, 'No reward to select.'));
  const shop = h('section', { class: 'shelf' },
    h('header', {},
      h('h2', {}, 'Shop'),
      actionButton({
        verb: 'Get new items', cost: { value: REROLL_COST, currency: 'gold' },
        run: act(() => rerollShop(run), true), disabled: run.gold < REROLL_COST,
      })),
    run.shop.length
      ? h('div', { class: acted?.rolled ? 'cards flip' : deal }, run.shop.map((offer) => {
        const cost = priceOf(offer, meta);
        return offerCard(offer, run, {
          verb: 'Buy',
          cost: { value: cost, currency: 'gold' },
          disabled: run.gold < cost,
          run: act(() => buyOffer(run, meta, offer)),
        });
      }))
      : h('p', { class: 'dim' }, 'The shop is empty.'));
  const enemy = pieceWell('b', 'Enemy pieces');
  // The king is first. The other pieces follow in the sequence of their values.
  const rank = (type: PieceType): number => (type === 'k' ? Infinity : VALUE[type]);
  enemy.show(run.enemy.pieces.map((p) => p.type).sort((a, b) => rank(b) - rank(a)));
  // The gold and the start key show two times. A wide screen shows them in the plaques. A narrow screen shows them in the bar at the bottom.
  const gold = () => counter({ from: acted?.goldBefore ?? run.gold, to: run.gold });
  const start = () => [
    button('Start the battle', () => app.startBattle(run), { class: 'primary', disabled: run.draft !== null }),
    run.draft && h('p', { class: 'reason' }, 'Select or skip the reward before the battle.'),
  ];
  return h('main', { class: 'camp' },
    h('h1', { class: 'sr-only' }, 'Camp'),
    plaque('enemy', 'Next enemy',
      enemyName(run, 'Next enemy'), relicList(run.enemy.traits, 'enemy'), enemy.el, h('div', { class: 'start' }, start())),
    h('div', { class: 'shelves' }, draft, shop),
    plaque('player', 'Your army and relics',
      h('div', { class: 'army' },
        h('h2', {}, 'Your army ', h('span', {}, `(${run.army.length} of ${ARMY_MAX})`)),
        h('p', { class: 'hint' }, 'To move a piece, select the piece and then select a square.')),
      viewArmy(app, screen),
      h('div', { class: 'relics' }, h('span', { class: 'kicker' }, 'Your relics'), relics),
      purse(gold())),
    h('div', { class: 'pin' }, h('span', { class: 'purse gold-count' }, amount('gold', gold())), start()));
}
