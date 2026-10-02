import { FLOORS } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h, pieceEl } from '../dom';
import { burst, tally } from '../effects';

// The tally starts after the king falls.
const TALLY_START_MS = 900;

export function viewOver(app: App, { summary }: ScreenOf<'over'>): HTMLElement {
  const { won, cleared, bonus, newBest } = summary;
  const rows = [{ label: 'Crowns for the floors', value: cleared }];
  if (won) rows.push({ label: 'Crowns for the win', value: bonus });
  return h('main', { class: `front over ${won ? 'won' : 'lost'}` },
    // The king that fell: the Black King after a win, your king after a defeat.
    h('div', { class: 'emblem' }, pieceEl('k', won ? 'b' : 'w'), won && burst(20)),
    h('h1', {}, won ? 'The Black King falls' : 'Your king fell'),
    h('p', {}, `You cleared ${cleared} of ${FLOORS.length} floors.`, newBest && ' ', newBest && h('strong', { class: 'badge' }, 'New best')),
    tally(rows, 'Total crowns', TALLY_START_MS),
    h('nav', { class: 'menu', 'aria-label': 'Menu' },
      button('New run', () => app.startRun(), { class: 'primary' }),
      button('Upgrades', () => app.show({ name: 'upgrades', bought: null })),
      button('Title', () => app.show({ name: 'title' }))));
}
