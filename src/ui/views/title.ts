import { FLOORS } from '../../game';
import type { App } from '../app';
import { button, h } from '../dom';

export function viewTitle(app: App): HTMLElement {
  const { meta, run } = app;
  const resume = run && (() => (run.phase === 'camp' ? app.openCamp(run) : app.startBattle(run)));
  const start = () => {
    if (!run || confirm('Your current run will end. Start a new run?')) app.startRun();
  };
  return h('main', { class: 'panel title' },
    h('h1', {}, 'Chrogue'),
    h('p', { class: 'tagline' }, 'Eight battles. One army. Every piece you lose stays lost.'),
    h('div', { class: 'menu' },
      run && button(`Continue run (floor ${run.floor})`, resume, { class: 'primary' }),
      button('New run', start, { class: run ? '' : 'primary' }),
      button('Upgrades', () => app.show({ name: 'upgrades', bought: null }))),
    h('p', { class: 'dim' }, `Crowns: ${meta.crowns} · Best: ${meta.best} of ${FLOORS.length} floors · Runs: ${meta.runs}`));
}
