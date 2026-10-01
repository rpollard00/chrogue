import { UPGRADES, UPGRADE_IDS, buyUpgrade, nextCost } from '../../game';
import type { App } from '../app';
import { button, h } from '../dom';

export function viewUpgrades(app: App): HTMLElement {
  const { meta } = app;
  const cards = UPGRADE_IDS.map((id) => {
    const def = UPGRADES[id], level = meta.upgrades[id] ?? 0, cost = nextCost(meta, id);
    const buy = () => {
      if (!buyUpgrade(meta, id)) return;
      app.saveMeta();
      app.render();
    };
    return h('div', { class: 'card' },
      h('h3', {}, def.name), h('p', {}, def.text),
      h('p', { class: 'dim' }, `Level ${level} of ${def.costs.length}`),
      cost === null
        ? button('Maximum level', null, { disabled: true })
        : button(`Buy for ${cost} crowns`, buy, { disabled: meta.crowns < cost }));
  });
  return h('main', { class: 'panel' },
    h('h2', {}, 'Upgrades'),
    h('p', {}, `You have ${meta.crowns} crowns. Upgrades apply to each new run.`),
    h('div', { class: 'cards' }, cards),
    button('Back', () => app.show({ name: 'title' })));
}
