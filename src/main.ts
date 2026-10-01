// The game page. It connects the application shell to the saved data in localStorage.
import { loadMeta, loadRun, saveMeta, saveRun } from './game';
import { createApp } from './ui/shell';

const root = document.getElementById('app');
if (!root) throw new Error('The page has no #app element');

createApp(root, { meta: loadMeta(), run: loadRun(), saveMeta, saveRun }).render();
