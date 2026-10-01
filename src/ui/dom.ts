// Small helpers that make DOM elements.
import type { Color, PieceType, Square } from '../engine';

export type Child = Node | string | null | undefined | false;
type Props = Record<string, string | boolean | (() => void) | null | undefined>;

export function h(tag: string, props: Props = {}, ...kids: (Child | Child[])[]): HTMLElement {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (typeof value === 'function') el.addEventListener(key.replace(/^on/, ''), value);
    else if (key === 'class' && typeof value === 'string') el.className = value;
    else if (value === true) el.setAttribute(key, '');
    else if (typeof value === 'string') el.setAttribute(key, value);
  }
  el.append(...kids.flat().filter((kid) => kid != null && kid !== false));
  return el;
}

export const button = (label: string, onclick: (() => void) | null, props: Props = {}): HTMLElement =>
  h('button', { type: 'button', onclick, ...props }, label);

// The text selector U+FE0E stops the browser from drawing the pawn as an emoji.
const GLYPH: Record<PieceType, string> = { k: '♚', q: '♛', r: '♜', b: '♝', n: '♞', p: '♟' };

export const glyph = (type: PieceType): string => GLYPH[type] + '︎';

export const pieceEl = (type: PieceType, color: Color): HTMLElement =>
  h('span', { class: `piece ${color}`, 'aria-hidden': 'true' }, glyph(type));

export const squareName = (s: Square): string => 'abcdefgh'[s & 7] + ((s >> 3) + 1);
