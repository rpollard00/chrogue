export const AXES = ["floor", "player", "enemy", "army", "traits", "relics"] as const;
export type Axis = (typeof AXES)[number];

export type PairedAxis = Exclude<Axis, "floor" | "army">;

export interface Relic {
  key: string;
  name: string;
  text: string;
  kind: "rule" | "effect";
  trait: boolean;
}

export interface Level {
  number: number;
  name: string;
}

export interface Floor {
  number: number;
  name: string;
  level: number;
  budget: number;
  traits: number;
  boss: boolean;
}

export type EnemyLevel = "floor" | number;

export interface AxisValues {
  floor: number[];
  player: number[];
  enemy: EnemyLevel[];
  army: string[];
  traits: string[];
  relics: string[][];
}

export const RESULTS = [
  { code: "w", label: "Won" },
  { code: "d", label: "Draw" },
  { code: "l", label: "Lost" },
] as const;
export const WON = 0;
export const DRAW = 1;
export const LOST = 2;

export const ENDS = [
  { code: "m", label: "Checkmate" },
  { code: "s", label: "Stalemate" },
  { code: "r", label: "Rout" },
  { code: "b", label: "Bare kings" },
  { code: "c", label: "Move clock" },
  { code: "x", label: "Ply limit" },
] as const;

export interface Cell {
  at: Record<Axis, number>;
  results: Uint8Array;
  ends: Uint8Array;
  plies: Int32Array;
  gold: Float64Array;
  lost: Int32Array;
  recruits: number;
}

export interface Dataset {
  command: string;
  seed: number;
  games: number;
  maxPlies: number;
  gold: number;
  relics: Relic[];
  levels: Level[];
  floors: Floor[];
  axes: AxisValues;
  size: Record<Axis, number>;
  labels: Record<Axis, string[]>;
  cells: Cell[];
}

export type ParseResult = { kind: "ok"; dataset: Dataset } | { kind: "error"; message: string };

class FormatError extends Error {}

