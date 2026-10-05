import { axisIndexes, formatValue, selectCells, summarize, type Dataset, type Summary, type Table } from "../analysis";
import type { Context, View } from "../app";
import { attachTip, h, svg } from "../dom";
import { niceCeil } from "../marks";
import { csvButton, sortableTable, type Column } from "../table";

interface Series {
  slot: "base" | "s1" | "s2" | "s3";
  label: string;
  points: Summary[];
}

interface FloorRow {
  floor: number;
  values: Summary[];
}

const WIDTH = 1160;
const HEIGHT = 360;
const LEFT = 56;
const RIGHT = 24;
const TOP = 16;
const BOTTOM = 58;

export function floorsView(ds: Dataset): View {
  let ctx: Context | undefined;
  let series: Series[] = [];
  let floors: readonly number[] = [];

  const picker = (label: string, slot: 0 | 1): { el: HTMLElement; input: HTMLSelectElement } => {
    const input = h("select", { attrs: { name: `compare-${slot}` } }, h("option", { text: "None", attrs: { value: "" } }), ...ds.labels.relics.map((text, i) => h("option", { text, attrs: { value: String(i) } })));
    input.addEventListener("change", () => {
      if (!ctx) return;
      const compare: [number | null, number | null] = [...ctx.state.compare];
      compare[slot] = input.value === "" ? null : Number(input.value);
      ctx.set({ compare });
    });
    return { el: h("label", { class: "field" }, h("span", { text: label }), input), input };
  };
  const second = picker("Second loadout", 0);
  const third = picker("Third loadout", 1);
  const title = h("h2");
  const legend = h("div", { class: "legend" });
  const chart = h("div", { class: "chart" });
  const tableHost = h("div");

  const toTable = (): Table => ({
    columns: ["floor", "name", ...series.flatMap((entry) => [entry.label, `${entry.label} lo`, `${entry.label} hi`, `${entry.label} games`])],
    rows: floors.map((floor, i) => [
      ds.axes.floor[floor] ?? "",
      ds.floors.find((entry) => entry.number === ds.axes.floor[floor])?.name ?? "",
      ...series.flatMap((entry): (string | number)[] => {
        const point = entry.points[i];
        return point ? [point.mean, point.lo, point.hi, point.n] : ["", "", "", ""];
      }),
    ]),
  });

  const el = h(
    "section",
    { class: "view" },
    h("div", { class: "toolbar" }, title, second.el, third.el, csvButton("floors.csv", toTable)),
    legend,
    chart,
    tableHost,
  );

  function draw(next: Context): void {
    const { metric } = next;
    const unit = metric.unit === "share" || metric.unit === "points";
    const top = unit ? 1 : niceCeil(Math.max(...series.flatMap((entry) => entry.points.map((point) => point.hi)).filter(Number.isFinite), 0));
    const plotW = WIDTH - LEFT - RIGHT;
    const plotH = HEIGHT - TOP - BOTTOM;
    const step = plotW / floors.length;
    const x = (i: number): number => LEFT + step * (i + 0.5);
    const y = (value: number): number => TOP + plotH * (1 - Math.min(1, Math.max(0, value / top)));
    const root = svg("svg", { viewBox: `0 0 ${WIDTH} ${HEIGHT}`, width: WIDTH, height: HEIGHT, role: "img" });
    root.setAttribute("aria-label", `${metric.label} for each floor. The table that follows has the same values.`);

    for (let tick = 0; tick <= 4; tick++) {
      const value = (top * tick) / 4;
      root.append(
        svg("line", { x1: LEFT, x2: WIDTH - RIGHT, y1: y(value), y2: y(value), class: tick === 0 ? "axis" : "grid" }),
        svg("text", { x: LEFT - 8, y: y(value) + 4, class: "tick", "text-anchor": "end" }, formatValue(metric, value)),
      );
    }
    floors.forEach((floor, i) => {
      const def = ds.floors.find((entry) => entry.number === ds.axes.floor[floor]);
      root.append(
        svg("text", { x: x(i), y: HEIGHT - BOTTOM + 18, class: "tick strong", "text-anchor": "middle" }, String(ds.axes.floor[floor])),
        svg("text", { x: x(i), y: HEIGHT - BOTTOM + 33, class: "tick", "text-anchor": "middle" }, def?.name ?? ""),
        svg("text", { x: x(i), y: HEIGHT - BOTTOM + 48, class: "tick", "text-anchor": "middle" }, def ? `L${def.level}, budget ${def.budget}${def.boss ? ", boss" : ""}` : ""),
      );
    });

    for (const entry of series) {
      const known = entry.points.map((point, i) => ({ point, i })).filter(({ point }) => Number.isFinite(point.mean));
      const upper = known.map(({ point, i }) => `${x(i)},${y(point.hi)}`);
      const lower = known.map(({ point, i }) => `${x(i)},${y(point.lo)}`).reverse();
      root.append(
        svg("polygon", { points: [...upper, ...lower].join(" "), class: `band ${entry.slot}` }),
        svg("polyline", { points: known.map(({ point, i }) => `${x(i)},${y(point.mean)}`).join(" "), class: `line ${entry.slot}` }),
      );
    }
    for (const entry of series) {
      entry.points.forEach((point, i) => {
        if (Number.isFinite(point.mean)) root.append(svg("circle", { cx: x(i), cy: y(point.mean), r: 4, class: `point ${entry.slot}` }));
      });
    }

    const cross = svg("line", { y1: TOP, y2: TOP + plotH, class: "cross", visibility: "hidden" });
    root.append(cross);
    floors.forEach((floor, i) => {
      const hit = svg("rect", { x: LEFT + step * i, y: TOP, width: step, height: plotH + BOTTOM, class: "hit", tabindex: 0 });
      hit.setAttribute("aria-label", `Floor ${ds.labels.floor[floor]}`);
      const show = (): void => {
        cross.setAttribute("x1", String(x(i)));
        cross.setAttribute("x2", String(x(i)));
        cross.setAttribute("visibility", "visible");
      };
      const hide = (): void => cross.setAttribute("visibility", "hidden");
      hit.addEventListener("pointerenter", show);
      hit.addEventListener("focus", show);
      hit.addEventListener("pointerleave", hide);
      hit.addEventListener("blur", hide);
      attachTip(hit, () => ({
        title: `Floor ${ds.labels.floor[floor]}`,
        rows: series.map((entry) => {
          const point = entry.points[i];
          return [point ? `${formatValue(metric, point.mean)} (${formatValue(metric, point.lo)} to ${formatValue(metric, point.hi)})` : "–", entry.label];
        }),
      }));
      root.append(hit);
    });
    chart.replaceChildren(root);
  }

  return {
    el,
    update(next) {
      ctx = next;
      const { filter, metric, state } = next;
      floors = axisIndexes(ds, filter, "floor");
      second.input.value = state.compare[0] === null ? "" : String(state.compare[0]);
      third.input.value = state.compare[1] === null ? "" : String(state.compare[1]);
      title.textContent = `${metric.label} for each floor`;

      const picked: [Series["slot"], number | null][] = [
        ["base", state.baseline],
        ["s1", state.loadout],
        ["s2", state.compare[0]],
        ["s3", state.compare[1]],
      ];
      const seen = new Set<number>();
      series = picked.flatMap(([slot, loadout]) => {
        if (loadout === null || seen.has(loadout)) return [];
        seen.add(loadout);
        const name = ds.labels.relics[loadout] ?? "";
        return [
          {
            slot,
            label: slot === "base" ? `${name} (baseline)` : name,
            points: floors.map((floor) => summarize(selectCells(ds, { ...filter, floor: [floor], relics: [loadout] }), metric)),
          },
        ];
      });

      legend.replaceChildren(
        ...series.map((entry) => h("span", { class: "key" }, h("i", { class: `swatch line-key ${entry.slot}` }), entry.label)),
        h("span", { class: "muted", text: "A band shows the 95% interval." }),
      );
      draw(next);

      const columns: Column<FloorRow>[] = [
        { key: "floor", label: "Floor", cell: (row) => ds.labels.floor[row.floor] ?? "" },
        ...series.map((entry, s): Column<FloorRow> => ({
          key: entry.slot,
          label: entry.label,
          numeric: true,
          cell: (row) => {
            const point = row.values[s];
            return point
              ? h("span", {}, formatValue(metric, point.mean), h("span", { class: "interval", text: ` ${formatValue(metric, point.lo)} to ${formatValue(metric, point.hi)}` }))
              : "";
          },
        })),
      ];
      const table = sortableTable(columns, () => {});
      table.update(
        floors.map((floor, i) => ({ floor, values: series.flatMap((entry) => entry.points[i] ?? []) })),
        { key: "", descending: false },
      );
      tableHost.replaceChildren(table.el);
    },
  };
}
