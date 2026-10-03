// The debug menu, a dialog above the layout. It changes the saved data at no cost. The game page loads it on request.
import './debug.css';
import type { PieceType } from '../engine';
import {
  FLOORS, PIECE_NAME, RELICS, RELIC_IDS, UPGRADES, UPGRADE_IDS,
  addUnit, barRelic, canAddUnit, isBarred, loadBarred, removeUnit, saveBarred, setFloor, setRelic, setUpgradeLevel,
} from '../game';
import type { RecruitType, RelicId, Run, UpgradeId } from '../game';
import type { App } from './app';
import { button, h, pieceEl } from './dom';

const RECRUITS: readonly RecruitType[] = ['p', 'n', 'b', 'r', 'q'];
const TOGGLE_KEY = '`';
// The enemy plaque has space for the traits of a boss, thus the menu gives no more traits than a boss can have.
const TRAITS_MAX = Math.max(...FLOORS.map((spec) => spec.traits));

const pieceName = (type: PieceType): string => PIECE_NAME[type].toLowerCase();

/** A key that stays down while its item is on. */
function toggle(label: string, on: boolean, set: (on: boolean) => void): HTMLElement {
  const key = button(label, () => {
    const next = key.getAttribute('aria-pressed') !== 'true';
    key.setAttribute('aria-pressed', String(next));
    set(next);
  }, { 'aria-pressed': String(on) });
  return key;
}

/** A field for a whole number that is 0 or more. */
function field(label: string, value: number, set: (value: number) => void): HTMLElement {
  const input = document.createElement('input');
  Object.assign(input, { type: 'number', min: '0', step: '1', value: String(value) });
  const clean = (): number => Math.max(0, Math.floor(input.valueAsNumber));
  input.addEventListener('input', () => {
    if (Number.isFinite(input.valueAsNumber)) set(clean());
  });
  // The field shows the value that the menu saved.
  input.addEventListener('change', () => {
    if (Number.isFinite(input.valueAsNumber)) input.value = String(clean());
  });
  return h('label', { class: 'field' }, h('span', {}, label), input);
}

const group = (label: string, ...kids: (HTMLElement | HTMLElement[])[]): HTMLElement =>
  h('div', { class: 'group', role: 'group', 'aria-label': label }, h('h3', {}, label), ...kids);

const relicToggles = (ids: readonly RelicId[], isOn: (id: RelicId) => boolean, set: (id: RelicId, on: boolean) => void): HTMLElement =>
  h('div', { class: 'toggles' }, ids.map((id) => toggle(RELICS[id].name, isOn(id), (on) => set(id, on))));

