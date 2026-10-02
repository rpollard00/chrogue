import { FLOORS } from '../../game';
import type { App } from '../app';
import { button, h } from '../dom';
import type { Child } from '../dom';
import { currencyIcon } from '../icons';

/** A recessed slot with a label and a value. */
const stat = (label: string, ...value: Child[]): HTMLElement =>
  h('div', { class: 'well' }, h('dt', { class: 'cap' }, label), h('dd', {}, ...value));

export function viewTitle(app: App): HTMLElement {
  const { meta, run } = app;
  const resume = run && (() => (run.phase === 'camp' ? app.openCamp(run) : app.startBattle(run)));
  const start = () => {
    if (!run || confirm('Your current run will end. Start a new run?')) app.startRun();
  };
  return h('main', { class: 'front' },
    h('h1', { class: 'wordmark' }, 'Chrogue'),
    h('p', { class: 'tagline' }, 'Eight battles. One army. Every piece you lose stays lost.'),
    h('nav', { class: 'menu', 'aria-label': 'Main menu' },
      run && button(`Continue run (floor ${run.floor})`, resume, { class: 'primary' }),
      button('New run', start, { class: run ? '' : 'primary' }),
      button('Upgrades', () => app.show({ name: 'upgrades', bought: null }))),
    h('dl', { class: 'stats' },
      stat('Crowns', currencyIcon('crowns'), String(meta.crowns)),
      stat('Best', `${meta.best} of ${FLOORS.length} floors`),
      stat('Runs', String(meta.runs))));
}
