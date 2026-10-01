import { UPGRADES, UPGRADE_IDS, buyUpgrade, nextCost } from '../../game';
import type { App, ScreenOf } from '../app';
import { button, h } from '../dom';
import { counter } from '../effects';

export function viewUpgrades(app: App, { bought }: ScreenOf<'upgrades'>): HTMLElement {
  const { meta } = app;
  const cards = UPGRADE_IDS.map((id) => {
    const def = UPGRADES[id], level = meta.upgrades[id] ?? 0, cost = nextCost(meta, id);
    const buy = () => {
      if (!buyUpgrade(meta, id)) return;
      app.saveMeta();
      app.show({ name: 'upgrades', bought: id });
    };
    const classes = ['card'];
    if (id === bought) classes.push('bought');
    if (cost === null) classes.push('maxed');
    const pips = def.costs.map((_, i) => {
      const pip = i >= level ? 'pip' : id === bought && i === level - 1 ? 'pip on new' : 'pip on';
      return h('span', { class: pip });
    });
    return h('div', { class: classes.join(' ') },
      h('h3', {}, def.name), h('p', {}, def.text),
      h('div', { class: 'level' },
        h('span', { class: 'pips', 'aria-hidden': 'true' }, pips),
        h('span', { class: 'dim' }, `Level ${level} of ${def.costs.length}`)),
      cost === null
        ? button('Maximum level', null, { disabled: true })
        : button(`Buy for ${cost} crowns`, buy, { disabled: meta.crowns < cost }));
  });
  const spent = bought ? UPGRADES[bought].costs[(meta.upgrades[bought] ?? 0) - 1] ?? 0 : 0;
  return h('main', { class: 'panel' },
    h('h2', {}, 'Upgrades'),
    h('p', {}, 'You have ', h('strong', { class: 'crown-count' }, counter({ from: meta.crowns + spent, to: meta.crowns })),
      ' crowns. Upgrades apply to each new run.'),
    h('div', { class: 'cards' }, cards),
    button('Back', () => app.show({ name: 'title' })));
}
