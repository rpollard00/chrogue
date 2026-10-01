import { FLOORS, RELIC_IDS } from '../../game';
import type { App } from '../app';
import { button, h } from '../dom';
import { relicList } from '../widgets';

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
      button('Upgrades', () => app.show({ name: 'upgrades', bought: null })),
      button('How to play', () => app.show({ name: 'help' }))),
    h('p', { class: 'dim' }, `Crowns: ${meta.crowns} · Best: ${meta.best} of ${FLOORS.length} floors · Runs: ${meta.runs}`));
}

export function viewHelp(app: App): HTMLElement {
  const rules = [
    'You play White. Your army stays with you from one battle to the next battle.',
    'A piece that the enemy captures is gone for the rest of the run.',
    'A pawn that you promote stays promoted.',
    'You win a battle when you checkmate the enemy king.',
    'You also win when the enemy king is alone, or when the enemy has no legal move.',
    'The same conditions apply to you. If you lose a battle, the run ends.',
    'If 50 moves pass with no capture and no pawn advance, the battle is a draw. You continue without a reward.',
    'Castling and en passant follow the usual chess rules.',
    'After each battle, you select a reward, buy pieces and relics, and arrange your first two ranks.',
    `Clear ${FLOORS.length} floors to win the run. Each run gives crowns. Crowns buy permanent upgrades.`,
  ];
  return h('main', { class: 'panel' },
    h('h2', {}, 'How to play'),
    h('ul', { class: 'rules' }, rules.map((rule) => h('li', {}, rule))),
    h('h2', {}, 'Relics'),
    relicList(RELIC_IDS, 'player', true),
    button('Back', () => app.show({ name: 'title' })));
}
