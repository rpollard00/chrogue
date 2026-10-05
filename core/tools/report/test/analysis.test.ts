import { describe, expect, test } from "bun:test";
import {
  METRICS,
  contrast,
  excludesZero,
  findCombos,
  interaction,
  formatDiff,
  formatValue,
  groupCells,
  loadoutEffects,
  pairedDifference,
  parseBalance,
  parseBalanceText,
  selectCells,
  summarize,
  sumOfParts,
  synergy,
  toCsv,
  type Dataset,
} from "../src/analysis";
import { PLANTED, generateFixture, type BalanceFile } from "../src/fixture";

function load(file: BalanceFile): Dataset {
  const result = parseBalance(file);
  if (result.kind === "error") throw new Error(result.message);
  return result.dataset;
}

const ds = load(generateFixture());
const loadout = (...keys: string[]): number =>
  ds.axes.relics.findIndex((entry) => entry.length === keys.length && keys.every((key) => entry.includes(key)));
const none = loadout();

function tiny(): BalanceFile {
  const file = generateFixture({ games: 4, floors: [1], players: [2], enemies: ["floor"], armies: ["auto"], traits: ["none"], singleCount: 1, pairedCount: 0 });
  const [base, single] = file.cells;
  if (!base || !single) throw new Error("the tiny fixture must have two cells");
  Object.assign(base, { results: "wdll", ends: "mcmr", plies: [10, 20, 30, 40], gold: [5, 1, 0, 0], lost: [0, 2, 4, 6] });
  Object.assign(single, { results: "wwdl", ends: "mmcr", plies: [12, 18, 30, 44], gold: [6, 5, 1, 0], lost: [1, 0, 2, 6] });
  return file;
}

function messageOf(input: unknown): string {
  const result = parseBalance(input);
  return result.kind === "error" ? result.message : "";
}

describe("parseBalance", () => {
  test("reads each cell of the grid", () => {
    expect(ds.cells.length).toBe(8 * 4 * 2 * 2 * 2 * 27);
    expect(ds.labels.relics[none]).toBe("No relics");
    expect(ds.labels.enemy).toEqual(["Level of the floor", "L5 Sergeant"]);
  });

  test("puts the cells in the order of the axes when the file has a different order", () => {
    const file = tiny();
    file.cells.reverse();
    const parsed = load(file);
    expect(parsed.cells.map((cell) => cell.at.relics)).toEqual([0, 1]);
  });

  test("rejects a file of a different format or version", () => {
    expect(messageOf({ ...tiny(), format: "other" })).toContain("format");
    expect(messageOf({ ...tiny(), version: 2 })).toContain("version 1");
    expect(messageOf([])).toContain("must be an object");
  });

  test("rejects a cell with a wrong length, a wrong index, or an unknown character", () => {
    const short = tiny();
    short.cells[0]?.plies.pop();
    expect(messageOf(short)).toBe('cells[0].plies: has 3 values, but "games" is 4');

    const far = tiny();
    Object.assign(far.cells[1] ?? {}, { relics: 2 });
    expect(messageOf(far)).toContain("cells[1].relics: index 2 is not in the axis");

    const odd = tiny();
    Object.assign(odd.cells[0] ?? {}, { results: "wq" + "ll" });
    expect(messageOf(odd)).toContain('unknown character "q"');
  });

  test("rejects a grid with a missing or a repeated cell", () => {
    const missing = tiny();
    missing.cells.pop();
    expect(messageOf(missing)).toBe("cells: has 1 cells, but the axes give 2");

    const repeated = tiny();
    Object.assign(repeated.cells[1] ?? {}, { relics: 0 });
    expect(messageOf(repeated)).toContain("a second cell has the same axis indexes");
  });

  test("rejects a loadout with a relic that the content does not have", () => {
    const file = tiny();
    file.axes.relics[1] = ["nope"];
    expect(messageOf(file)).toContain('relic "nope" is not in content.relics');
  });

  test("reports text that is not JSON", () => {
    const result = parseBalanceText("{");
    expect(result.kind === "error" && result.message).toContain("not JSON");
  });
});

