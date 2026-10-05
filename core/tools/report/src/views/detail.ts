import {
  DRAW,
  ENDS,
  LOST,
  METRICS,
  RESULTS,
  WON,
  formatDiff,
  formatValue,
  pairedDifference,
  selectCells,
  summarize,
  type Cell,
  type Dataset,
  type Metric,
  type Table,
} from "../analysis";
import type { Context, View } from "../app";
import { attachTip, h, svg } from "../dom";
import { niceCeil } from "../marks";
import { csvButton, sortableTable, type Column } from "../table";

const RESULT_CLASS = ["won", "draw", "lost"] as const;
const ROW_LIMIT = 400;
const HIST_WIDTH = 620;
const HIST_HEIGHT = 240;

function cellCount(n: number): string {
  return n === 1 ? "1 cell" : `${n} cells`;
}

function share(part: number, total: number): string {
  return total > 0 ? `${((part / total) * 100).toFixed(1)}%` : "–";
}

function endCounts(cells: readonly Cell[]): number[][] {
  const counts = ENDS.map(() => RESULTS.map(() => 0));
  for (const cell of cells) {
    for (let game = 0; game < cell.ends.length; game++) {
      const row = counts[cell.ends[game] ?? 0];
      const result = cell.results[game] ?? 0;
      if (row) row[result] = (row[result] ?? 0) + 1;
    }
  }
  return counts;
}

function legend(): HTMLElement {
  return h("div", { class: "legend" }, ...RESULTS.map((result, i) => h("span", { class: "key" }, h("i", { class: `swatch ${RESULT_CLASS[i]}` }), result.label)));
}

function endsChart(cells: readonly Cell[], baseCells: readonly Cell[]): HTMLElement {
  const counts = endCounts(cells);
  const base = endCounts(baseCells);
  const sum = (rows: number[][]): number => rows.flat().reduce((a, b) => a + b, 0);
  const total = sum(counts);
  const baseTotal = sum(base);
  const largest = Math.max(1, ...counts.map((row) => row.reduce((a, b) => a + b, 0)));
  return h(
    "table",
    { class: "bars" },
    h("thead", {}, h("tr", {}, h("th", { text: "End" }), h("th", {}), h("th", { class: "num", text: "Games" }), h("th", { class: "num", text: "Share" }), h("th", { class: "num", text: "Baseline" }))),
    h(
      "tbody",
      {},
      ...ENDS.map((end, e) => {
        const row = counts[e] ?? [];
        const games = row.reduce((a, b) => a + b, 0);
        const track = h("div", { class: "track" });
        row.forEach((n, result) => {
          if (n === 0) return;
          const segment = h("i", { class: `segment-bar ${RESULT_CLASS[result]}`, attrs: { style: `width:${(n / largest) * 100}%`, tabindex: "0" } });
          attachTip(segment, () => ({ title: `${end.label}, ${RESULTS[result]?.label.toLowerCase()}`, rows: [[n.toLocaleString("en-US"), "Games"], [share(n, total), "Share of all games"]] }));
          track.append(segment);
        });
        return h(
          "tr",
          {},
          h("th", { text: end.label, attrs: { scope: "row" } }),
          h("td", { class: "track-cell" }, track),
          h("td", { class: "num", text: games.toLocaleString("en-US") }),
          h("td", { class: "num", text: share(games, total) }),
          h("td", { class: "num muted", text: share((base[e] ?? []).reduce((a, b) => a + b, 0), baseTotal) }),
        );
      }),
    ),
  );
}

