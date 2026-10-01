// The icon set. Each icon is a line drawing on a grid of 24 units. To add an icon, add one entry to `paths`.
import type { PieceType } from '../engine';
import type { RelicId, UpgradeId } from '../game';
import { h, pieceEl } from './dom';

// Each relic has an icon with the same id. The compiler reports a relic that has no icon.
const paths = {
  forcedMarch: 'M6 12l6-6 6 6M6 19l6-6 6 6',
  backpedal: 'M9 14l-4-4 4-4M5 10h9a5 5 0 0 1 0 10h-3',
  earlyPromo: 'M12 3l2.7 5.6 6.1.8-4.5 4.3 1.1 6.1L12 16.9l-5.4 2.9 1.1-6.1-4.5-4.3 6.1-.8z',
  kingKnight: 'M6 20V11a6 6 0 0 1 12 0v9h-4v-9a2 2 0 0 0-4 0v9z',
  longLeap: 'M7 21V7h11M14 3l4 4-4 4',
  sidestep: 'M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3',
  bounty: 'M12 5a7 7 0 1 0 0 14a7 7 0 0 0 0-14M12 2v5M12 17v5M2 12h5M17 12h5',
  secondWind: 'M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5',
  conscription: 'M6 21V4M6 5h12l-3 4 3 4H6',
  interest: 'M3 17l6-6 4 4 8-8M15 7h6v6',
  coins: 'M5 8c0-1.7 3.1-3 7-3s7 1.3 7 3-3.1 3-7 3-7-1.3-7-3zM5 8v4c0 1.7 3.1 3 7 3s7-1.3 7-3V8M5 12v4c0 1.7 3.1 3 7 3s7-1.3 7-3v-4',
  coin: 'M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18M12 8v8',
  crown: 'M4 18h16l1-10-5 4-4-7-4 7-5-4z',
  chest: 'M4 19v-9a4 4 0 0 1 4-4h8a4 4 0 0 1 4 4v9zM4 12h16M12 11v4',
  tag: 'M3 12V4h8l10 10-8 8zM7.5 8.5h.01',
} satisfies Record<RelicId, string> & Record<string, string>;

export type IconId = keyof typeof paths;

export function icon(id: IconId): HTMLElement {
  const el = h('span', { class: 'icon', 'aria-hidden': 'true' });
  el.innerHTML = `<svg viewBox="0 0 24 24"><path d="${paths[id]}"/></svg>`;
  return el;
}

/** The picture of a card or of a token: a chess piece or an icon. */
export type Art = { kind: 'piece'; type: PieceType } | { kind: 'icon'; id: IconId };

export const UPGRADE_ART: Record<UpgradeId, Art> = {
  pawn: { kind: 'piece', type: 'p' },
  gold: { kind: 'icon', id: 'chest' },
  bishop: { kind: 'piece', type: 'b' },
  haggle: { kind: 'icon', id: 'tag' },
};

/** A round disk that holds the picture. */
export const medal = (art: Art): HTMLElement =>
  h('span', { class: 'medal' }, art.kind === 'piece' ? pieceEl(art.type, 'w') : icon(art.id));

export type Currency = 'gold' | 'crowns';

const CURRENCY_ICON: Record<Currency, IconId> = { gold: 'coin', crowns: 'crown' };

export const currencyIcon = (currency: Currency): HTMLElement => {
  const el = icon(CURRENCY_ICON[currency]);
  el.classList.add(currency);
  return el;
};