/** Adds the debug menu to the page. The key in the corner of the screen, or the ` key, opens it. */
export function mountDebug(app: App): void {
  const { meta } = app;
  for (const id of loadBarred()) barRelic(id, true);

  const dialog = document.createElement('dialog');
  dialog.className = 'debug';
  dialog.setAttribute('aria-label', 'Debug menu');
  // True if the screen below the menu shows data that the menu changed.
  let stale = false;

  const metaChanged = (): void => {
    app.saveMeta();
    stale = true;
  };
  const runChanged = (): void => {
    app.saveRun();
    stale = true;
  };

  function upgradeRow(id: UpgradeId): HTMLElement {
    const { name, costs } = UPGRADES[id], level = h('output', {});
    const step = (by: number) => () => {
      setUpgradeLevel(meta, id, (meta.upgrades[id] ?? 0) + by);
      metaChanged();
      show();
    };
    const less = button('−', step(-1), { 'aria-label': `Decrease the level of ${name}` });
    const more = button('+', step(1), { 'aria-label': `Increase the level of ${name}` });
    function show(): void {
      const now = meta.upgrades[id] ?? 0;
      level.textContent = `${now} of ${costs.length}`;
      less.toggleAttribute('disabled', now === 0);
      more.toggleAttribute('disabled', now === costs.length);
      // A key that goes off gives the keyboard focus to the other key.
      if (document.activeElement === less && now === 0) more.focus();
      if (document.activeElement === more && now === costs.length) less.focus();
    }
    show();
    return h('li', {}, h('span', {}, name), less, level, more);
  }

  const progress = (): HTMLElement =>
    h('section', {},
      h('h2', {}, 'Progress'),
      field('Crowns', meta.crowns, (value) => {
        meta.crowns = value;
        metaChanged();
      }),
      group('Upgrades', h('ul', { class: 'levels' }, UPGRADE_IDS.map(upgradeRow))));

  function runSection(run: Run): HTMLElement {
    const floor = document.createElement('select');
    floor.append(...FLOORS.map((spec, i) => h('option', { value: String(i + 1), selected: i + 1 === run.floor }, `${i + 1}. ${spec.name}`)));
    const traits = h('div', {});
    const limitTraits = (): void => {
      const full = run.enemy.traits.length >= TRAITS_MAX;
      for (const key of traits.querySelectorAll('button')) key.disabled = full && key.getAttribute('aria-pressed') !== 'true';
    };
    function showTraits(): void {
      traits.replaceChildren(relicToggles(RELIC_IDS.filter((id) => RELICS[id].foeText), (id) => run.enemy.traits.includes(id), (id, on) => {
        setRelic(run.enemy.traits, id, on);
        runChanged();
        limitTraits();
      }));
      limitTraits();
    }
    floor.addEventListener('change', () => {
      setFloor(run, Number(floor.value));
      runChanged();
      showTraits();
    });

    const army = h('div', { class: 'well lined units' });
    const adders = RECRUITS.map((type) => h('button', {
      type: 'button', 'aria-label': `Add a ${pieceName(type)}`,
      onclick: () => {
        addUnit(run, type);
        runChanged();
        showArmy();
      },
    }, '+', pieceEl(type, 'w')));
    // focus is the position of the piece that gets the keyboard focus, or -1.
    function showArmy(focus = -1): void {
      army.replaceChildren(...run.army.map((unit) => h('button', {
        type: 'button', 'aria-label': `Remove the ${pieceName(unit.type)}`, disabled: unit.type === 'k',
        onclick: () => {
          const at = run.army.indexOf(unit);
          removeUnit(run, unit.id);
          runChanged();
          showArmy(Math.min(at, run.army.length - 1));
        },
      }, pieceEl(unit.type, 'w'))));
      const full = !canAddUnit(run), last = army.lastElementChild;
      // The keys that add a piece go off when the army is full. Then the last piece gets the keyboard focus.
      if (full && adders.includes(document.activeElement as HTMLElement) && last instanceof HTMLElement) last.focus();
      for (const adder of adders) adder.toggleAttribute('disabled', full);
      const piece = army.children[focus];
      if (piece instanceof HTMLElement) piece.focus();
    }
    showArmy();
    showTraits();

    return h('section', {},
      h('h2', {}, 'Run'),
      h('div', { class: 'fields' },
        h('label', { class: 'field' }, h('span', {}, 'Floor'), floor),
        field('Gold', run.gold, (value) => {
          run.gold = value;
          runChanged();
        })),
      group('Army', h('p', { class: 'hint' }, 'Select a piece to remove it.'), army, h('div', { class: 'toggles adders' }, adders)),
      group('Your relics', relicToggles(RELIC_IDS, (id) => run.relics.includes(id), (id, on) => {
        setRelic(run.relics, id, on);
        runChanged();
      })),
      group(`Enemy traits (${TRAITS_MAX} at most)`, traits));
  }

  const offers = (): HTMLElement =>
    h('section', {},
      h('h2', {}, 'Offers'),
      group('Relics that the game can offer', relicToggles(RELIC_IDS, (id) => !isBarred(id), (id, on) => {
        barRelic(id, !on);
        saveBarred(RELIC_IDS.filter(isBarred));
      })));

  function open(): void {
    const { run, screen } = app;
    stale = false;
    dialog.replaceChildren(
      h('header', {},
        h('h1', {}, 'Debug'),
        screen.name === 'battle' && h('p', { class: 'hint' }, 'If you make a change, the battle starts again when you close this menu.'),
        button('Close', () => dialog.close())),
      h('div', { class: 'sections' },
        progress(),
        run ? runSection(run) : h('section', {}, h('h2', {}, 'Run'), h('p', { class: 'dim' }, 'No run is in progress.')),
        offers()));
    dialog.showModal();
  }

  // The screen below the menu shows the changes only after the menu closes.
  dialog.addEventListener('close', () => {
    const { screen } = app;
    if (!stale) return;
    // The camp reads the run each time that it shows. A battle has a copy of the army, thus it starts again.
    // A battle with a result is in the saved data already, thus its screen stays.
    if (screen.name === 'battle') return screen.view.battle.result ? undefined : app.startBattle(screen.run);
    if (screen.name === 'camp') screen.cue = null;
    app.render();
  });
  document.addEventListener('keydown', (event) => {
    if (event.key !== TOGGLE_KEY) return;
    event.preventDefault();
    if (dialog.open) dialog.close();
    else open();
  });
  document.body.append(button('Debug', open, { class: 'debug-open quiet' }), dialog);
}