describe("metrics and summaries", () => {
  const small = load(tiny());
  const base = selectCells(small, { relics: [0] });

  test("each metric reads its value for each game", () => {
    expect(summarize(base, METRICS.win).mean).toBe(0.25);
    expect(summarize(base, METRICS.draw).mean).toBe(0.25);
    expect(summarize(base, METRICS.loss).mean).toBe(0.5);
    expect(summarize(base, METRICS.score).mean).toBe(0.375);
    expect(summarize(base, METRICS.gold).mean).toBe(1.5);
    expect(summarize(base, METRICS.lost).mean).toBe(3);
    expect(summarize(base, METRICS.plies).mean).toBe(25);
  });

  test("a rate gets the Wilson interval", () => {
    const s = summarize(base, METRICS.win);
    expect(s.n).toBe(4);
    expect(s.lo).toBeCloseTo(0.0456, 3);
    expect(s.hi).toBeCloseTo(0.6994, 3);
  });

  test("another metric gets the normal interval", () => {
    const s = summarize(base, METRICS.plies);
    expect(s.sd).toBeCloseTo(12.91, 2);
    expect(s.hi - s.mean).toBeCloseTo((1.959964 * s.sd) / 2, 6);
    expect(s.mean - s.lo).toBeCloseTo(s.hi - s.mean, 9);
  });

  test("an interval becomes narrower with more games", () => {
    const one = summarize(selectCells(ds, { relics: [none], floor: [0], player: [0] }), METRICS.win);
    const all = summarize(selectCells(ds, { relics: [none] }), METRICS.win);
    expect(all.n).toBeGreaterThan(one.n);
    expect(all.hi - all.lo).toBeLessThan(one.hi - one.lo);
  });

  test("an empty selection has no mean", () => {
    expect(summarize([], METRICS.win)).toEqual({ n: 0, mean: NaN, sd: NaN, lo: NaN, hi: NaN });
  });
});

describe("selection", () => {
  test("a filter keeps the product of its values", () => {
    const cells = selectCells(ds, { floor: [0, 3], player: [1], relics: [none] });
    expect(cells.length).toBe(2 * 1 * 2 * 2 * 2);
    expect(new Set(cells.map((cell) => cell.at.floor))).toEqual(new Set([0, 3]));
  });

  test("a group-by gives one group for each combination", () => {
    const groups = groupCells(ds, { relics: [none] }, ["floor", "player"]);
    expect(groups.length).toBe(8 * 4);
    expect(groups[0]?.at).toEqual({ floor: 0, player: 0 });
    expect(groups.every((group) => group.cells.length === 2 * 2 * 2)).toBe(true);
  });
});

