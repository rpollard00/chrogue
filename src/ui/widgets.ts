// Elements that more than one screen uses.
import { RELICS, blockedReason, describeOffer } from '../game';
import type { Offer, OfferIcon, RelicId, Run } from '../game';
import { button, h, pieceEl } from './dom';

function offerIcon(icon: OfferIcon): HTMLElement {
  if (icon === 'relic') return h('span', { class: 'icon relic', 'aria-hidden': 'true' }, '✦');
  if (icon === 'gold') return h('span', { class: 'icon gold', 'aria-hidden': 'true' }, '●');
  return pieceEl(icon, 'w');
}

export function offerCard(offer: Offer, run: Run, action: { label: string; run: () => void; disabled?: boolean }): HTMLElement {
  const info = describeOffer(offer), blocked = blockedReason(offer, run);
  return h('div', { class: 'card' },
    offerIcon(info.icon), h('h3', {}, info.name), h('p', {}, blocked ?? info.text),
    button(action.label, action.run, { disabled: blocked !== null || action.disabled }));
}

export function relicList(ids: readonly RelicId[], side: 'player' | 'enemy', empty = 'None'): HTMLElement {
  if (!ids.length) return h('p', { class: 'dim' }, empty);
  return h('ul', { class: 'relics' }, ids.map((id) => {
    const relic = RELICS[id];
    return h('li', {}, h('strong', {}, relic.name), ' ', side === 'enemy' ? relic.foeText ?? relic.text : relic.text);
  }));
}
