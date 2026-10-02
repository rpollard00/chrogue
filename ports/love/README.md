# Chrogue: the LÖVE port of the battle screen

This folder is a prototype. It draws the battle screen of Chrogue in LÖVE 11.5 with three shaders.
It has one prepared battle. It does not have the title, the camp, or the upgrades.

The rules are not in this folder. `bun run build:lua` makes them from `../../src/engine` and `../../src/game`
with TypeScriptToLua. The result goes to `generated/`. That folder is not in the repository.

## Build and run

1. Run `bun install` in this folder.
2. Run `bun run build:lua`. Do this step again after each change in `../../src`.
3. Run `love .` in this folder. LÖVE 11.5 is necessary. `bun run start` uses the copy in `~/.cache/chrogue-tools/`.

`F1` changes the effects: all on, post pass off, all off. The corner of the window shows the mode.

## Command line

The options come after the game folder.

- `--size 1440x900`: the size of the window. The default is 1280 by 720.
- `--demo NAME`: a prepared position: `mate`, `promo`, `check`, `defeat`, or `draw`. The default is the boss of floor 4.
- `--seed 7`: the seed of the random numbers of the enemy AI. With a seed, the enemy gives the same answers in each session.
- `--script FILE`: a Lua file that returns a list of steps. `script.lua` has the list of the steps.
- `--novsync`: no limit for the frames per second, for a measurement.

## Tests

- `bun run test`: the proof for the generated rules. It runs in `luajit` with no window. It has the perft counts and
  the rule cases of `../../test/engine.test.ts`, battle cases of `../../test/game.test.ts`, and the moves that the TypeScript AI selects.
- `bun run test:app`: drives the port with the scripts of `test/app/`. Each run opens a window for some seconds.
  It saves screenshots and state files in `/tmp/chrogue-ports/love/`, and it compares the state files with the expected state.

## Structure

| File | Function |
|---|---|
| `main.lua` | The window, the frame, the command line, and the state file |
| `rules.lua`, `rules_runtime.lua` | Load the generated rules, and the functions that they need |
| `scenario.lua` | The prepared battles |
| `screen.lua` | The state and the behavior of the battle. It is the port of `viewBattle` |
| `layout.lua` | The set layout: one table of rectangles in stage units |
| `board.lua`, `plaques.lua`, `fan.lua`, `result.lua` | One module for each area |
| `shaders.lua` | The background, the foil, and the post pass |
| `input.lua` | The control at a point, and the function of a click |
| `gfx.lua`, `icons.lua`, `theme.lua` | Surfaces, text, pieces, icons, colors, and typefaces |
| `script.lua` | The test hook |
| `tools/` | The two steps of the build |

The stage is 80 by 45 units. One unit is 1rem of the web game. The stage becomes larger or smaller as one unit.

## How the build changes the rules

Lua and JavaScript give different results for some code. The build corrects these places from the types of the source:

- `tools/prepare-rules.ts` copies the rules to `.rules-src/`. It puts a condition that can be `0` or `''` into `__truthy(...)`,
  because Lua has these values as true. It changes `Object.keys` of a constant object to a list, because Lua does not keep the sequence of keys.
  It stops with an error at a place that it cannot change safely.
- `tools/tstl-plugin.cjs` corrects a loop and a spread of a list that can have `null` items (the board), and it adds `Object.hasOwn`.
- `rules_runtime.lua` has a sort that keeps the sequence of equal items, as the sort of JavaScript. The enemy AI needs it to select the same move.

## Typefaces

`fonts/` has Fira Sans and Fira Sans Condensed (SIL Open Font License, `fonts/Fira-OFL.txt`) and DejaVu Sans
(`fonts/DejaVu-LICENSE.txt`). The files have no changes. The pieces are the chess glyphs of DejaVu Sans, as in the web game.
