import { VALUE } from '../../engine';
import type { Square } from '../../engine';
import {
  ARMY_MAX, FLOORS, PIECE_NAME, REROLL_COST,
  buyOffer, floorOf, moveUnit, priceOf, rerollShop, skipDraft, takeDraft,
} from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h, pieceEl, squareName } from '../dom';
import { counter } from '../effects';
import { amount, flashRelics, offerCard, relicList } from '../widgets';

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
  const { run } = screen, { meta } = app, spec = floorOf(run);
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
  const draft = run.draft && h('section', {},
    h('h2', {}, 'Select one reward'),
    h('div', { class: deal },
      run.draft.map((offer) => offerCard(offer, run, { label: 'Take', run: act(() => takeDraft(run, offer)) }))),
    button('Skip the reward', act(() => skipDraft(run))));
  const shop = h('section', {},
    h('h2', {}, 'Shop'),
    run.shop.length
      ? h('div', { class: acted?.rolled ? 'cards flip' : deal }, run.shop.map((offer) => {
        const cost = priceOf(offer, meta);
        return offerCard(offer, run, {
          label: `Buy for ${cost} gold`,
          disabled: run.gold < cost,
          run: act(() => buyOffer(run, meta, offer)),
        });
      }))
      : h('p', { class: 'dim' }, 'The shop is empty.'),
    button(`Get new items for ${REROLL_COST} gold`, act(() => rerollShop(run), true), { disabled: run.gold < REROLL_COST }));
  const enemy = [...run.enemy.pieces].sort((a, b) => VALUE[b.type] - VALUE[a.type] || (a.type === 'k' ? -1 : 1));
  return h('main', { class: 'panel camp' },
    h('header', {},
      h('h1', {}, 'Camp'),
      h('p', { class: 'gold-count purse' }, amount('gold', counter({ from: acted?.goldBefore ?? run.gold, to: run.gold })))),
    draft,
    shop,
    h('section', {},
      h('h2', {}, `Your army (${run.army.length} of ${ARMY_MAX})`),
      h('p', { class: 'dim' }, 'To move a piece, select the piece and then select a square.'),
      viewArmy(app, screen),
      h('h3', {}, 'Your relics'), relics),
    h('section', {},
      h('h2', {}, `Next: floor ${run.floor} of ${FLOORS.length}, ${spec.name}${spec.boss ? ' (boss)' : ''}`),
      h('div', { class: 'taken' }, enemy.map((p) => pieceEl(p.type, 'b'))),
      run.enemy.traits.length ? relicList(run.enemy.traits, 'enemy') : null,
      button('Start the battle', () => app.startBattle(run), { class: 'primary', disabled: run.draft !== null }),
      run.draft && h('p', { class: 'dim' }, 'Select or skip the reward before the battle.')));
}
