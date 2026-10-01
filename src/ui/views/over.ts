import { FLOORS } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h } from '../dom';

export function viewOver(app: App, { summary }: ScreenOf<'over'>): HTMLElement {
  return h('main', { class: 'panel title' },
    h('h1', {}, summary.won ? 'The Black King falls' : 'Your king fell'),
    h('p', {}, `You cleared ${summary.cleared} of ${FLOORS.length} floors and got ${summary.crowns} crowns.`),
    h('div', { class: 'menu' },
      button('New run', () => app.startRun(), { class: 'primary' }),
      button('Upgrades', () => app.show({ name: 'upgrades' })),
      button('Title', () => app.show({ name: 'title' }))));
}
