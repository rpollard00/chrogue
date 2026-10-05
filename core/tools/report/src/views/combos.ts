import { excludesZero, formatDiff, pairedDifference, synergy, type Estimate, type Pair, type Table } from "../analysis";
import type { Context, View } from "../app";
import { attachTip, button, h } from "../dom";
import { axisHead, divergingFill, poleOf, scaleOf, whisker, type Scale } from "../marks";
import { csvButton, sortableTable, type Column } from "../table";

interface Row {
  pair: Pair;
  label: string;
  both: Estimate;
  a: Estimate;
  b: Estimate;
  synergy: Estimate;
}

const MARK_WIDTH = 220;

export function combosView(): View {
  let ctx: Context | undefined;
  let rows: Row[] = [];
  let computedFor = "";
  let scale: Scale = scaleOf([]);

  const diff = (pick: (row: Row) => Estimate) => (row: Row) => (ctx ? formatDiff(ctx.metric, pick(row).mean) : "");
  const columns: Column<Row>[] = [
    {
      key: "name",
      label: "Pair",
      sort: (row) => row.label,
      cell: (row) => button(row.label, () => ctx?.set({ loadout: row.pair.pair, view: "detail" }), { class: "link" }),
    },
    { key: "both", label: "Pair", title: "The pair against no relics", numeric: true, sort: (row) => row.both.mean, cell: diff((row) => row.both) },
    { key: "a", label: "First relic", title: "The first relic alone against no relics", numeric: true, sort: (row) => row.a.mean, cell: diff((row) => row.a) },
    { key: "b", label: "Second relic", title: "The second relic alone against no relics", numeric: true, sort: (row) => row.b.mean, cell: diff((row) => row.b) },
    {
      key: "sum",
      label: "Sum of the parts",
      numeric: true,
      sort: (row) => row.a.mean + row.b.mean,
      cell: (row) => (ctx ? formatDiff(ctx.metric, row.a.mean + row.b.mean) : ""),
    },
    {
      key: "synergy",
      label: "Synergy",
      title: "The pair minus the sum of its parts",
      numeric: true,
      sort: (row) => row.synergy.mean,
      cell: (row) =>
        ctx
          ? h(
              "span",
              { class: excludesZero(row.synergy) ? "sure" : "unsure" },
              formatDiff(ctx.metric, row.synergy.mean),
              h("span", { class: "interval", text: ` ${formatDiff(ctx.metric, row.synergy.lo)} to ${formatDiff(ctx.metric, row.synergy.hi)}` }),
            )
          : "",
    },
    { key: "mark", label: "Synergy and its 95% interval", cell: (row) => (ctx ? whisker(row.synergy, scale, ctx.metric, MARK_WIDTH) : "") },
    { key: "games", label: "Games", numeric: true, sort: (row) => row.both.n, cell: (row) => row.both.n.toLocaleString("en-US") },
  ];
  const table = sortableTable(columns, (comboSort) => ctx?.set({ comboSort }));
  const matrix = h("table", { class: "heat div" });
  const title = h("h2");

  const toTable = (): Table => ({
    columns: ["pair", "games", "pair_diff", "first_diff", "second_diff", "synergy", "synergy_lo", "synergy_hi", "synergy_se"],
    rows: rows.map((row) => [row.label, row.both.n, row.both.mean, row.a.mean, row.b.mean, row.synergy.mean, row.synergy.lo, row.synergy.hi, row.synergy.se]),
  });

  const el = h(
    "section",
    { class: "view" },
    h("div", { class: "toolbar" }, title, csvButton("combos.csv", toTable)),
    h(
      "p",
      { class: "muted", text: "Synergy is the pair minus the sum of its parts, game by game. A positive synergy is more than the sum, and a negative synergy is less than the sum. Each column is a paired difference from the loadout with no relics, thus the baseline has no effect here." },
    ),
    h("div", { class: "split" }, table.el, h("div", {}, h("h3", { text: "Synergy of each pair" }), matrix)),
  );

  function drawMatrix(next: Context): void {
    const { ds, metric } = next;
    const singles = [...new Set(rows.flatMap((row) => [row.pair.a, row.pair.b]))].sort((a, b) => a - b);
    const limit = Math.max(1e-9, ...rows.map((row) => Math.abs(row.synergy.mean)).filter(Number.isFinite));
    const name = (loadout: number): string => ds.labels.relics[loadout] ?? "";
    const find = (a: number, b: number): Row | undefined =>
      rows.find((row) => (row.pair.a === a && row.pair.b === b) || (row.pair.a === b && row.pair.b === a));
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
                  [formatDiff(metric, row.both.mean), "The pair against no relics"],
                  [formatDiff(metric, row.a.mean + row.b.mean), "Sum of the parts"],
                  [row.both.n.toLocaleString("en-US"), "Games"],
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
      const { ds, filter, metric, pairs } = next;
      const key = JSON.stringify([next.state.metric, filter]);
      if (key !== computedFor) {
        computedFor = key;
        const against = (pair: Pair, value: number): Estimate => pairedDifference(ds, { axis: "relics", value, against: pair.none, filter, metric });
        rows = pairs.map((pair) => ({
          pair,
          label: ds.labels.relics[pair.pair] ?? "",
          both: against(pair, pair.pair),
          a: against(pair, pair.a),
          b: against(pair, pair.b),
          synergy: synergy(ds, { pair, filter, metric }),
        }));
        scale = scaleOf(rows.flatMap((row) => [row.synergy.lo, row.synergy.hi, row.synergy.mean]));
        table.setHead("mark", axisHead(scale, MARK_WIDTH, (x) => formatDiff(metric, x)));
        drawMatrix(next);
      }
      title.textContent = `${metric.label}: pairs and their parts`;
      table.update(rows, next.state.comboSort, (row) => (row.pair.pair === next.state.loadout ? "chosen" : ""));
    },
  };
}
