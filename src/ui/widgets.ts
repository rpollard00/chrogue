// Elements that more than one screen uses.
import { RELICS, blockedReason, describeOffer } from '../game';
import type { Offer, RelicId, Run } from '../game';
import { h } from './dom';
import { replay } from './effects';
import { currencyIcon, medal } from './icons';
import type { Art, Currency } from './icons';
import { infoTip } from './tip';

/** Shows an amount of gold or of crowns with the icon of the currency. */
export const amount = (currency: Currency, value: Node | string): HTMLElement =>
  h('span', { class: `amount ${currency}` }, currencyIcon(currency), h('span', { class: 'sr-only' }, currency === 'gold' ? 'Gold: ' : 'Crowns: '), value);

type CardKind = Offer['kind'] | 'upgrade';

const KIND_LABEL: Record<CardKind, string> = { piece: 'Unit', relic: 'Relic', gold: 'Gold', upgrade: 'Upgrade' };

export interface CardFace {
  kind: CardKind;
  art: Art;
  name: string;
  /** The text of the info button. null when the name tells all. */
  text: string | null;
  /** A short text across the card, for a card that the player cannot take. */
  stamp?: string | null;
  /** True if the player bought the card a moment ago. */
  bought?: boolean;
  /** True if the card has no more levels. */
  maxed?: boolean;
}

export interface CardAction {
  verb: string;
  cost?: { value: number; currency: Currency };
  /** null when the button has no function. */
  run: (() => void) | null;
  disabled?: boolean;
}

/** A button that shows its cost with the icon of the currency. */
export function actionButton({ verb, cost, run, disabled }: CardAction, props: Record<string, string> = {}): HTMLElement {
  return h('button',
    { type: 'button', onclick: run, disabled, 'aria-label': cost && `${verb} for ${cost.value} ${cost.currency}`, ...props },
    verb, cost && amount(cost.currency, String(cost.value)));
}

/** The one card of the game. Each kind has its color. `extra` goes between the name and the button. */
export function card(face: CardFace, action: CardAction, extra?: HTMLElement): HTMLElement {
  const classes = ['card', face.stamp && 'blocked', face.bought && 'bought', face.maxed && 'maxed'];
  return h('div', { class: classes.filter(Boolean).join(' '), 'data-kind': face.kind },
    h('div', { class: 'card-top' },
      h('span', { class: 'kind' }, KIND_LABEL[face.kind]),
      face.text !== null && infoTip(face.name, face.text)),
    medal(face.art),
    h('h3', {}, face.name),
    extra,
    face.stamp && h('p', { class: 'stamp' }, face.stamp),
    actionButton(action, { class: 'act' }));
}

function offerArt(offer: Offer): Art {
  switch (offer.kind) {
    case 'piece': return { kind: 'piece', type: offer.type };
    case 'relic': return { kind: 'icon', id: offer.id };
    case 'gold': return { kind: 'icon', id: 'coins' };
  }
}

export function offerCard(offer: Offer, run: Run, action: CardAction): HTMLElement {
  const info = describeOffer(offer), stamp = blockedReason(offer, run);
  return card({ kind: offer.kind, art: offerArt(offer), ...info, stamp }, { ...action, disabled: stamp !== null || action.disabled });
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

/** Starts the flash of some relics in a list that relicList made. */
export function flashRelics(list: HTMLElement, ids: readonly RelicId[]): void {
  for (const id of ids) {
    const item = list.querySelector<HTMLElement>(`[data-relic='${id}']`);
    if (item) replay(item, 'flash');
  }
}