function histogram(cells: readonly Cell[], maxPlies: number): SVGSVGElement {
  const width = Math.max(5, Math.ceil(maxPlies / 30 / 5) * 5);
  const bins = Math.floor(maxPlies / width) + 1;
  const counts = Array.from({ length: bins }, () => [0, 0, 0]);
  for (const cell of cells) {
    for (let game = 0; game < cell.plies.length; game++) {
      const bin = counts[Math.min(bins - 1, Math.floor((cell.plies[game] ?? 0) / width))];
      const result = cell.results[game] ?? 0;
      if (bin) bin[result] = (bin[result] ?? 0) + 1;
    }
  }
  const left = 48;
  const bottom = 30;
  const top = 8;
  const plotW = HIST_WIDTH - left - 8;
  const plotH = HIST_HEIGHT - top - bottom;
  const peak = Math.ceil(niceCeil(Math.max(1, ...counts.map((bin) => bin.reduce((a, b) => a + b, 0)))) / 4) * 4;
  const step = plotW / bins;
  const y = (n: number): number => top + plotH * (1 - n / peak);
  const root = svg("svg", { viewBox: `0 0 ${HIST_WIDTH} ${HIST_HEIGHT}`, width: HIST_WIDTH, height: HIST_HEIGHT, role: "img" });
  root.setAttribute("aria-label", "The number of battles for each length in plies");
  for (let tick = 0; tick <= 4; tick++) {
    const n = (peak * tick) / 4;
    root.append(
      svg("line", { x1: left, x2: HIST_WIDTH - 8, y1: y(n), y2: y(n), class: tick === 0 ? "axis" : "grid" }),
      svg("text", { x: left - 6, y: y(n) + 4, class: "tick", "text-anchor": "end" }, Math.round(n).toLocaleString("en-US")),
    );
  }
  const labelEvery = Math.ceil(bins / 6);
  counts.forEach((bin, i) => {
    const x = left + step * i;
    let below = 0;
    bin.forEach((n, result) => {
      if (n === 0) return;
      const size = (n / peak) * plotH;
      root.append(svg("rect", { x: x + 1, y: y(below + n), width: Math.max(1, step - 2), height: Math.max(0, size - (below > 0 ? 2 : 0)), class: `fill ${RESULT_CLASS[result]}` }));
      below += n;
    });
    if (i % labelEvery === 0) root.append(svg("text", { x, y: HIST_HEIGHT - bottom + 16, class: "tick", "text-anchor": "middle" }, String(i * width)));
    const hit = svg("rect", { x, y: top, width: step, height: plotH, class: "hit", tabindex: 0 });
    hit.setAttribute("aria-label", `${i * width} to ${i * width + width - 1} plies`);
    attachTip(hit, () => ({
      title: `${i * width} to ${i * width + width - 1} plies`,
      rows: RESULTS.map((result, r): [string, string] => [(bin[r] ?? 0).toLocaleString("en-US"), result.label]),
    }));
    root.append(hit);
  });
  root.append(svg("text", { x: HIST_WIDTH - 8, y: HIST_HEIGHT - 2, class: "tick", "text-anchor": "end" }, "plies"));
  return root;
}

function tally(cell: Cell): [number, number, number] {
  const counts: [number, number, number] = [0, 0, 0];
  for (const result of cell.results) {
    if (result === WON) counts[0]++;
    else if (result === DRAW) counts[1]++;
    else if (result === LOST) counts[2]++;
  }
  return counts;
}

const CELL_METRICS: Metric[] = [METRICS.win, METRICS.score, METRICS.gold, METRICS.lost, METRICS.plies];

function cellTable(ds: Dataset, cells: readonly Cell[]): Table {
  return {
    columns: ["floor", "player", "enemy", "army", "traits", "games", "won", "draw", "lost", ...CELL_METRICS.map((metric) => metric.key), "recruits"],
    rows: cells.map((cell) => [
      ds.axes.floor[cell.at.floor] ?? "",
      ds.axes.player[cell.at.player] ?? "",
      ds.axes.enemy[cell.at.enemy] ?? "",
      ds.axes.army[cell.at.army] ?? "",
      ds.axes.traits[cell.at.traits] ?? "",
      cell.results.length,
      ...tally(cell),
      ...CELL_METRICS.map((metric) => summarize([cell], metric).mean),
      cell.recruits,
    ]),
  };
}

