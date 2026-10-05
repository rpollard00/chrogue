import { excludesZero, type Estimate, type Metric } from "./analysis";
import { h, svg } from "./dom";

export type Pole = "cool" | "warm" | "none";

export function poleOf(metric: Metric, x: number): Pole {
  if (!(x > 0 || x < 0)) return "none";
  return x > 0 === (metric.favors !== "enemy") ? "cool" : "warm";
}

export function poleLabels(metric: Metric): { negative: string; positive: string } {
  if (metric.favors === "neither") return { negative: "lower", positive: "higher" };
  const player = "helps the player";
  const enemy = "helps the enemy";
  return metric.favors === "player" ? { negative: enemy, positive: player } : { negative: player, positive: enemy };
}

export interface Scale {
  min: number;
  max: number;
  at: (x: number) => number;
}

export function scaleOf(values: readonly number[]): Scale {
  let min = 0;
  let max = 0;
  for (const x of values) {
    if (!Number.isFinite(x)) continue;
    min = Math.min(min, x);
    max = Math.max(max, x);
  }
  if (min === max) max = min + 1;
  const pad = (max - min) * 0.04;
  min -= pad;
  max += pad;
  return { min, max, at: (x) => (x - min) / (max - min) };
}

export function whisker(estimate: Estimate, scale: Scale, metric: Metric, width: number): SVGSVGElement {
  const height = 20;
  const x = (value: number): number => Math.round(scale.at(value) * width * 10) / 10;
  const root = svg("svg", { width, height, viewBox: `0 0 ${width} ${height}`, class: "whisker", "aria-hidden": "true" });
  root.append(svg("line", { x1: x(0), x2: x(0), y1: 0, y2: height, class: "zero" }));
  if (!Number.isFinite(estimate.mean)) return root;
  const solid = excludesZero(estimate);
  const pole = solid ? poleOf(metric, estimate.mean) : "none";
  if (Number.isFinite(estimate.lo)) {
    root.append(svg("line", { x1: x(estimate.lo), x2: x(estimate.hi), y1: height / 2, y2: height / 2, class: `range ${solid ? "solid" : "open"}` }));
  }
  root.append(svg("circle", { cx: x(estimate.mean), cy: height / 2, r: 4.5, class: `dot ${solid ? "solid" : "open"} pole-${pole}` }));
  return root;
}

export function divergingFill(pole: Pole, strength: number): string {
  const share = Math.round(Math.min(1, Math.abs(strength)) * 100);
  return pole === "none" ? "var(--div-mid)" : `color-mix(in oklab, var(--pole-${pole}) ${share}%, var(--div-mid))`;
}

export function sequentialFill(t: number): string {
  return `color-mix(in oklab, var(--seq-hi) ${Math.round(Math.min(1, Math.max(0, t)) * 100)}%, var(--seq-lo))`;
}

export function axisHead(scale: Scale, width: number, format: (x: number) => string): HTMLElement {
  const zero = scale.at(0);
  return h(
    "div",
    { class: "axis-head", attrs: { style: `width:${width}px` } },
    h("span", { text: zero > 0.15 ? format(scale.min) : "" }),
    h("span", { class: "axis-zero", text: "0", attrs: { style: `left:${zero * 100}%` } }),
    h("span", { text: zero < 0.85 ? format(scale.max) : "" }),
  );
}

export function niceCeil(x: number): number {
  if (!(x > 0)) return 1;
  const power = 10 ** Math.floor(Math.log10(x));
  const step = [1, 2, 2.5, 5, 10].find((m) => m * power >= x) ?? 10;
  return step * power;
}
