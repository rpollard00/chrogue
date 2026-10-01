// Elements that more than one screen uses.
import { RELICS, blockedReason, describeOffer } from '../game';
import type { Offer, OfferIcon, RelicId, Run } from '../game';
import { button, h, pieceEl } from './dom';
import { replay } from './effects';
import { currencyIcon, medal } from './icons';
import type { Currency } from './icons';
import { infoTip } from './tip';

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

export function relicList(ids: readonly RelicId[], side: 'player' | 'enemy', detail = false): HTMLElement {
  if (!ids.length) return h('p', { class: 'dim' }, 'None');
  return h('ul', { class: side === 'enemy' ? 'tokens foe' : 'tokens' }, ids.map((id) => {
    const relic = RELICS[id], text = side === 'enemy' ? relic.foeText ?? relic.text : relic.text;
    return h('li', { class: detail ? 'token detail' : 'token', 'data-relic': id },
      medal({ kind: 'icon', id }),
      detail ? h('span', {}, h('strong', {}, relic.name), text) : h('strong', {}, relic.name),
      !detail && infoTip(relic.name, text));
  }));
}

/** Shows an amount of gold or of crowns with the icon of the currency. */
export const amount = (currency: Currency, value: Node | string): HTMLElement =>
  h('span', { class: `amount ${currency}` }, currencyIcon(currency), h('span', { class: 'sr-only' }, currency === 'gold' ? 'Gold: ' : 'Crowns: '), value);

/** Starts the flash of some relics in a list that relicList made. */
export function flashRelics(list: HTMLElement, ids: readonly RelicId[]): void {
  for (const id of ids) {
    const item = list.querySelector<HTMLElement>(`[data-relic='${id}']`);
    if (item) replay(item, 'flash');
  }
}
