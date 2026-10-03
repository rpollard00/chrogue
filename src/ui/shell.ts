// The application shell. It owns the state and the change of screens.
import { createBattle, finishRun, newRun, settleBattle } from '../game';
import type { Meta, Run, RunSummary } from '../game';
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
      case 'upgrades': return viewUpgrades(app);
      case 'battle': return viewBattle(app, screen);
      case 'camp': return viewCamp(app, screen);
      case 'over': return viewOver(app, screen);
    }
  }

  // Adds the run to the permanent data and removes it from the saved data.
  function closeRun(current: Run, won: boolean): RunSummary {
    const summary = finishRun(meta, current, won);
    store.saveMeta(meta);
    run = null;
    store.saveRun(null);
    return summary;
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
    settleBattle(current, battle) {
      const next = settleBattle(current, battle);
      if (next === 'camp') {
        store.saveRun(current);
        return () => app.openCamp(current);
      }
      const summary = closeRun(current, next === 'won');
      return () => app.show({ name: 'over', summary });
    },
    endRun(current: Run, won: boolean) {
      app.show({ name: 'over', summary: closeRun(current, won) });
    },
    saveRun: () => store.saveRun(run),
    saveMeta: () => store.saveMeta(meta),
  };
  return app;
}
