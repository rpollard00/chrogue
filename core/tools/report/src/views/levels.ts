import { axisIndexes, excludesZero, formatDiff, formatValue, pairedDifference, selectCells, summarize, type Estimate, type Summary, type Table } from "../analysis";
import type { Context, View } from "../app";
import { attachTip, button, h } from "../dom";
import { divergingFill, poleLabels, poleOf, sequentialFill } from "../marks";
import { csvButton } from "../table";

interface HeatCell {
  player: number;
  across: number;
  summary: Summary;
  diff: Estimate;
}

export function levelsView(): View {
  let ctx: Context | undefined;
  let cells: HeatCell[] = [];
  const modes = {
    diff: button("Difference from the baseline", () => ctx?.set({ heat: "diff" }), { class: "segment" }),
    value: button("Value", () => ctx?.set({ heat: "value" }), { class: "segment" }),
  };
  const title = h("h2");
  const note = h("p", { class: "muted" });
  const grid = h("table", { class: "heat" });
  const scaleBar = h("div", { class: "scale" });
  const acrossOf = (next: Context): "enemy" | "floor" => (next.ds.size.enemy === 1 && next.ds.axes.enemy[0] === "floor" ? "floor" : "enemy");

  const toTable = (): Table => {
    if (!ctx) return { columns: [], rows: [] };
    const { ds, metric } = ctx;
    const across = acrossOf(ctx);
    return {
      columns: ["player", across, "games", metric.key, "lo", "hi", "diff", "diff_lo", "diff_hi"],
      rows: cells.map((cell) => [
        ds.labels.player[cell.player] ?? "",
        ds.labels[across][cell.across] ?? "",
        cell.summary.n,
        cell.summary.mean,
        cell.summary.lo,
        cell.summary.hi,
        cell.diff.mean,
        cell.diff.lo,
        cell.diff.hi,
      ]),
    };
  };

  const el = h(
    "section",
    { class: "view" },
    h("div", { class: "toolbar" }, title, h("div", { class: "segments", attrs: { role: "group", "aria-label": "Cell content" } }, modes.diff, modes.value), csvButton("levels.csv", toTable)),
    note,
    grid,
    scaleBar,
  );

  return {
    el,
    update(next) {
      ctx = next;
      const { ds, filter, metric, state } = next;
      const across = acrossOf(next);
      const players = axisIndexes(ds, filter, "player");
      const columns = axisIndexes(ds, filter, across);
      const diffMode = state.heat === "diff";
      modes.diff.setAttribute("aria-pressed", String(diffMode));
      modes.value.setAttribute("aria-pressed", String(!diffMode));
      title.textContent = `${metric.label}, ${ds.labels.relics[state.loadout]}: player level and ${across === "floor" ? "floor" : "enemy level"}`;
      note.textContent =
        diffMode && state.loadout === state.baseline
          ? "The loadout is the baseline, thus each difference is zero. Select a different loadout or show the value."
          : diffMode
            ? `Each cell is the paired difference from the baseline (${ds.labels.relics[state.baseline]}). Bold text shows a 95% interval that excludes zero.`
            : "Each cell is the mean of the games in the cell.";

      cells = players.flatMap((player) =>
        columns.map((column) => {
          const cellFilter = { ...filter, player: [player], [across]: [column] };
          return {
            player,
            across: column,
            summary: summarize(selectCells(ds, { ...cellFilter, relics: [state.loadout] }), metric),
            diff: pairedDifference(ds, { axis: "relics", value: state.loadout, against: state.baseline, filter: cellFilter, metric }),
          };
        }),
      );

      const means = cells.map((cell) => cell.summary.mean).filter(Number.isFinite);
      const unit = metric.unit === "share" || metric.unit === "points";
      const low = unit ? 0 : Math.min(...means);
      const high = unit ? 1 : Math.max(...means);
      const limit = Math.max(1e-9, ...cells.map((cell) => Math.abs(cell.diff.mean)).filter(Number.isFinite));

      const head = h(
        "tr",
        {},
        h("th", { class: "corner", text: across === "floor" ? "Player ╲ Floor" : "Player ╲ Enemy" }),
        ...columns.map((column) => h("th", { text: ds.labels[across][column] ?? "", attrs: { scope: "col" } })),
      );
      const body = players.map((player) =>
        h(
          "tr",
          {},
          h("th", { text: ds.labels.player[player] ?? "", attrs: { scope: "row" } }),
          ...cells
            .filter((cell) => cell.player === player)
            .map((cell) => {
              const t = diffMode ? cell.diff.mean / limit : (cell.summary.mean - low) / (high - low || 1);
              const td = h("td", {
                text: diffMode ? formatDiff(metric, cell.diff.mean) : formatValue(metric, cell.summary.mean),
                class: `${Math.abs(t) > 0.55 ? "strong" : ""} ${diffMode && excludesZero(cell.diff) ? "sure" : ""}`,
                attrs: { tabindex: "0", style: `background:${diffMode ? divergingFill(poleOf(metric, cell.diff.mean), t) : sequentialFill(t)}` },
              });
              attachTip(td, () => ({
                title: `${ds.labels.player[cell.player]} against ${ds.labels[across][cell.across]}`,
                rows: [
                  [formatValue(metric, cell.summary.mean), metric.label],
                  [`${formatValue(metric, cell.summary.lo)} to ${formatValue(metric, cell.summary.hi)}`, "95% interval"],
                  [formatDiff(metric, cell.diff.mean), "Difference from the baseline"],
                  [`${formatDiff(metric, cell.diff.lo)} to ${formatDiff(metric, cell.diff.hi)}`, "95% interval of the difference"],
                  [cell.summary.n.toLocaleString("en-US"), "Games"],
                ],
              }));
              return td;
            }),
        ),
      );
      grid.className = `heat ${diffMode ? "div" : "seq"}`;
      grid.replaceChildren(h("thead", {}, head), h("tbody", {}, ...body));

      const labels = poleLabels(metric);
      scaleBar.replaceChildren(
        ...(diffMode
          ? [
              h("span", { text: `${formatDiff(metric, -limit)}, ${labels.negative}` }),
              h("i", { class: `ramp diverging ${metric.favors === "enemy" ? "flip" : ""}` }),
              h("span", { text: `${formatDiff(metric, limit)}, ${labels.positive}` }),
            ]
          : [h("span", { text: formatValue(metric, low) }), h("i", { class: "ramp sequential" }), h("span", { text: formatValue(metric, high) })]),
      );
    },
  };
}
