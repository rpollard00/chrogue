// The game page. It connects the application shell to the saved data in localStorage.
import { loadMeta, loadRun, saveMeta, saveRun } from './game';
import { createApp } from './ui/shell';

const root = document.getElementById('app');
if (!root) throw new Error('The page has no #app element');

const app = createApp(root, { meta: loadMeta(), run: loadRun(), saveMeta, saveRun });
app.render();

// The development server always has the debug menu. A deployed game has it only with ?debug in the address.
if (import.meta.env.DEV || new URLSearchParams(location.search).has('debug')) {
  void import('./ui/debug').then(({ mountDebug }) => mountDebug(app));
}