describe("paired difference", () => {
  test("is the mean of the game-by-game differences", () => {
    const small = load(tiny());
    const diff = pairedDifference(small, { axis: "relics", value: 1, against: 0, metric: METRICS.score });
    expect(diff.n).toBe(4);
    expect(diff.seeds).toBe(4);
    expect(diff.mean).toBeCloseTo(0.25, 9);
    expect(diff.se).toBeCloseTo(Math.sqrt(1 / 12 / 4), 9);
  });

  test("a loadout against itself has no difference", () => {
    const diff = pairedDifference(ds, { axis: "relics", value: none, against: none, metric: METRICS.win });
    expect(diff.mean).toBe(0);
    expect(diff.se).toBe(0);
  });

  test("finds the strong relic and the weak relic of the fixture", () => {
    const strong = pairedDifference(ds, { axis: "relics", value: loadout(PLANTED.strong), against: none, metric: METRICS.win });
    const weak = pairedDifference(ds, { axis: "relics", value: loadout(PLANTED.weak), against: none, metric: METRICS.win });
    expect(strong.lo).toBeGreaterThan(0);
    expect(weak.hi).toBeLessThan(0);
    expect(excludesZero(strong) && excludesZero(weak)).toBe(true);

    const ranked = loadoutEffects(ds, { baseline: none, metric: METRICS.win })
      .filter((effect) => ds.axes.relics[effect.loadout]?.length === 1)
      .sort((a, b) => b.diff.mean - a.diff.mean);
    expect(ranked[0]?.label).toBe("Queen's Flight");
    expect(ranked.at(-1)?.label).toBe("Tactical Retreat");
  });

  test("a relic with no planted effect on the result has an interval that includes zero", () => {
    const diff = pairedDifference(ds, { axis: "relics", value: loadout("bounty"), against: none, metric: METRICS.score });
    expect(excludesZero(diff)).toBe(false);
    const gold = pairedDifference(ds, { axis: "relics", value: loadout("bounty"), against: none, metric: METRICS.gold });
    expect(gold.lo).toBeGreaterThan(0);
  });

  test("the effect of the strong relic fades at a high player level", () => {
    const at = (player: number) =>
      pairedDifference(ds, {
        axis: "relics",
        value: loadout(PLANTED.strong),
        against: none,
        filter: { player: [player], enemy: [1] },
        metric: METRICS.score,
      }).mean;
    expect(at(0)).toBeGreaterThan(at(3) + 0.05);
  });

  test("the paired interval is narrower than the unpaired interval", () => {
    const filter = { player: [1], enemy: [0], army: [0], traits: [1] };
    const a = loadout("sidestep");
    const paired = pairedDifference(ds, { axis: "relics", value: a, against: none, filter, metric: METRICS.score });
    const left = summarize(selectCells(ds, { ...filter, relics: [a] }), METRICS.score);
    const right = summarize(selectCells(ds, { ...filter, relics: [none] }), METRICS.score);
    const unpaired = Math.sqrt(left.sd ** 2 / left.n + right.sd ** 2 / right.n);
    expect(paired.mean).toBeCloseTo(left.mean - right.mean, 9);
    expect(paired.se).toBeLessThan(unpaired * 0.8);
  });

  test("a second cell with the same run seeds and the same differences does not make the interval narrower", () => {
    const file = generateFixture({ games: 4, floors: [1], players: [2, 3], enemies: ["floor"], armies: ["auto"], traits: ["none"], singleCount: 1, pairedCount: 0 });
    const source = tiny().cells;
    file.cells.forEach((cell, i) => {
      const { results, ends, plies, gold, lost } = source[i % 2] ?? cell;
      Object.assign(cell, { results, ends, plies, gold, lost });
    });
    const twice = pairedDifference(load(file), { axis: "relics", value: 1, against: 0, metric: METRICS.score });
    const once = pairedDifference(load(tiny()), { axis: "relics", value: 1, against: 0, metric: METRICS.score });
    expect(twice.n).toBe(8);
    expect(twice.seeds).toBe(4);
    expect(twice.se).toBeCloseTo(once.se, 9);
  });

  test("works along the player axis", () => {
    const diff = pairedDifference(ds, { axis: "player", value: 3, against: 0, filter: { relics: [none] }, metric: METRICS.win });
    expect(diff.lo).toBeGreaterThan(0);
  });
});

