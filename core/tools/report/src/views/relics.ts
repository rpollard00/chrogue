import {
  axisIndexes,
  excludesZero,
  formatDiff,
  formatValue,
  loadoutEffects,
  pairedDifference,
  type Dataset,
  type Estimate,
  type LoadoutEffect,
  type Metric,
  type Table,
} from "../analysis";
import type { Context, View } from "../app";
import { attachTip, button, h, svg } from "../dom";
import { axisHead, poleLabels, poleOf, scaleOf, whisker } from "../marks";
import { csvButton, sortableTable, type Column } from "../table";

interface Row {
  effect: LoadoutEffect;
  search: string;
  floors: readonly number[];
  byFloor: Estimate[];
  spread: number;
  baseline: boolean;
}

const MARK_WIDTH = 300;
const BAR = 9;
const PROFILE_HEIGHT = 22;

function profile(row: Row, ctx: Context, limit: number): SVGSVGElement {
  const width = row.byFloor.length * (BAR + 2);
  const mid = PROFILE_HEIGHT / 2;
  const root = svg("svg", { width, height: PROFILE_HEIGHT, viewBox: `0 0 ${width} ${PROFILE_HEIGHT}`, class: "profile", tabindex: 0, role: "img" });
  root.setAttribute("aria-label", `Difference from the baseline for each floor, ${row.effect.label}`);
  root.append(svg("line", { x1: 0, x2: width, y1: mid, y2: mid, class: "zero" }));
  row.byFloor.forEach((estimate, i) => {
    if (!Number.isFinite(estimate.mean)) return;
    const size = Math.max(1, (Math.abs(estimate.mean) / limit) * (mid - 1));
    const up = estimate.mean > 0;
    root.append(
      svg("rect", {
        x: i * (BAR + 2),
        y: up ? mid - size : mid,
        width: BAR,
        height: size,
        class: `bar pole-${poleOf(ctx.metric, estimate.mean)} ${excludesZero(estimate) ? "solid" : "open"}`,
      }),
    );
  });
  attachTip(root, () => ({
    title: `${row.effect.label}, difference for each floor`,
    rows: row.byFloor.map((estimate, i) => [
      `${formatDiff(ctx.metric, estimate.mean)} (${formatDiff(ctx.metric, estimate.lo)} to ${formatDiff(ctx.metric, estimate.hi)})`,
      `Floor ${ctx.ds.labels.floor[row.floors[i] ?? 0] ?? ""}`,
    ]),
  }));
  return root;
}

function compute(ctx: Context): Row[] {
  const { ds, filter, metric, state } = ctx;
  const floors = axisIndexes(ds, filter, "floor");
  return loadoutEffects(ds, { baseline: state.baseline, filter, metric }).map((effect) => {
    const byFloor = floors.map((floor) =>
      pairedDifference(ds, { axis: "relics", value: effect.loadout, against: state.baseline, filter: { ...filter, floor: [floor] }, metric }),
    );
    const means = byFloor.map((estimate) => estimate.mean).filter(Number.isFinite);
    return {
      effect,
      search: `${effect.label} ${ds.axes.relics[effect.loadout]?.join(" ") ?? ""}`.toLowerCase(),
      floors,
      byFloor,
      spread: means.length > 0 ? Math.max(...means) - Math.min(...means) : NaN,
      baseline: effect.loadout === state.baseline,
    };
  });
}

function relicText(ds: Dataset, loadout: number): string {
  return (ds.axes.relics[loadout] ?? []).map((key) => ds.relics.find((relic) => relic.key === key)?.text ?? key).join("\n");
}

function toTable(ds: Dataset, rows: readonly Row[], metric: Metric): Table {
  const floors = rows[0]?.floors ?? [];
  return {
    columns: ["loadout", "relics", "games", metric.key, "lo", "hi", "diff", "diff_lo", "diff_hi", "diff_se", "run_seeds", ...floors.map((floor) => `diff_floor_${ds.axes.floor[floor]}`)],
    rows: rows.map(({ effect, byFloor }) => [
      effect.label,
      ds.axes.relics[effect.loadout]?.join("+") ?? "",
      effect.summary.n,
      effect.summary.mean,
      effect.summary.lo,
      effect.summary.hi,
      effect.diff.mean,
      effect.diff.lo,
      effect.diff.hi,
      effect.diff.se,
      effect.diff.seeds,
      ...byFloor.map((estimate) => estimate.mean),
    ]),
  };
}

