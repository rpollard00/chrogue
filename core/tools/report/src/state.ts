import { METRIC_KEYS, type Axis, type Dataset, type Filter, type MetricKey } from "./analysis";

export const VIEWS = ["relics", "levels", "floors", "combos", "detail"] as const;
export type ViewKey = (typeof VIEWS)[number];

export const FILTER_AXES = ["floor", "player", "enemy", "army", "traits"] as const satisfies readonly Axis[];
export type FilterAxis = (typeof FILTER_AXES)[number];

export interface Sort {
  key: string;
  descending: boolean;
}

export interface State {
  view: ViewKey;
  metric: MetricKey;
  baseline: number;
  loadout: number;
  compare: [number | null, number | null];
  filters: Record<FilterAxis, number[] | null>;
  sort: Sort;
  comboSort: Sort;
  comboSize: number | null;
  query: string;
  heat: "value" | "diff";
}

export function defaultState(ds: Dataset): State {
  const empty = ds.axes.relics.findIndex((keys) => keys.length === 0);
  const baseline = Math.max(0, empty);
  return {
    view: "relics",
    metric: "win",
    baseline,
    loadout: ds.size.relics > 1 ? (baseline === 0 ? 1 : 0) : 0,
    compare: [null, null],
    filters: { floor: null, player: null, enemy: null, army: null, traits: null },
    sort: { key: "diff", descending: true },
    comboSort: { key: "synergy", descending: true },
    comboSize: null,
    query: "",
    heat: "diff",
  };
}

export function filterOf(state: State): Filter {
  const filter: Filter = {};
  for (const axis of FILTER_AXES) {
    const kept = state.filters[axis];
    if (kept) filter[axis] = kept;
  }
  return filter;
}

function isView(value: string | null): value is ViewKey {
  return VIEWS.some((view) => view === value);
}

function isMetric(value: string | null): value is MetricKey {
  return METRIC_KEYS.some((key) => key === value);
}

function indexes(value: string | null, size: number): number[] | null {
  if (value === null) return null;
  const kept = [...new Set(value.split(".").map(Number))].filter((i) => Number.isInteger(i) && i >= 0 && i < size).sort((a, b) => a - b);
  return kept.length === 0 || kept.length === size ? null : kept;
}

function index(value: string | null, size: number): number | null {
  const i = value === null || value === "" ? NaN : Number(value);
  return Number.isInteger(i) && i >= 0 && i < size ? i : null;
}

function sortOf(value: string | null, fallback: Sort): Sort {
  if (!value) return fallback;
  return value.startsWith("-") ? { key: value.slice(1), descending: true } : { key: value, descending: false };
}

export function readHash(hash: string, ds: Dataset): State {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const base = defaultState(ds);
  const loadouts = ds.size.relics;
  const view = params.get("view");
  const metric = params.get("metric");
  const heat = params.get("heat");
  const [b, c] = (params.get("compare") ?? "").split(".");
  return {
    view: isView(view) ? view : base.view,
    metric: isMetric(metric) ? metric : base.metric,
    baseline: index(params.get("baseline"), loadouts) ?? base.baseline,
    loadout: index(params.get("loadout"), loadouts) ?? base.loadout,
    compare: [index(b ?? null, loadouts), index(c ?? null, loadouts)],
    filters: {
      floor: indexes(params.get("floor"), ds.size.floor),
      player: indexes(params.get("player"), ds.size.player),
      enemy: indexes(params.get("enemy"), ds.size.enemy),
      army: indexes(params.get("army"), ds.size.army),
      traits: indexes(params.get("traits"), ds.size.traits),
    },
    sort: sortOf(params.get("sort"), base.sort),
    comboSort: sortOf(params.get("combosort"), base.comboSort),
    comboSize: Number(params.get("combosize")) >= 2 ? Number(params.get("combosize")) : base.comboSize,
    query: params.get("q") ?? "",
    heat: heat === "value" ? "value" : base.heat,
  };
}

export function writeHash(state: State, ds: Dataset): string {
  const base = defaultState(ds);
  const params = new URLSearchParams();
  const sort = (value: Sort): string => (value.descending ? "-" : "") + value.key;
  const put = (name: string, value: string, fallback: string): void => {
    if (value !== fallback) params.set(name, value);
  };
  put("view", state.view, base.view);
  put("metric", state.metric, base.metric);
  put("baseline", String(state.baseline), String(base.baseline));
  put("loadout", String(state.loadout), String(base.loadout));
  put("compare", state.compare.map((i) => i ?? "").join("."), ".");
  for (const axis of FILTER_AXES) put(axis, state.filters[axis]?.join(".") ?? "", "");
  put("sort", sort(state.sort), sort(base.sort));
  put("combosort", sort(state.comboSort), sort(base.comboSort));
  put("combosize", String(state.comboSize ?? ""), "");
  put("q", state.query, "");
  put("heat", state.heat, base.heat);
  const text = params.toString();
  return text ? `#${text}` : "";
}