export function detailView(): View {
  let ctx: Context | undefined;
  let cells: Cell[] = [];
  const title = h("h2");
  const relics = h("ul", { class: "relic-list" });
  const tiles = h("div", { class: "tiles" });
  const ends = h("div");
  const lengths = h("div", { class: "chart" });
  const count = h("span", { class: "muted" });

  const text = (pick: (cell: Cell, ds: Dataset) => string | number) => (cell: Cell) => (ctx ? String(pick(cell, ctx.ds)) : "");
  const mean = (metric: Metric): Column<Cell> => ({
    key: metric.key,
    label: metric.label,
    numeric: true,
    sort: (cell) => summarize([cell], metric).mean,
    cell: (cell) => formatValue(metric, summarize([cell], metric).mean),
  });
  const columns: Column<Cell>[] = [
    { key: "floor", label: "Floor", sort: (cell) => cell.at.floor, cell: text((cell, ds) => ds.labels.floor[cell.at.floor] ?? "") },
    { key: "player", label: "Player", sort: (cell) => cell.at.player, cell: text((cell, ds) => `L${ds.axes.player[cell.at.player]}`) },
    { key: "enemy", label: "Enemy", sort: (cell) => cell.at.enemy, cell: text((cell, ds) => (ds.axes.enemy[cell.at.enemy] === "floor" ? "floor" : `L${ds.axes.enemy[cell.at.enemy]}`)) },
    { key: "army", label: "Army", sort: (cell) => cell.at.army, cell: text((cell, ds) => ds.axes.army[cell.at.army] ?? "") },
    { key: "traits", label: "Enemy traits", sort: (cell) => cell.at.traits, cell: text((cell, ds) => ds.axes.traits[cell.at.traits] ?? "") },
    { key: "wdl", label: "Won, draw, lost", numeric: true, sort: (cell) => tally(cell)[0], cell: (cell) => tally(cell).join(" · ") },
    ...CELL_METRICS.map(mean),
    { key: "recruits", label: "Recruits", numeric: true, sort: (cell) => cell.recruits, cell: (cell) => String(cell.recruits) },
  ];
  let sort = { key: "floor", descending: false };
  const table = sortableTable(columns, (next) => {
    sort = next;
    table.update(cells.slice(0, ROW_LIMIT), sort);
  });

  const el = h(
    "section",
    { class: "view" },
    title,
    relics,
    tiles,
    h("div", { class: "split" }, h("div", {}, h("h3", { text: "How the battles end" }), legend(), ends), h("div", {}, h("h3", { text: "Length of the battles" }), legend(), lengths)),
    h("div", { class: "toolbar" }, h("h3", { text: "Cells" }), count, csvButton("cells.csv", () => (ctx ? cellTable(ctx.ds, cells) : { columns: [], rows: [] }))),
    table.el,
  );

  return {
    el,
    update(next) {
      ctx = next;
      const { ds, filter, state } = next;
      cells = selectCells(ds, { ...filter, relics: [state.loadout] });
      const baseCells = selectCells(ds, { ...filter, relics: [state.baseline] });
      title.textContent = ds.labels.relics[state.loadout] ?? "";
      relics.replaceChildren(
        ...(ds.axes.relics[state.loadout] ?? []).flatMap((key) => {
          const relic = ds.relics.find((entry) => entry.key === key);
          return relic ? [h("li", {}, h("b", { text: relic.name }), ` ${relic.text}`)] : [];
        }),
      );

      const games = cells.reduce((sum, cell) => sum + cell.results.length, 0);
      const rate = (metric: Metric): HTMLElement => {
        const diff = pairedDifference(ds, { axis: "relics", value: state.loadout, against: state.baseline, filter, metric });
        return h(
          "div",
          { class: "tile" },
          h("span", { class: "tile-label", text: metric.label }),
          h("span", { class: "tile-value", text: formatValue(metric, summarize(cells, metric).mean) }),
          h("span", { class: "tile-note", text: state.loadout === state.baseline ? "baseline" : `${formatDiff(metric, diff.mean)} from the baseline` }),
        );
      };
      tiles.replaceChildren(
        h("div", { class: "tile" }, h("span", { class: "tile-label", text: "Games" }), h("span", { class: "tile-value", text: games.toLocaleString("en-US") }), h("span", { class: "tile-note", text: cellCount(cells.length) })),
        rate(METRICS.win),
        rate(METRICS.draw),
        rate(METRICS.loss),
        h(
          "div",
          { class: "tile" },
          h("span", { class: "tile-label", text: "Recruits" }),
          h("span", { class: "tile-value", text: cells.reduce((sum, cell) => sum + cell.recruits, 0).toLocaleString("en-US") }),
          h("span", { class: "tile-note", text: "units that relics added" }),
        ),
      );
      ends.replaceChildren(endsChart(cells, baseCells));
      lengths.replaceChildren(histogram(cells, ds.maxPlies));
      count.textContent = cells.length > ROW_LIMIT ? `The first ${ROW_LIMIT} of ${cells.length} cells. The CSV file has each cell.` : cellCount(cells.length);
      table.update(cells.slice(0, ROW_LIMIT), sort);
    },
  };
}
