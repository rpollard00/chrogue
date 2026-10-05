import { METRICS, METRIC_KEYS, findCombos, type Combo, type Dataset, type Filter, type Metric } from "./analysis";
import { button, h } from "./dom";
import { FILTER_AXES, VIEWS, filterOf, readHash, writeHash, type FilterAxis, type State, type ViewKey } from "./state";
import { combosView } from "./views/combos";
import { detailView } from "./views/detail";
import { floorsView } from "./views/floors";
import { levelsView } from "./views/levels";
import { relicsView } from "./views/relics";

export interface Context {
  ds: Dataset;
  state: State;
  filter: Filter;
  metric: Metric;
  combos: readonly Combo[];
  set(patch: Partial<State>): void;
}

export interface View {
  el: HTMLElement;
  update(ctx: Context): void;
}

const VIEW_LABELS: Record<ViewKey, string> = {
  relics: "Relics",
  levels: "Levels",
  floors: "Floors",
  combos: "Combos",
  detail: "Loadout detail",
};

const AXIS_LABELS: Record<FilterAxis, string> = {
  floor: "Floors",
  player: "Player level",
  enemy: "Enemy level",
  army: "Army",
  traits: "Enemy traits",
};

function chipLabel(ds: Dataset, axis: FilterAxis, i: number): string {
  if (axis === "player") return `L${ds.axes.player[i]}`;
  if (axis === "enemy") return ds.axes.enemy[i] === "floor" ? "floor" : `L${ds.axes.enemy[i]}`;
  return String(ds.axes[axis][i]);
}

function select(label: string, options: readonly string[], onChange: (index: number) => void): { el: HTMLElement; input: HTMLSelectElement } {
  const input = h("select", { attrs: { name: label.toLowerCase() } }, ...options.map((text, i) => h("option", { text, attrs: { value: String(i) } })));
  input.addEventListener("change", () => onChange(Number(input.value)));
  return { el: h("label", { class: "field" }, h("span", { text: label }), input), input };
}

function header(ds: Dataset, openFile: () => void): HTMLElement {
  const games = ds.cells.length * ds.games;
  const facts: [string, string][] = [
    ["Seed", String(ds.seed)],
    ["Games for each cell", ds.games.toLocaleString("en-US")],
    ["Cells", ds.cells.length.toLocaleString("en-US")],
    ["Games", games.toLocaleString("en-US")],
    ["Ply limit", String(ds.maxPlies)],
    ["Gold before a battle", String(ds.gold)],
  ];
  const swept: [string, number][] = [
    ["floors", ds.size.floor],
    ["player levels", ds.size.player],
    ["enemy levels", ds.size.enemy],
    ["armies", ds.size.army],
    ["trait sets", ds.size.traits],
    ["loadouts", ds.size.relics],
  ];
  return h(
    "header",
    { class: "top" },
    h("div", { class: "top-row" }, h("h1", { text: "Balance report" }), h("code", { class: "command", text: ds.command }), button("Open a file", openFile, { class: "quiet" })),
    h(
      "dl",
      { class: "facts" },
      ...facts.map(([label, value]) => h("div", {}, h("dt", { text: label }), h("dd", { text: value }))),
      h(
        "div",
        {},
        h("dt", { text: "Swept axes" }),
        h("dd", {
          text:
            swept
              .filter(([, size]) => size > 1)
              .map(([label, size]) => `${size} ${label}`)
              .join(" × ") || "none",
        }),
      ),
    ),
  );
}

export function mountReport(root: HTMLElement, ds: Dataset, openFile: () => void): () => void {
  const combos = findCombos(ds);
  const views: Record<ViewKey, View> = {
    relics: relicsView(),
    levels: levelsView(),
    floors: floorsView(ds),
    combos: combosView(),
    detail: detailView(),
  };
  const shown = VIEWS.filter((view) => view !== "combos" || combos.length > 0);
  let state = readHash(location.hash, ds);

  const set = (patch: Partial<State>): void => {
    state = { ...state, ...patch };
    const hash = writeHash(state, ds);
    if (hash !== location.hash) history.replaceState(null, "", hash || location.pathname + location.search);
    render();
  };

  const metric = select(
    "Metric",
    METRIC_KEYS.map((key) => METRICS[key].label),
    (i) => set({ metric: METRIC_KEYS[i] ?? "win" }),
  );
  const baseline = select("Baseline", ds.labels.relics, (i) => set({ baseline: i }));
  const loadout = select("Loadout", ds.labels.relics, (i) => set({ loadout: i }));

  const toggle = (axis: FilterAxis, i: number): void => {
    const kept = state.filters[axis];
    const next = kept === null ? [i] : kept.includes(i) ? kept.filter((k) => k !== i) : [...kept, i].sort((a, b) => a - b);
    const all = next.length === 0 || next.length === ds.size[axis];
    set({ filters: { ...state.filters, [axis]: all ? null : next } });
  };

  const chips = new Map<FilterAxis, HTMLButtonElement[]>();
  const groups = FILTER_AXES.map((axis) => {
    const name = h("span", { class: "group-name", text: AXIS_LABELS[axis] });
    if (ds.size[axis] === 1) return h("div", { class: "group" }, name, h("span", { class: "fixed", text: ds.labels[axis][0] ?? "" }));
    const buttons = ds.labels[axis].map((label, i) => button(chipLabel(ds, axis, i), () => toggle(axis, i), { class: "chip", title: label }));
    chips.set(axis, buttons);
    return h("div", { class: "group", attrs: { role: "group", "aria-label": AXIS_LABELS[axis] } }, name, ...buttons);
  });
  const reset = button("All values", () => set({ filters: { floor: null, player: null, enemy: null, army: null, traits: null } }), {
    class: "quiet",
    title: "Remove each filter",
  });

  const tabs = new Map<ViewKey, HTMLButtonElement>(
    shown.map((view) => [view, button(VIEW_LABELS[view], () => set({ view }), { class: "tab", attrs: { role: "tab" } })]),
  );

  root.replaceChildren(
    header(ds, openFile),
    h(
      "section",
      { class: "controls", attrs: { "aria-label": "Controls for each view" } },
      h("div", { class: "pickers" }, metric.el, baseline.el, loadout.el),
      h("div", { class: "filters" }, ...groups, ...(chips.size > 0 ? [reset] : [])),
    ),
    h("nav", { class: "tabs", attrs: { role: "tablist" } }, ...tabs.values()),
    ...shown.map((view) => views[view].el),
  );

  function render(): void {
    const view = shown.includes(state.view) ? state.view : "relics";
    metric.input.value = String(METRIC_KEYS.indexOf(state.metric));
    baseline.input.value = String(state.baseline);
    loadout.input.value = String(state.loadout);
    for (const [axis, buttons] of chips) {
      const kept = state.filters[axis];
      buttons.forEach((chip, i) => chip.setAttribute("aria-pressed", String(kept === null || kept.includes(i))));
    }
    reset.disabled = FILTER_AXES.every((axis) => state.filters[axis] === null);
    for (const [key, tab] of tabs) tab.setAttribute("aria-selected", String(key === view));
    for (const key of shown) views[key].el.hidden = key !== view;
    views[view].update({ ds, state, filter: filterOf(state), metric: METRICS[state.metric], combos, set });
  }

  const listeners = new AbortController();
  window.addEventListener(
    "hashchange",
    () => {
      state = readHash(location.hash, ds);
      render();
    },
    { signal: listeners.signal },
  );
  render();
  return () => listeners.abort();
}