function fail(path: string, expected: string): never {
  throw new FormatError(`${path}: ${expected}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function record(value: unknown, path: string): Record<string, unknown> {
  return isRecord(value) ? value : fail(path, "must be an object");
}

function list(value: unknown, path: string): unknown[] {
  return Array.isArray(value) ? value : fail(path, "must be an array");
}

function text(value: unknown, path: string): string {
  return typeof value === "string" ? value : fail(path, "must be a string");
}

function flag(value: unknown, path: string): boolean {
  return typeof value === "boolean" ? value : fail(path, "must be true or false");
}

function number(value: unknown, path: string): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fail(path, "must be a number");
}

function count(value: unknown, path: string): number {
  const n = number(value, path);
  return Number.isInteger(n) && n >= 0 ? n : fail(path, "must be a whole number that is 0 or more");
}

function parseRelic(value: unknown, path: string): Relic {
  const r = record(value, path);
  const kind = text(r.kind, `${path}.kind`);
  if (kind !== "rule" && kind !== "effect") fail(`${path}.kind`, 'must be "rule" or "effect"');
  return {
    key: text(r.key, `${path}.key`),
    name: text(r.name, `${path}.name`),
    text: text(r.text, `${path}.text`),
    kind,
    trait: flag(r.trait, `${path}.trait`),
  };
}

function parseLevel(value: unknown, path: string): Level {
  const r = record(value, path);
  return { number: count(r.number, `${path}.number`), name: text(r.name, `${path}.name`) };
}

function parseFloor(value: unknown, path: string): Floor {
  const r = record(value, path);
  return {
    number: count(r.number, `${path}.number`),
    name: text(r.name, `${path}.name`),
    level: count(r.level, `${path}.level`),
    budget: number(r.budget, `${path}.budget`),
    traits: count(r.traits, `${path}.traits`),
    boss: flag(r.boss, `${path}.boss`),
  };
}

function nonEmpty<T>(values: T[], path: string): T[] {
  return values.length > 0 ? values : fail(path, "must have one value or more");
}

function parseAxes(value: unknown, relics: Relic[], floors: Floor[]): AxisValues {
  const r = record(value, "axes");
  const known = new Set(relics.map((relic) => relic.key));
  const floorNumbers = new Set(floors.map((floor) => floor.number));
  const each = <T>(name: string, parse: (item: unknown, path: string) => T): T[] =>
    nonEmpty(
      list(r[name], `axes.${name}`).map((item, i) => parse(item, `axes.${name}[${i}]`)),
      `axes.${name}`,
    );
  return {
    floor: each("floors", (item, path) => {
      const n = count(item, path);
      return floorNumbers.has(n) ? n : fail(path, `floor ${n} is not in content.floors`);
    }),
    player: each("players", count),
    enemy: each("enemies", (item, path) => (item === "floor" ? "floor" : count(item, path))),
    army: each("armies", text),
    traits: each("traits", text),
    relics: each("relics", (item, path) =>
      list(item, path).map((key, i) => {
        const k = text(key, `${path}[${i}]`);
        return known.has(k) ? k : fail(`${path}[${i}]`, `relic "${k}" is not in content.relics`);
      }),
    ),
  };
}

function codes(value: unknown, path: string, games: number, alphabet: readonly { code: string }[]): Uint8Array {
  const s = text(value, path);
  if (s.length !== games) fail(path, `has ${s.length} characters, but "games" is ${games}`);
  const out = new Uint8Array(games);
  for (let i = 0; i < games; i++) {
    const code = alphabet.findIndex((entry) => entry.code === s[i]);
    if (code < 0) fail(path, `has the unknown character "${s[i]}" at game ${i}`);
    out[i] = code;
  }
  return out;
}

function numbers<T extends Int32Array | Float64Array>(value: unknown, path: string, out: T): T {
  const items = list(value, path);
  if (items.length !== out.length) fail(path, `has ${items.length} values, but "games" is ${out.length}`);
  for (let i = 0; i < items.length; i++) out[i] = number(items[i], `${path}[${i}]`);
  return out;
}

export function cellIndex(size: Record<Axis, number>, at: Record<Axis, number>): number {
  let index = 0;
  for (const axis of AXES) index = index * size[axis] + at[axis];
  return index;
}

function parseCell(value: unknown, path: string, games: number, size: Record<Axis, number>): Cell {
  const r = record(value, path);
  const position = (axis: Axis): number => {
    const i = count(r[axis], `${path}.${axis}`);
    return i < size[axis] ? i : fail(`${path}.${axis}`, `index ${i} is not in the axis (${size[axis]} values)`);
  };
  return {
    at: {
      floor: position("floor"),
      player: position("player"),
      enemy: position("enemy"),
      army: position("army"),
      traits: position("traits"),
      relics: position("relics"),
    },
    results: codes(r.results, `${path}.results`, games, RESULTS),
    ends: codes(r.ends, `${path}.ends`, games, ENDS),
    plies: numbers(r.plies, `${path}.plies`, new Int32Array(games)),
    gold: numbers(r.gold, `${path}.gold`, new Float64Array(games)),
    lost: numbers(r.lost, `${path}.lost`, new Int32Array(games)),
    recruits: count(r.recruits, `${path}.recruits`),
  };
}

function parseCells(value: unknown, games: number, size: Record<Axis, number>): Cell[] {
  const items = list(value, "cells");
  const expected = AXES.reduce((product, axis) => product * size[axis], 1);
  if (items.length !== expected) fail("cells", `has ${items.length} cells, but the axes give ${expected}`);
  const cells = new Array<Cell | undefined>(expected);
  items.forEach((item, i) => {
    const cell = parseCell(item, `cells[${i}]`, games, size);
    const index = cellIndex(size, cell.at);
    if (cells[index]) fail(`cells[${i}]`, "a second cell has the same axis indexes");
    cells[index] = cell;
  });
  return cells.filter((cell) => cell !== undefined);
}

function levelLabel(levels: Level[], n: number): string {
  const level = levels.find((entry) => entry.number === n);
  return level ? `L${n} ${level.name}` : `L${n}`;
}

function relicName(relics: Relic[], key: string): string {
  return relics.find((relic) => relic.key === key)?.name ?? key;
}

function axisLabels(axes: AxisValues, relics: Relic[], levels: Level[], floors: Floor[]): Record<Axis, string[]> {
  return {
    floor: axes.floor.map((n) => `${n} ${floors.find((floor) => floor.number === n)?.name ?? ""}`.trim()),
    player: axes.player.map((n) => levelLabel(levels, n)),
    enemy: axes.enemy.map((n) => (n === "floor" ? "Level of the floor" : levelLabel(levels, n))),
    army: axes.army,
    traits: axes.traits.map((spec) =>
      spec === "floor"
        ? "Traits of the floor"
        : spec === "none"
          ? "No traits"
          : spec
              .split("+")
              .map((key) => relicName(relics, key))
              .join(" + "),
    ),
    relics: axes.relics.map((keys) =>
      keys.length === 0 ? "No relics" : keys.map((key) => relicName(relics, key)).join(" + "),
    ),
  };
}

export function parseBalance(input: unknown): ParseResult {
  try {
    const root = record(input, "file");
    if (root.format !== "chrogue-balance") fail("format", 'must be "chrogue-balance"');
    if (root.version !== 1) fail("version", `this report reads version 1, but the file has ${String(root.version)}`);
    const games = count(root.games, "games");
    const content = record(root.content, "content");
    const each = <T>(name: string, parse: (item: unknown, path: string) => T): T[] =>
      list(content[name], `content.${name}`).map((item, i) => parse(item, `content.${name}[${i}]`));
    const relics = each("relics", parseRelic);
    const levels = each("levels", parseLevel);
    const floors = each("floors", parseFloor);
    const axes = parseAxes(root.axes, relics, floors);
    const size: Record<Axis, number> = {
      floor: axes.floor.length,
      player: axes.player.length,
      enemy: axes.enemy.length,
      army: axes.army.length,
      traits: axes.traits.length,
      relics: axes.relics.length,
    };
    return {
      kind: "ok",
      dataset: {
        command: text(root.command, "command"),
        seed: number(root.seed, "seed"),
        games,
        maxPlies: count(root.max_plies, "max_plies"),
        gold: number(root.gold, "gold"),
        relics,
        levels,
        floors,
        axes,
        size,
        labels: axisLabels(axes, relics, levels, floors),
        cells: parseCells(root.cells, games, size),
      },
    };
  } catch (error) {
    if (error instanceof FormatError) return { kind: "error", message: error.message };
    throw error;
  }
}

export function parseBalanceText(json: string): ParseResult {
  let input: unknown;
  try {
    input = JSON.parse(json);
  } catch (error) {
    return { kind: "error", message: `The file is not JSON: ${error instanceof Error ? error.message : String(error)}` };
  }
  return parseBalance(input);
}

export interface Metric {
  key: string;
  label: string;
  unit: "share" | "points" | "gold" | "piece value" | "plies";
  interval: "wilson" | "normal";
  favors: "player" | "enemy" | "neither";
  value: (cell: Cell, game: number) => number;
}

const SCORES = [1, 0.5, 0];

export const METRICS = {
  win: {
    key: "win",
    label: "Win rate",
    unit: "share",
    interval: "wilson",
    favors: "player",
    value: (cell, game) => (cell.results[game] === WON ? 1 : 0),
  },
  score: {
    key: "score",
    label: "Score",
    unit: "points",
    interval: "normal",
    favors: "player",
    value: (cell, game) => SCORES[cell.results[game] ?? LOST] ?? 0,
  },
  loss: {
    key: "loss",
    label: "Loss rate",
    unit: "share",
    interval: "wilson",
    favors: "enemy",
    value: (cell, game) => (cell.results[game] === LOST ? 1 : 0),
  },
  draw: {
    key: "draw",
    label: "Draw rate",
    unit: "share",
    interval: "wilson",
    favors: "neither",
    value: (cell, game) => (cell.results[game] === DRAW ? 1 : 0),
  },
  gold: {
    key: "gold",
    label: "Gold reward",
    unit: "gold",
    interval: "normal",
    favors: "player",
    value: (cell, game) => cell.gold[game] ?? 0,
  },
  lost: {
    key: "lost",
    label: "Piece value lost",
    unit: "piece value",
    interval: "normal",
    favors: "enemy",
    value: (cell, game) => cell.lost[game] ?? 0,
  },
  plies: {
    key: "plies",
    label: "Length",
    unit: "plies",
    interval: "normal",
    favors: "neither",
    value: (cell, game) => cell.plies[game] ?? 0,
  },
} satisfies Record<string, Metric>;

export type MetricKey = keyof typeof METRICS;
export const METRIC_KEYS = Object.keys(METRICS).filter((key): key is MetricKey => key in METRICS);

export type Filter = Partial<Record<Axis, readonly number[]>>;

export function axisIndexes(ds: Dataset, filter: Filter, axis: Axis): readonly number[] {
  return filter[axis] ?? Array.from({ length: ds.size[axis] }, (_, i) => i);
}

function eachCell(ds: Dataset, filter: Filter, visit: (cell: Cell) => void): void {
  const picks = AXES.map((axis) => axisIndexes(ds, filter, axis));
  const walk = (depth: number, index: number): void => {
    if (depth === AXES.length) {
      const cell = ds.cells[index];
      if (cell) visit(cell);
      return;
    }
    const size = ds.size[AXES[depth] ?? "floor"];
    for (const i of picks[depth] ?? []) walk(depth + 1, index * size + i);
  };
  walk(0, 0);
}

export function selectCells(ds: Dataset, filter: Filter = {}): Cell[] {
  const cells: Cell[] = [];
  eachCell(ds, filter, (cell) => cells.push(cell));
  return cells;
}

export interface Group {
  at: Partial<Record<Axis, number>>;
  cells: Cell[];
}

export function groupCells(ds: Dataset, filter: Filter, by: readonly Axis[]): Group[] {
  const groups = new Map<string, Group>();
  eachCell(ds, filter, (cell) => {
    const key = by.map((axis) => cell.at[axis]).join(",");
    let group = groups.get(key);
    if (!group) {
      group = { at: Object.fromEntries(by.map((axis) => [axis, cell.at[axis]])), cells: [] };
      groups.set(key, group);
    }
    group.cells.push(cell);
  });
  return [...groups.values()];
}

const Z95 = 1.959964;

export interface Summary {
  n: number;
  mean: number;
  sd: number;
  lo: number;
  hi: number;
}

export function summarize(cells: readonly Cell[], metric: Metric): Summary {
  let n = 0;
  let sum = 0;
  let squares = 0;
  for (const cell of cells) {
    const games = cell.results.length;
    for (let game = 0; game < games; game++) {
      const x = metric.value(cell, game);
      sum += x;
      squares += x * x;
    }
    n += games;
  }
  if (n === 0) return { n, mean: NaN, sd: NaN, lo: NaN, hi: NaN };
  const mean = sum / n;
  const sd = n > 1 ? Math.sqrt(Math.max(0, squares - n * mean * mean) / (n - 1)) : NaN;
  if (metric.interval === "wilson") {
    const z2 = Z95 * Z95;
    const center = (mean + z2 / (2 * n)) / (1 + z2 / n);
    const half = (Z95 * Math.sqrt((mean * (1 - mean)) / n + z2 / (4 * n * n))) / (1 + z2 / n);
    return { n, mean, sd, lo: Math.max(0, center - half), hi: Math.min(1, center + half) };
  }
  const half = (Z95 * sd) / Math.sqrt(n);
  return { n, mean, sd, lo: mean - half, hi: mean + half };
}

export interface Estimate {
  n: number;
  seeds: number;
  mean: number;
  se: number;
  lo: number;
  hi: number;
}

export interface Term {
  index: number;
  weight: number;
}

export interface ContrastQuery {
  axis: PairedAxis;
  terms: readonly Term[];
  filter?: Filter;
  metric: Metric;
}

/**
 * The weighted sum of a metric over values of one axis, game by game, in each combination of the other axes.
 * Games with the same floor, army, and game number have the same run seed in each cell. Thus the standard error
 * comes from the mean of each run seed, not from each game.
 */
export function contrast(ds: Dataset, query: ContrastQuery): Estimate {
  const { axis, terms, metric } = query;
  const filter: Filter = { ...query.filter, [axis]: [0] };
  const floors = axisIndexes(ds, filter, "floor");
  const armies = axisIndexes(ds, filter, "army");
  const slot = (list: readonly number[]): Map<number, number> => new Map(list.map((value, i) => [value, i]));
  const floorSlot = slot(floors);
  const armySlot = slot(armies);
  const sums = new Float64Array(floors.length * armies.length * ds.games);
  const counts = new Uint32Array(sums.length);
  let n = 0;
  let total = 0;
  eachCell(ds, filter, (first) => {
    const cells = terms.map((term) => ds.cells[cellIndex(ds.size, { ...first.at, [axis]: term.index })]);
    const base = ((floorSlot.get(first.at.floor) ?? 0) * armies.length + (armySlot.get(first.at.army) ?? 0)) * ds.games;
    for (let game = 0; game < ds.games; game++) {
      let d = 0;
      for (let t = 0; t < terms.length; t++) {
        const cell = cells[t];
        const term = terms[t];
        if (cell && term) d += term.weight * metric.value(cell, game);
      }
      sums[base + game] = (sums[base + game] ?? 0) + d;
      counts[base + game] = (counts[base + game] ?? 0) + 1;
      total += d;
    }
    n += ds.games;
  });
  if (n === 0) return { n, seeds: 0, mean: NaN, se: NaN, lo: NaN, hi: NaN };
  const seeds = sums.length;
  const mean = total / n;
  let squares = 0;
  for (let i = 0; i < seeds; i++) {
    const d = (sums[i] ?? 0) / (counts[i] ?? 1) - mean;
    squares += d * d;
  }
  const se = seeds > 1 ? Math.sqrt(squares / (seeds - 1) / seeds) : NaN;
  return { n, seeds, mean, se, lo: mean - Z95 * se, hi: mean + Z95 * se };
}

export interface DifferenceQuery {
  axis: PairedAxis;
  value: number;
  against: number;
  filter?: Filter;
  metric: Metric;
}

export function pairedDifference(ds: Dataset, query: DifferenceQuery): Estimate {
  return contrast(ds, {
    axis: query.axis,
    terms: [
      { index: query.value, weight: 1 },
      { index: query.against, weight: -1 },
    ],
    filter: query.filter,
    metric: query.metric,
  });
}

export interface Pair {
  pair: number;
  a: number;
  b: number;
  none: number;
}

export function findPairs(ds: Dataset): Pair[] {
  const index = new Map(ds.axes.relics.map((keys, i) => [[...keys].sort().join("+"), i]));
  const none = index.get("");
  if (none === undefined) return [];
  return ds.axes.relics.flatMap((keys, pair) => {
    const [first, second] = keys;
    if (keys.length !== 2 || first === undefined || second === undefined) return [];
    const a = index.get(first);
    const b = index.get(second);
    return a === undefined || b === undefined ? [] : [{ pair, a, b, none }];
  });
}

export function synergy(ds: Dataset, query: { pair: Pair; filter?: Filter; metric: Metric }): Estimate {
  const { pair } = query;
  return contrast(ds, {
    axis: "relics",
    terms: [
      { index: pair.pair, weight: 1 },
      { index: pair.a, weight: -1 },
      { index: pair.b, weight: -1 },
      { index: pair.none, weight: 1 },
    ],
    filter: query.filter,
    metric: query.metric,
  });
}

export interface LoadoutEffect {
  loadout: number;
  label: string;
  summary: Summary;
  diff: Estimate;
}

export function loadoutEffects(
  ds: Dataset,
  query: { baseline: number; filter?: Filter; metric: Metric },
): LoadoutEffect[] {
  const filter = query.filter ?? {};
  return ds.axes.relics.map((_, loadout) => ({
    loadout,
    label: ds.labels.relics[loadout] ?? "",
    summary: summarize(selectCells(ds, { ...filter, relics: [loadout] }), query.metric),
    diff: pairedDifference(ds, { axis: "relics", value: loadout, against: query.baseline, filter, metric: query.metric }),
  }));
}

export function excludesZero(estimate: Estimate): boolean {
  return estimate.lo > 0 || estimate.hi < 0;
}

function fixed(x: number, digits: number): string {
  return Number.isFinite(x) ? x.toFixed(digits) : "–";
}

export function formatValue(metric: Metric, x: number): string {
  if (metric.unit === "share") return Number.isFinite(x) ? `${(x * 100).toFixed(1)}%` : "–";
  return fixed(x, metric.unit === "points" ? 3 : 2);
}

export function formatDiff(metric: Metric, x: number): string {
  if (!Number.isFinite(x)) return "–";
  const sign = x > 0 ? "+" : x < 0 ? "−" : "";
  const size = Math.abs(x);
  if (metric.unit === "share") return `${sign}${(size * 100).toFixed(1)} pp`;
  return `${sign}${size.toFixed(metric.unit === "points" ? 3 : 2)}`;
}

export interface Table {
  columns: string[];
  rows: (string | number)[][];
}

export function toCsv(table: Table): string {
  const field = (value: string | number): string => {
    if (typeof value === "number") return Number.isFinite(value) ? String(value) : "";
    return /[",\n\r]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
  };
  return [table.columns, ...table.rows].map((row) => row.map(field).join(",")).join("\n") + "\n";
}
