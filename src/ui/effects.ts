// Motion that more than one screen uses. Each effect shows its end state when the player prefers reduced motion.
import { h } from './dom';

const ROW_STEP_MS = 260;
const COUNT_MS = 600;

const reducedMotion = (): boolean => matchMedia('(prefers-reduced-motion: reduce)').matches;

/** A number that counts from `from` to `to`. A screen reader gets only the end value. */
export function counter({ to, from = 0, delay = 0 }: { to: number; from?: number; delay?: number }): HTMLElement {
  const animate = from !== to && !reducedMotion();
  const shown = h('span', { 'aria-hidden': 'true' }, String(animate ? from : to));
  const el = h('span', { class: 'counter' }, shown, h('span', { class: 'sr-only' }, String(to)));
  if (!animate) return el;
  let start = 0;
  const frame = (now: number) => {
    start ||= now + delay;
    const t = Math.min(1, Math.max(0, (now - start) / COUNT_MS));
    shown.textContent = String(Math.round(from + (to - from) * (1 - (1 - t) ** 3)));
    // The count stops when the screen changes.
    if (t < 1 && el.isConnected) requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  return el;
}

/** Removes the element when its animations end. With no animation, the element goes immediately. */
export function removeWhenDone(el: HTMLElement): void {
  void Promise.allSettled(el.getAnimations().map((animation) => animation.finished)).then(() => el.remove());
}

export interface TallyRow {
  label: string;
  value: number;
}

/**
 * Rows that come into view one after the other. A total row follows when there is more than one row.
 * startMs is the delay before the first row.
 */
export function tally(rows: TallyRow[], totalLabel: string, startMs = 0): HTMLElement {
  const row = ({ label, value }: TallyRow, i: number, total = false) => {
    const delay = startMs + i * ROW_STEP_MS;
    return h('div', { class: total ? 'tally-row total' : 'tally-row', style: `--delay: ${delay}ms` },
      h('dt', {}, label), h('dd', {}, counter({ to: value, delay })));
  };
  const sum = rows.reduce((total, { value }) => total + value, 0);
  return h('dl', { class: 'tally' },
    rows.map((r, i) => row(r, i)),
    rows.length > 1 && row({ label: totalLabel, value: sum }, rows.length, true));
}

/** Sparks that fly out from the center of the parent element. The parent must have a position. */
export function burst(count = 16): HTMLElement {
  return h('span', { class: 'burst', 'aria-hidden': 'true' },
    Array.from({ length: count }, (_, i) =>
      h('i', { style: `--angle: ${(360 / count) * i}deg; --reach: ${3 + (i % 3) * 1.25}rem` })));
}
