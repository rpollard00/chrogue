import { UPGRADES, UPGRADE_IDS, UPGRADE_SLOTS, buyUpgrade, nextCost } from '../../game';
import type { UpgradeId } from '../../game';
import type { App } from '../app';
import { button, h } from '../dom';
import { counter, replay } from '../effects';
import { UPGRADE_ART, medal } from '../icons';
import { actionButton, amount } from '../widgets';

/** The pips of the levels. `fresh` is the level that the player bought a moment ago. */
function pips(id: UpgradeId, level: number, fresh: number | null): HTMLElement[] {
  const marks = UPGRADES[id].costs.map((_, i) => h('span', { class: i >= level ? 'pip' : i + 1 === fresh ? 'pip on new' : 'pip on' }));
  return [
    h('span', { class: 'pips', 'aria-hidden': 'true' }, marks),
    h('span', { class: 'sr-only' }, `Level ${level} of ${marks.length}`),
  ];
}

/** The screen builds its elements one time. A selection or a purchase changes the elements in their place. */
export function viewUpgrades(app: App): HTMLElement {
  const { meta } = app;
  const levelOf = (id: UpgradeId): number => meta.upgrades[id] ?? 0;
  const canBuy = (id: UpgradeId): boolean => (nextCost(meta, id) ?? Infinity) <= meta.crowns;
  let selected = UPGRADE_IDS.find(canBuy) ?? UPGRADE_IDS[0];

  const purse = h('p', { class: 'well purse crown-count' }, amount('crowns', String(meta.crowns)));
  const panel = h('section', { class: 'detail', 'aria-live': 'polite', 'aria-label': 'The upgrade that you selected' });
  const slots = new Map(UPGRADE_IDS.map((id) => [id, h('button', { type: 'button', onclick: () => select(id) })]));

  function fillSlot(id: UpgradeId, fresh: number | null = null): void {
    const slot = slots.get(id);
    if (!slot) return;
    const level = levelOf(id), cost = nextCost(meta, id);
    // An upgrade that the player does not have and cannot buy is dim.
    slot.className = ['slot', cost === null && 'maxed', level === 0 && !canBuy(id) && 'far'].filter(Boolean).join(' ');
    slot.setAttribute('aria-pressed', String(id === selected));
    slot.replaceChildren(
      medal(UPGRADE_ART[id]),
      h('span', { class: 'name' }, UPGRADES[id].name),
      h('span', { class: 'state' }, pips(id, level, fresh), cost !== null && h('span', { class: canBuy(id) ? 'can' : '' }, amount('crowns', String(cost)))));
  }

  function fillPanel(fresh: number | null = null): void {
    const def = UPGRADES[selected], level = levelOf(selected), cost = nextCost(meta, selected);
    const mark = (i: number): string => (i < level ? 'Bought' : i === level ? 'Next' : '');
    panel.classList.toggle('maxed', cost === null);
    panel.replaceChildren(
      h('span', { class: 'kind' }, 'Upgrade'),
      medal(UPGRADE_ART[selected]),
      h('div', { class: 'id' },
        h('h2', {}, def.name),
        h('div', { class: 'lv' }, pips(selected, level, fresh), h('span', { 'aria-hidden': 'true' }, `Level ${level} of ${def.costs.length}`))),
      h('p', {}, def.text),
      h('ol', { class: 'well ladder', 'aria-label': 'The cost of each level' }, def.costs.map((value, i) =>
        h('li', { class: mark(i).toLowerCase() }, h('span', {}, `Level ${i + 1}`), amount('crowns', String(value)), h('span', {}, mark(i))))),
      actionButton(cost === null
        ? { verb: 'Max level', run: null, disabled: true }
        : { verb: 'Buy', cost: { value: cost, currency: 'crowns' }, run: buy, disabled: !canBuy(selected) }, { class: 'primary' }));
  }

  function select(id: UpgradeId): void {
    selected = id;
    for (const [other, slot] of slots) slot.setAttribute('aria-pressed', String(other === id));
    fillPanel();
  }

  function buy(): void {
    const before = meta.crowns;
    if (!buyUpgrade(meta, selected)) return;
    app.saveMeta();
    purse.replaceChildren(amount('crowns', counter({ from: before, to: meta.crowns })));
    // The crowns are fewer, thus each slot can change.
    for (const id of UPGRADE_IDS) fillSlot(id, id === selected ? levelOf(id) : null);
    fillPanel(levelOf(selected));
    replay(panel, 'bought');
    // The panel has a new key. The focus stays on the key, or goes to the slot when the key has no function.
    const key = panel.querySelector<HTMLElement>('button:not(:disabled)');
    (key ?? slots.get(selected))?.focus();
  }

  for (const id of UPGRADE_IDS) fillSlot(id);
  fillPanel();
  const empty = Array.from({ length: UPGRADE_SLOTS - slots.size }, () => h('div', { class: 'slot' }));
  return h('main', { class: 'upgrades' },
    h('header', {}, h('h1', {}, 'Upgrades'), purse),
    h('section', { class: 'shelf medal-board', 'aria-label': 'Upgrades' }, [...slots.values()], empty),
    panel,
    h('footer', {}, button('Back', () => app.show({ name: 'title' })), h('p', { class: 'hint' }, 'Upgrades apply to each new run.')));
}