describe("synergy", () => {
  const combos = findCombos(ds);
  const comboOf = (keys: readonly string[]) => {
    const combo = combos.find((entry) => entry.loadout === loadout(...keys));
    if (!combo) throw new Error(`no set ${keys.join("+")}`);
    return combo;
  };
  const mean = (index: number) => summarize(selectCells(ds, { relics: [index] }), METRICS.score).mean;

  test("finds each set that has its parts in the data", () => {
    expect(combos.filter((combo) => combo.size === 2).length).toBe(10);
    expect(combos.filter((combo) => combo.size === 3).length).toBe(4);
    expect(combos.every((combo) => combo.none === none && combo.parts.length === combo.size)).toBe(true);
    expect(combos.every((combo) => combo.subsets?.length === 2 ** combo.size - 1)).toBe(true);
    const noParts = load(generateFixture({ games: 2, floors: [1], players: [2], singleCount: 2, pairedCount: 2 }));
    noParts.axes.relics[1] = ["sidestep"];
    expect(findCombos(noParts)).toEqual([]);
  });

  test("a set of three with no pair of its relics in the data has a synergy and no interaction", () => {
    const file = generateFixture({ games: 2, floors: [1], players: [2], singleCount: 3, pairedCount: 0, tripleCount: 3 });
    const [triple] = findCombos(load(file));
    expect(triple?.size).toBe(3);
    expect(triple?.subsets).toBeNull();
    if (triple) expect(interaction(load(file), { combo: triple, metric: METRICS.score })).toBeNull();
  });

  test("of a pair is x(a+b) − x(a) − x(b) + x(none), and the interaction is the same", () => {
    const pair = comboOf(PLANTED.synergy);
    const value = synergy(ds, { combo: pair, metric: METRICS.score });
    const [a = 0, b = 0] = pair.parts;
    expect(value.mean).toBeCloseTo(mean(pair.loadout) - mean(a) - mean(b) + mean(none), 9);
    expect(interaction(ds, { combo: pair, metric: METRICS.score })?.mean).toBeCloseTo(value.mean, 9);
  });

  test("of a set of three is the set minus its three relics alone plus two times no relics", () => {
    const triple = comboOf(PLANTED.triple);
    const parts = triple.parts.reduce((sum, part) => sum + mean(part), 0);
    expect(synergy(ds, { combo: triple, metric: METRICS.score }).mean).toBeCloseTo(mean(triple.loadout) - parts + 2 * mean(none), 9);
    expect(sumOfParts(ds, { combo: triple, metric: METRICS.score }).mean).toBeCloseTo(parts - 3 * mean(none), 9);
  });

  test("the interaction of a set of three takes away its three pairs", () => {
    const triple = comboOf(PLANTED.triple);
    const [a = "", b = "", c = ""] = PLANTED.triple;
    const pairs = mean(loadout(a, b)) + mean(loadout(a, c)) + mean(loadout(b, c));
    const singles = triple.parts.reduce((sum, part) => sum + mean(part), 0);
    const value = interaction(ds, { combo: triple, metric: METRICS.score });
    expect(value?.mean).toBeCloseTo(mean(triple.loadout) - pairs + singles - mean(none), 9);
  });

  test("finds the planted sets above their parts and the planted pair below its parts", () => {
    const up = synergy(ds, { combo: comboOf(PLANTED.synergy), metric: METRICS.score });
    const down = synergy(ds, { combo: comboOf(PLANTED.antiSynergy), metric: METRICS.score });
    expect(up.lo).toBeGreaterThan(0);
    expect(down.hi).toBeLessThan(0);
    const beyond = (combo: (typeof combos)[number]) => interaction(ds, { combo, metric: METRICS.score })?.mean ?? NaN;
    const triples = combos.filter((combo) => combo.size === 3).sort((x, y) => beyond(y) - beyond(x));
    expect(triples[0]).toEqual(comboOf(PLANTED.triple));
    expect(interaction(ds, { combo: comboOf(PLANTED.triple), metric: METRICS.score })?.lo).toBeGreaterThan(0);
  });

  test("a contrast with one term is the mean of that value", () => {
    const value = contrast(ds, { axis: "relics", terms: [{ index: none, weight: 1 }], metric: METRICS.win });
    expect(value.mean).toBeCloseTo(summarize(selectCells(ds, { relics: [none] }), METRICS.win).mean, 9);
  });
});

describe("output", () => {
  test("formats a share as percent and its difference as percentage points", () => {
    expect(formatValue(METRICS.win, 0.4321)).toBe("43.2%");
    expect(formatDiff(METRICS.win, 0.051)).toBe("+5.1 pp");
    expect(formatDiff(METRICS.plies, -3.14159)).toBe("−3.14");
    expect(formatValue(METRICS.gold, NaN)).toBe("–");
  });

  test("writes CSV with quotes where a field needs them", () => {
    const csv = toCsv({ columns: ["Loadout", "Win rate"], rows: [["Bounty, twice", 0.5], ['The "Warden"', NaN]] });
    expect(csv).toBe('Loadout,Win rate\n"Bounty, twice",0.5\n"The ""Warden""",\n');
  });
});
