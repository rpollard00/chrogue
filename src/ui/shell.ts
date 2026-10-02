// The application shell. It owns the state and the change of screens.
import { createBattle, finishRun, newRun } from '../game';
import type { Meta, Run } from '../game';
import type { App, Screen } from './app';
import { viewBattle } from './views/battle';
import { viewCamp } from './views/camp';
import { viewOver } from './views/over';
import { viewTitle } from './views/title';
import { viewUpgrades } from './views/upgrades';

/** The data at the start of the shell and the functions that save it. */
export interface Store {
  meta: Meta;
  run: Run | null;
  saveMeta(meta: Meta): void;
  saveRun(run: Run | null): void;
}

export function createApp(root: HTMLElement, store: Store): App {
  const { meta } = store;
  let { run } = store;
  let screen: Screen = { name: 'title' };

  function view(): HTMLElement {
    switch (screen.name) {
      case 'title': return viewTitle(app);
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
      store.saveRun(current);
      const battle = createBattle(current);
      app.show({ name: 'battle', run: current, view: { battle, selected: -1, targets: [], promotion: null, last: null, busy: false } });
    },
    openCamp(current: Run) {
      store.saveRun(current);
      app.show({ name: 'camp', run: current, selected: -1, cue: { kind: 'enter' }, reward: current.draft && { offers: current.draft, taken: null } });
    },
    endRun(current: Run, won: boolean) {
      const summary = finishRun(meta, current, won);
      store.saveMeta(meta);
      run = null;
      store.saveRun(null);
      app.show({ name: 'over', summary });
    },
    saveRun: () => store.saveRun(run),
    saveMeta: () => store.saveMeta(meta),
  };
  return app;
}