export function relicsView(): View {
  let ctx: Context | undefined;
  let rows: Row[] = [];
  let computedFor = "";
  let scale = scaleOf([]);
  let limit = 1;

  const columns: Column<Row>[] = [
    {
      key: "name",
      label: "Loadout",
      sort: (row) => row.effect.label,
      cell: (row) => {
        const name = button(row.effect.label, () => ctx?.set({ loadout: row.effect.loadout, view: "detail" }), {
          class: "link",
          title: ctx ? relicText(ctx.ds, row.effect.loadout) : "",
        });
        return row.baseline ? h("span", {}, name, h("span", { class: "tag", text: "baseline" })) : name;
      },
    },
    {
      key: "value",
      label: "Value",
      numeric: true,
      sort: (row) => row.effect.summary.mean,
      cell: (row) =>
        ctx
          ? h(
              "span",
              { title: `95% interval ${formatValue(ctx.metric, row.effect.summary.lo)} to ${formatValue(ctx.metric, row.effect.summary.hi)}` },
              formatValue(ctx.metric, row.effect.summary.mean),
            )
          : "",
    },
    {
      key: "diff",
      label: "Difference from the baseline",
      numeric: true,
      sort: (row) => (row.baseline ? NaN : row.effect.diff.mean),
      cell: (row) => {
        if (!ctx || row.baseline) return "";
        const { diff } = row.effect;
        return h(
          "span",
          { class: excludesZero(diff) ? "sure" : "unsure" },
          formatDiff(ctx.metric, diff.mean),
          h("span", { class: "interval", text: ` ${formatDiff(ctx.metric, diff.lo)} to ${formatDiff(ctx.metric, diff.hi)}` }),
        );
      },
    },
    {
      key: "mark",
      label: "Difference and its 95% interval",
      cell: (row) => (ctx && !row.baseline ? whisker(row.effect.diff, scale, ctx.metric, MARK_WIDTH) : ""),
    },
    { key: "games", label: "Games", numeric: true, sort: (row) => row.effect.summary.n, cell: (row) => row.effect.summary.n.toLocaleString("en-US") },
    {
      key: "spread",
      label: "For each floor",
      title: "The difference from the baseline for each floor. The sort order is the largest difference between two floors.",
      sort: (row) => (row.baseline ? NaN : row.spread),
      cell: (row) => (ctx && !row.baseline ? profile(row, ctx, limit) : ""),
    },
  ];

  const table = sortableTable(columns, (sort) => ctx?.set({ sort }));
  const search = h("input", { attrs: { type: "search", name: "query", placeholder: "Filter the loadouts", "aria-label": "Filter the loadouts" } });
  search.addEventListener("input", () => ctx?.set({ query: search.value }));
  const count = h("span", { class: "muted" });
  const direction = h("span", { class: "muted" });
  const legend = h(
    "span",
    { class: "keys" },
    h("span", { class: "key" }, h("i", { class: "swatch dot-solid" }), "The 95% interval excludes zero"),
    h("span", { class: "key" }, h("i", { class: "swatch dot-open" }), "The 95% interval includes zero"),
  );
  const el = h(
    "section",
    { class: "view" },
    h("div", { class: "toolbar" }, search, count, csvButton("relics.csv", () => (ctx ? toTable(ctx.ds, rows, ctx.metric) : { columns: [], rows: [] }))),
    h("div", { class: "legend" }, legend, direction),
    table.el,
  );

  return {
    el,
    update(next) {
      ctx = next;
      const key = JSON.stringify([next.state.metric, next.state.baseline, next.filter]);
      if (key !== computedFor) {
        rows = compute(next);
        computedFor = key;
        const others = rows.filter((row) => !row.baseline);
        scale = scaleOf(others.flatMap((row) => [row.effect.diff.lo, row.effect.diff.hi, row.effect.diff.mean]));
        limit = Math.max(1e-9, ...others.flatMap((row) => row.byFloor.map((estimate) => Math.abs(estimate.mean))).filter(Number.isFinite));
        table.setHead("mark", others.length > 0 ? axisHead(scale, MARK_WIDTH, (x) => formatDiff(next.metric, x)) : h("span"));
        const sides = poleLabels(next.metric);
        direction.textContent = next.metric.favors === "neither" ? "" : `A negative difference: ${sides.negative}. A positive difference: ${sides.positive}.`;
        table.setHead("value", h("span", { text: next.metric.label }));
      }
      if (search.value !== next.state.query) search.value = next.state.query;
      const query = next.state.query.trim().toLowerCase();
      const visible = rows.filter((row) => row.search.includes(query));
      count.textContent = `${visible.length} of ${rows.length} loadouts`;
      table.update(visible, next.state.sort, (row) => (row.effect.loadout === next.state.loadout ? "chosen" : ""));
    },
  };
}
