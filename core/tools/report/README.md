# Balance report

This folder makes an HTML report from a `balance.json` file. The tool `balance` of `chrogue-tools` writes that file.

The report is one HTML file with the data in it. It opens from the disk and it uses no network. The folder also has a TypeScript module that reads the same file in a Bun script.

The report shows the size of each effect and its interval. It does not tell you if an effect is good or bad for the game.

## Commands

Run the commands from this folder. Bun is necessary.

- Install the packages: `bun install`
- Make a report: `bun run report <balance.json> [out.html]`. The default output is `balance.html` in the folder of the input.
- Development server: `bun run dev`
- Tests: `bun test`
- Type check: `bun run check`
- Make a file with invented results: `bun run fixture [out.json] [games] [seed]`

`bun run report` builds the page into `dist/` and puts the data in a copy of it. If the sources did not change, the command uses the page that is in `dist/`.

A page with no data shows a file selector. You can also drop a file on the page. Thus `dist/index.html` and the development server can open each `balance.json`.

## Structure

- `src/analysis.ts`: The model of the file, the parser, the metrics, the selection, and the statistics. It does not use the page.
- `src/fixture.ts`: A generator of a `balance.json` with invented results and planted effects. The tests and the development of the page use it.
- `src/state.ts`: The state of the page and its form in the URL hash.
- `src/app.ts`: The header, the controls, and the tabs. `src/views/` has one file for each view.
- `src/marks.ts`, `src/table.ts`, `src/dom.ts`: The parts that the views share.
- `scripts/report.ts`: The command `bun run report`.

## Views

The metric, the baseline loadout, the loadout, and the filters apply to each view. The URL hash holds them, thus you can copy the URL of a view.

A click on a filter value when the filter has all values keeps only that value. Other clicks add or remove a value.

- Relics: Each loadout with its paired difference from the baseline. A solid dot shows a 95% interval that excludes zero.
- Levels: The loadout for each player level and enemy level. If each enemy has the level of its floor, the columns are the floors.
- Floors: The metric for each floor, for the baseline and for a maximum of three loadouts.
- Combos: Each set of two or more relics against the sum of its parts. A selector shows the sets of one size. The tab shows only when the data has such a set, each of its relics alone, and the loadout with no relics. The matrix shows the pairs.
- Loadout detail: How the battles of a loadout end, their length, and the cells.

## Statistics

- A mean of a selection has a 95% interval: the Wilson interval for a rate, and the normal interval for the other metrics. This interval takes the games as independent.
- Game `i` of each cell with the same floor and army has the same run seed. Thus a difference between two loadouts is the mean of the differences game by game. The same applies to the player level, the enemy level, and the enemy traits.
- The standard error of a difference comes from the means of the run seeds, not from each game. As a result, more cells with the same run seeds do not make an interval narrower than the data permits.
- The synergy of a set is `x(set) − Σ x(relic) + (size − 1) x(none)` game by game: the set minus the sum of its relics alone. For a pair, this is `x(a+b) − x(a) − x(b) + x(none)`. A positive value means that the set is worth more than the sum of its parts.
- "Beyond smaller sets" is for a set of three or more relics. It is the part of the set that no smaller set of its relics explains: the sum of `x` for each subset of the set, with a minus sign for a subset that lacks an odd number of the relics. For a set of three, this is `x(abc) − x(ab) − x(ac) − x(bc) + x(a) + x(b) + x(c) − x(none)`. The data must have each subset. A large synergy with a small value here means that a pair in the set gives the synergy.
- A table shows the first 500 rows of its sort. The CSV of the view has each row.

## Use the module in a script

`src/analysis.ts` has no dependencies. This script prints the five single relics with the largest paired difference of the win rate at player level 3:

```ts
import { METRICS, formatDiff, loadoutEffects, parseBalanceText } from "./src/analysis";

const result = parseBalanceText(await Bun.file("balance.json").text());
if (result.kind === "error") throw new Error(result.message);
const ds = result.dataset;

const baseline = ds.axes.relics.findIndex((keys) => keys.length === 0);
const level3 = ds.axes.player.indexOf(3);

const top = loadoutEffects(ds, { baseline, filter: { player: [level3] }, metric: METRICS.win })
  .filter((effect) => ds.axes.relics[effect.loadout]?.length === 1)
  .sort((a, b) => b.diff.mean - a.diff.mean)
  .slice(0, 5);

for (const { label, diff } of top) {
  console.log(label, formatDiff(METRICS.win, diff.mean), `(${formatDiff(METRICS.win, diff.lo)} to ${formatDiff(METRICS.win, diff.hi)})`);
}
```

Save the script in this folder and run it with `bun <script>`.

A filter holds indexes into the axes, not the values of the axes. `selectCells` and `groupCells` give the cells of a selection. `summarize`, `pairedDifference`, `synergy`, `interaction`, and `contrast` give the statistics. `findCombos` gives the sets that have their parts in the data. `toCsv` writes a table.
