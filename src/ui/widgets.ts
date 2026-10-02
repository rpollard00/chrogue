// Elements that more than one screen uses.
import type { Color, PieceType } from '../engine';
import { FLOORS, PIECE_NAME, RELICS, blockedReason, describeOffer, floorOf } from '../game';
import type { Offer, RelicId, Run } from '../game';
import { h, pieceEl } from './dom';
import type { Child } from './dom';
import { replay } from './effects';
import { currencyIcon, medal } from './icons';
import type { Art, Currency } from './icons';
import { infoTip } from './tip';

/** Shows an amount of gold or of crowns with the icon of the currency. */
export const amount = (currency: Currency, value: Node | string): HTMLElement =>
  h('span', { class: `amount ${currency}` }, currencyIcon(currency), h('span', { class: 'sr-only' }, currency === 'gold' ? 'Gold: ' : 'Crowns: '), value);

export type Side = 'player' | 'enemy';

/** A raised bar that holds the state of one side. Its medal shows the king of the side. */
export const plaque = (side: Side, label: string, ...kids: (Child | Child[])[]): HTMLElement =>
  h('section', { class: side === 'enemy' ? 'plaque foe' : 'plaque me', 'aria-label': label },
    h('span', { class: 'medal' }, pieceEl('k', side === 'enemy' ? 'b' : 'w')), ...kids);

/** The name of the enemy of the floor, the floor, and a badge on a boss floor. `kicker` is a small text above the name. */
export function enemyName(run: Run, kicker?: string): HTMLElement {
  const spec = floorOf(run);
  return h('div', { class: 'id' },
    kicker && h('span', { class: 'kicker' }, kicker),
    h('h2', { class: 'name' }, spec.name),
    h('span', { class: 'sub' }, `Floor ${run.floor} of ${FLOORS.length}`, spec.boss && h('span', { class: 'boss-badge' }, 'Boss')));
}

/** A recessed slot that shows the gold of the run. `note` is a text after the amount. */
export const purse = (value: Node | string, note?: Child): HTMLElement =>
  h('div', { class: 'well purse gold-count' }, amount('gold', value), note);

const ORDER: readonly PieceType[] = ['k', 'q', 'r', 'b', 'n', 'p'];

/** The names of some pieces with the count of each, such as "1 rook, 2 pawns". */
export const countPieces = (types: readonly PieceType[]): string =>
  ORDER.flatMap((type) => {
    const count = types.filter((t) => t === type).length;
    return count ? [`${count} ${PIECE_NAME[type].toLowerCase()}${count > 1 ? 's' : ''}`] : [];
  }).join(', ');

export interface PieceWell {
  el: HTMLElement;
  /** Shows the pieces. A call adds only the pieces that are new. */
  show(types: readonly PieceType[]): void;
}

/**
 * A well for pieces that are not on the board. Its lining has the brown of the board.
 * `label` names the group for a screen reader. `caption` is a visible text before the pieces.
 * With `slots`, the well has the width of that number of pieces from the start, thus it does not change the layout when a piece comes.
 * With no `slots`, the well is hidden while it has no piece.
 */
export function pieceWell(color: Color, label: string, caption?: string, slots?: number): PieceWell {
  const row = h('span', { class: 'taken', role: 'img' });
  const el = h('div', { class: 'well lined', hidden: slots === undefined, style: slots === undefined ? null : `--slots: ${slots}` },
    caption && h('span', { class: 'cap' }, caption), row);
  return {
    el,
    show(types) {
      for (let i = row.childElementCount; i < types.length; i++) row.append(pieceEl(types[i], color));
      row.setAttribute('aria-label', `${label}: ${countPieces(types) || 'none'}`);
      el.hidden = slots === undefined && types.length === 0;
    },
  };
}

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
  /** For a reward after the player selects: the card that the player took, or a card that the player passed. */
  settled?: 'chosen' | 'passed';
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
  const classes = ['card', face.stamp && 'blocked', face.bought && 'bought', face.maxed && 'maxed', face.settled];
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

/** A reward card after the player selects. The card has no function. */
export const settledCard = (offer: Offer, taken: boolean): HTMLElement =>
  card({ kind: offer.kind, art: offerArt(offer), ...describeOffer(offer), settled: taken ? 'chosen' : 'passed' },
    { verb: taken ? 'Taken' : 'Take', run: null, disabled: true });

export function offerCard(offer: Offer, run: Run, action: CardAction): HTMLElement {
  const info = describeOffer(offer), stamp = blockedReason(offer, run);
  return card({ kind: offer.kind, art: offerArt(offer), ...info, stamp }, { ...action, disabled: stamp !== null || action.disabled });
}

/** A list of relic tokens, or of enemy traits. The list is hidden when it has no item. */
export function relicList(ids: readonly RelicId[], side: Side): HTMLElement {
  const label = side === 'enemy' ? 'Enemy traits' : 'Your relics';
  return h('ul', { class: side === 'enemy' ? 'tokens foe' : 'tokens', 'aria-label': label, hidden: !ids.length }, ids.map((id) => {
    const relic = RELICS[id], text = side === 'enemy' ? relic.foeText ?? relic.text : relic.text;
    return h('li', { class: 'token', 'data-relic': id },
      medal({ kind: 'icon', id }),
      h('strong', {}, relic.name),
      infoTip(relic.name, text));
  }));
}

/** Starts the flash of some relics in a list that relicList made. */
export function flashRelics(list: HTMLElement, ids: readonly RelicId[]): void {
  for (const id of ids) {
    const item = list.querySelector<HTMLElement>(`[data-relic='${id}']`);
    if (item) replay(item, 'flash');
  }
}
