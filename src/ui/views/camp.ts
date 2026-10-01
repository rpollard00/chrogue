import { VALUE } from '../../engine';
import type { Square } from '../../engine';
import {
  ARMY_MAX, FLOORS, PIECE_NAME, REROLL_COST,
  buyOffer, floorOf, moveUnit, priceOf, rerollShop, skipDraft, takeDraft,
} from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h, pieceEl, squareName } from '../dom';
import { offerCard, relicList } from '../widgets';

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
  app.render();
}

function viewArmy(app: App, screen: CampScreen): HTMLElement {
  const grid = h('div', { class: 'board homes' });
  for (let r = 1; r >= 0; r--) {
    for (let f = 0; f < 8; f++) {
      const s = r * 8 + f, unit = screen.run.army.find((u) => u.home === s);
      const classes = ['square', (f + r) % 2 ? 'light' : 'dark', s === screen.selected ? 'selected' : ''];
      const label = squareName(s) + (unit ? `, ${PIECE_NAME[unit.type].toLowerCase()}` : '');
      grid.append(h('button',
        { type: 'button', class: classes.join(' '), 'aria-label': label, onclick: () => clickHome(app, screen, s) },
        unit && pieceEl(unit.type, 'w')));
    }
  }
  return grid;
}

export function viewCamp(app: App, screen: CampScreen): HTMLElement {
  const { run } = screen, { meta } = app, spec = floorOf(run);
  // Runs a camp action, then saves and shows the result.
  const act = (action: () => void) => () => {
    action();
    app.saveRun();
    app.render();
  };
  const draft = run.draft && h('section', {},
    h('h2', {}, 'Select one reward'),
    h('div', { class: 'cards' },
      run.draft.map((offer) => offerCard(offer, run, { label: 'Take', run: act(() => takeDraft(run, offer)) }))),
    button('Skip the reward', act(() => skipDraft(run))));
  const shop = h('section', {},
    h('h2', {}, 'Shop'),
    run.shop.length
      ? h('div', { class: 'cards' }, run.shop.map((offer) => {
        const cost = priceOf(offer, meta);
        return offerCard(offer, run, {
          label: `Buy for ${cost} gold`,
          disabled: run.gold < cost,
          run: act(() => buyOffer(run, meta, offer)),
        });
      }))
      : h('p', { class: 'dim' }, 'The shop is empty.'),
    button(`Get new items for ${REROLL_COST} gold`, act(() => rerollShop(run)), { disabled: run.gold < REROLL_COST }));
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
      viewArmy(app, screen),
      h('h3', {}, 'Your relics'), relicList(run.relics, 'player')),
    h('section', {},
      h('h2', {}, `Next: floor ${run.floor} of ${FLOORS.length}, ${spec.name}${spec.boss ? ' (boss)' : ''}`),
      h('div', { class: 'taken' }, enemy.map((p) => pieceEl(p.type, 'b'))),
      run.enemy.traits.length ? relicList(run.enemy.traits, 'enemy') : null,
      button('Start the battle', () => app.startBattle(run), { class: 'primary', disabled: run.draft !== null }),
      run.draft && h('p', { class: 'dim' }, 'Select or skip the reward before the battle.')));
}
