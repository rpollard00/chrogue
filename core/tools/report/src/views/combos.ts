import {
  excludesZero,
  formatDiff,
  interaction,
  pairedDifference,
  sumOfParts,
  synergy,
  type Combo,
  type Estimate,
  type Table,
} from "../analysis";
import type { Context, View } from "../app";
import { attachTip, button, h } from "../dom";
import { axisHead, divergingFill, poleOf, scaleOf, whisker, type Scale } from "../marks";
import { csvButton, sortableTable, type Column } from "../table";

interface Row {
  combo: Combo;
  label: string;
  set: Estimate;
  parts: Estimate;
  synergy: Estimate;
  beyond: Estimate | null;
}

const MARK_WIDTH = 180;

export function combosView(): View {
  let ctx: Context | undefined;
  let rows: Row[] = [];
  let computedFor = "";
  let scale: Scale = scaleOf([]);

  const diff = (pick: (row: Row) => Estimate) => (row: Row) => (ctx ? formatDiff(ctx.metric, pick(row).mean) : "");
  const withInterval = (estimate: Estimate): Node | string =>
    ctx
      ? h(
          "span",
          { class: excludesZero(estimate) ? "sure" : "unsure" },
          formatDiff(ctx.metric, estimate.mean),
          h("span", { class: "interval", text: ` ${formatDiff(ctx.metric, estimate.lo)} to ${formatDiff(ctx.metric, estimate.hi)}` }),
        )
      : "";
  const columns: Column<Row>[] = [
    {
      key: "name",
      label: "Set",
      sort: (row) => row.label,
      cell: (row) => button(row.label, () => ctx?.set({ loadout: row.combo.loadout, view: "detail" }), { class: "link" }),
    },
    { key: "size", label: "Relics", numeric: true, sort: (row) => row.combo.size, cell: (row) => String(row.combo.size) },
    { key: "set", label: "Set", title: "The set against no relics", numeric: true, sort: (row) => row.set.mean, cell: diff((row) => row.set) },
    {
      key: "sum",
      label: "Sum of the parts",
      title: "The sum of each relic alone against no relics",
      numeric: true,
      sort: (row) => row.parts.mean,
      cell: diff((row) => row.parts),
    },
    {
      key: "synergy",
      label: "Synergy",
      title: "The set minus the sum of its parts",
      numeric: true,
      sort: (row) => row.synergy.mean,
      cell: (row) => withInterval(row.synergy),
    },
    { key: "mark", label: "Synergy and its 95% interval", cell: (row) => (ctx ? whisker(row.synergy, scale, ctx.metric, MARK_WIDTH) : "") },
    {
      key: "beyond",
      label: "Beyond smaller sets",
      title: "The part of the set that no smaller set of its relics explains",
      numeric: true,
      sort: (row) => row.beyond?.mean ?? NaN,
      cell: (row) => {
        if (!ctx || !row.beyond || row.combo.size < 3) return "";
        const { mean, lo, hi } = row.beyond;
        return h("span", {
          class: excludesZero(row.beyond) ? "sure" : "unsure",
          text: formatDiff(ctx.metric, mean),
          title: `95% interval: ${formatDiff(ctx.metric, lo)} to ${formatDiff(ctx.metric, hi)}`,
        });
      },
    },
    { key: "games", label: "Games", numeric: true, sort: (row) => row.set.n, cell: (row) => row.set.n.toLocaleString("en-US") },
  ];
  const table = sortableTable(columns, (comboSort) => ctx?.set({ comboSort }));
  const matrix = h("table", { class: "heat div" });
  const matrixBox = h("div", { class: "matrix" }, h("h3", { text: "Synergy of each pair" }), matrix);
  const title = h("h2");
  const sizes = h("div", { class: "group", attrs: { role: "group", "aria-label": "Relics in the set" } });
  const count = h("span", { class: "muted" });

  const toTable = (): Table => ({
    columns: ["set", "relics", "games", "set_diff", "sum_of_parts", "synergy", "synergy_lo", "synergy_hi", "synergy_se", "beyond", "beyond_lo", "beyond_hi"],
    rows: rows.map((row) => [
      row.label,
      row.combo.size,
      row.set.n,
      row.set.mean,
      row.parts.mean,
      row.synergy.mean,
      row.synergy.lo,
      row.synergy.hi,
      row.synergy.se,
      row.beyond?.mean ?? NaN,
      row.beyond?.lo ?? NaN,
      row.beyond?.hi ?? NaN,
    ]),
  });

  const el = h(
    "section",
    { class: "view" },
    h("div", { class: "toolbar" }, title, sizes, count, csvButton("combos.csv", toTable)),
    h("p", {
      class: "muted",
      text: "Synergy is the set minus the sum of its relics alone, game by game. A positive synergy is more than the sum, and a negative synergy is less than the sum. For a set of three or more relics, \"Beyond smaller sets\" is the part that no smaller set of its relics explains: the pairs in a set of three, and the pairs and the sets of three in a set of four. Each column is a paired difference from the loadout with no relics, thus the baseline has no effect here.",
    }),
    h("div", { class: "split" }, table.el, matrixBox),
  );

  const sizeChips = new Map<number | null, HTMLButtonElement>();
  function drawSizes(next: Context): void {
    if (sizeChips.size === 0) {
      const all = [...new Set(next.combos.map((combo) => combo.size))].sort((a, b) => a - b);
      sizes.hidden = all.length < 2;
      for (const size of [null, ...all]) {
        sizeChips.set(size, button(size === null ? "All" : String(size), () => ctx?.set({ comboSize: size }), { class: "chip" }));
      }
      sizes.replaceChildren(h("span", { class: "group-name", text: "Relics in the set" }), ...sizeChips.values());
    }
    for (const [size, chip] of sizeChips) chip.setAttribute("aria-pressed", String(next.state.comboSize === size));
  }

  function drawMatrix(next: Context): void {
    const { ds, metric } = next;
    const pairs = rows.filter((row) => row.combo.size === 2);
    matrixBox.hidden = pairs.length === 0;
    const singles = [...new Set(pairs.flatMap((row) => row.combo.parts))].sort((a, b) => a - b);
    const limit = Math.max(1e-9, ...pairs.map((row) => Math.abs(row.synergy.mean)).filter(Number.isFinite));
    const name = (loadout: number): string => ds.labels.relics[loadout] ?? "";
    const byParts = new Map(pairs.map((row) => [[...row.combo.parts].sort((a, b) => a - b).join(","), row]));
    const find = (a: number, b: number): Row | undefined => byParts.get([a, b].sort((x, y) => x - y).join(","));
    matrix.replaceChildren(
      h("thead", {}, h("tr", {}, h("th", { class: "corner" }), ...singles.map((single) => h("th", { text: name(single), attrs: { scope: "col" } })))),
      h(
        "tbody",
        {},
        ...singles.map((rowRelic) =>
          h(
            "tr",
            {},
            h("th", { text: name(rowRelic), attrs: { scope: "row" } }),
            ...singles.map((columnRelic) => {
              const row = rowRelic === columnRelic ? undefined : find(rowRelic, columnRelic);
              if (!row) return h("td", { class: "blank" });
              const t = row.synergy.mean / limit;
              const td = h("td", {
                text: formatDiff(metric, row.synergy.mean),
                class: `${Math.abs(t) > 0.55 ? "strong" : ""} ${excludesZero(row.synergy) ? "sure" : ""}`,
                attrs: { tabindex: "0", style: `background:${divergingFill(poleOf(metric, row.synergy.mean), t)}` },
              });
              attachTip(td, () => ({
                title: row.label,
                rows: [
                  [formatDiff(metric, row.synergy.mean), row.synergy.mean >= 0 ? "Synergy, more than the sum" : "Synergy, less than the sum"],
                  [`${formatDiff(metric, row.synergy.lo)} to ${formatDiff(metric, row.synergy.hi)}`, "95% interval"],
                  [formatDiff(metric, row.set.mean), "The pair against no relics"],
                  [formatDiff(metric, row.parts.mean), "Sum of the parts"],
                  [row.set.n.toLocaleString("en-US"), "Games"],
                ],
              }));
              return td;
            }),
          ),
        ),
      ),
    );
  }

  return {
    el,
    update(next) {
      ctx = next;
      const { ds, filter, metric, combos } = next;
      const key = JSON.stringify([next.state.metric, filter]);
      if (key !== computedFor) {
        computedFor = key;
        rows = combos.map((combo) => ({
          combo,
          label: ds.labels.relics[combo.loadout] ?? "",
          set: pairedDifference(ds, { axis: "relics", value: combo.loadout, against: combo.none, filter, metric }),
          parts: sumOfParts(ds, { combo, filter, metric }),
          synergy: synergy(ds, { combo, filter, metric }),
          beyond: interaction(ds, { combo, filter, metric }),
        }));
        scale = scaleOf(rows.flatMap((row) => [row.synergy.lo, row.synergy.hi, row.synergy.mean]));
        table.setHead("mark", axisHead(scale, MARK_WIDTH, (x) => formatDiff(metric, x)));
        drawMatrix(next);
      }
      drawSizes(next);
      const size = next.state.comboSize;
      const visible = size === null ? rows : rows.filter((row) => row.combo.size === size);
      title.textContent = `${metric.label}: sets of relics and their parts`;
      table.update(visible, next.state.comboSort, (row) => (row.combo.loadout === next.state.loadout ? "chosen" : ""));
      count.textContent = table.shown() < visible.length ? `The first ${table.shown()} of ${visible.length} sets` : `${visible.length} sets`;
    },
  };
}
