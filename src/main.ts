// The application shell. It owns the state, the saved data, and the change of screens.
import { createBattle, finishRun, loadMeta, loadRun, newRun, saveMeta, saveRun } from './game';
import type { Run } from './game';
import type { App, Screen } from './ui/app';
import { viewBattle } from './ui/views/battle';
import { viewCamp } from './ui/views/camp';
import { viewOver } from './ui/views/over';
import { viewHelp, viewTitle } from './ui/views/title';
import { viewUpgrades } from './ui/views/upgrades';

const root = document.getElementById('app');
if (!root) throw new Error('The page has no #app element');

const meta = loadMeta();
let run = loadRun();
let screen: Screen = { name: 'title' };

function view(): HTMLElement {
  switch (screen.name) {
    case 'title': return viewTitle(app);
    case 'help': return viewHelp(app);
    case 'upgrades': return viewUpgrades(app, screen);
    case 'battle': return viewBattle(app, screen);
    case 'camp': return viewCamp(app, screen);
    case 'over': return viewOver(app, screen);
  }
}

const app: App = {
  meta,
  get run() {
    return run;
  },
  get screen() {
    return screen;
  },
  show(next) {
    screen = next;
    app.render();
  },
  render() {
    root.replaceChildren(view());
  },
  startRun() {
    run = newRun(meta);
    app.startBattle(run);
  },
  startBattle(current: Run) {
    current.phase = 'battle';
    saveRun(current);
    const battle = createBattle(current);
    app.show({ name: 'battle', run: current, view: { battle, selected: -1, targets: [], promotion: null, last: null, busy: false } });
  },
  openCamp(current: Run) {
    saveRun(current);
    app.show({ name: 'camp', run: current, selected: -1 });
  },
  endRun(current: Run, won: boolean) {
    const summary = finishRun(meta, current, won);
    saveMeta(meta);
    run = null;
    saveRun(null);
    app.show({ name: 'over', summary });
  },
  saveRun: () => saveRun(run),
  saveMeta: () => saveMeta(meta),
};

app.render();
