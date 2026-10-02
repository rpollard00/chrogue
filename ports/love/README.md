# Chrogue in LÖVE

This folder is a client of Chrogue for LÖVE 11.5. It has all the screens of a run: the title, the upgrades, the battle, the camp, and the end of a run.

The client has no game rules. The Rust core in `../../core` has them. The client starts the core, sends commands to it, draws the `view` of each response, and plays the `events` of each response as motion. `../../core/PROTOCOL.md` is the contract between the two.

The client has only the wide layout. The phone layout is a later task.

## Build and run

1. Build the core. In `../../core`, run `cargo build --release`.
2. In this folder, run `love .`. LÖVE 11.5 is necessary. A copy is at `~/.cache/chrogue-tools/love.AppImage`.

The client looks for the core at `../../core/target/release/chrogue-core`, from this folder. To use a different binary, set `CHROGUE_CORE` to its path. If the client does not find the core, the window shows the path that it tried.

The core keeps the saved data in the save folder of LÖVE (`~/.local/share/love/chrogue-love` on Linux): `meta.json` and `run.json`.

## Command line

The options come after the game folder. The client gives `--seed`, `--debug`, `--no-save`, and `--keep-alive` to the core.

- `--size 1440x900`: the size of the window. The default is 1280 by 720.
- `--seed 7`: the seed of the core. The same seed and the same clicks give the same game.
- `--debug`: the core accepts the debug commands of `PROTOCOL.md`. The test scripts use them to prepare positions.
- `--no-save`: the core keeps the saved data in memory only.
- `--save-dir PATH`: the core keeps the saved data in `PATH`, not in the save folder of LÖVE.
- `--keep-alive`: the core continues after the game stops. The game prints the address of the core.
- `--connect HOST:PORT`: the client connects to a core that runs, and does not start a core.
- `--script FILE`: a Lua file of test steps. See "Test scripts".
- `--novsync`: no limit for the frames per second, for a measurement.

## Keys

- `F1` changes the effects: all on, post pass off, all off. The corner of the window shows the mode.
- `Escape` closes the dialog, the promotion picker, and a pinned paper tip. Then it clears the selection of a piece.
- `Enter` does the function of the primary key: Continue run or New run on the title, Continue after a battle, Start the battle in the camp, Buy on the upgrades, and New run at the end of a run. In a dialog, `Enter` is OK.

## The connection to the core

The client starts the core with `--listen 127.0.0.1:0`. It reads the address from the first line of the core, and connects with TCP. The socket does not block: the client reads and writes in each frame. Thus the window continues to draw while the core selects the enemy move.

The client sends one request at a time. The other requests wait in a queue. After each connection, the client sends `view` and opens the screen of the response.

- After the move of the player, the client waits 350 ms, then it sends `enemy_move`. While it waits, the lamp shows "The enemy thinks".
- If the connection is lost, the window shows a panel, and the client tries again each second. If the core that the client started stopped, the client starts a new core with the same save folder.
- When the game stops, it sends `quit` to the core that it started. With `--keep-alive`, the core continues. If the game stops with an error, the core stops when no client connects for 5 seconds.

To continue a battle in a new window:

1. Start the game with `--keep-alive`. Play, then close the window.
2. Read the address in the output, for example `127.0.0.1:43201`.
3. Start the game with `--connect 127.0.0.1:43201`. The same battle continues.

## Test scripts

`--script FILE` gives a Lua file that returns a list of steps. A click goes through `love.mousemoved`, `love.mousepressed`, and `love.mousereleased`, thus it uses the same path as a click of a person. With a script, an error stops the game with exit code 1.

| Step | Function |
|---|---|
| `{ 'click', 'e2' }` | A square of the battle board, or a home square of the camp (`a1` to `h2`). An index from 0 is also correct. |
| `{ 'press', NAME, ARG }` | A named control of the screen or of the dialog. |
| `{ 'play', FUNCTION }` | The function gets the view and gives a move of `view.moves`. The step clicks its two squares, and the first piece of the promotion picker. |
| `{ 'hover', 'medal', 'player', 2 }` | The pointer on a medal of a fan: `player` or `enemy`, and the number of the medal. |
| `{ 'hover', 'stash', 'player' }` | The pointer on a stash. |
| `{ 'hover', 'control', NAME, ARG }` | The pointer on a named control. |
| `{ 'hover', 'none' }` | The pointer leaves the window. |
| `{ 'key', 'f1' }` | A key. |
| `{ 'wait', 0.5 }` | A wait in seconds. |
| `{ 'settle' }` | A wait until no request waits for the core, the enemy moved, and no motion changes the state. |
| `{ 'screen', 'camp' }` | A wait until the screen is `camp` and settled. |
| `{ 'send', { cmd = 'debug_set_floor', floor = 8 } }` | A request to the core, to prepare a test. |
| `{ 'expect', FUNCTION, 'label' }` | The function gets the view, the state of the client, and the app. If it gives false, the game stops with an error. |
| `{ 'fps', 3, 'label' }` | Counts the frames for 3 seconds, and prints the frames per second. |
| `{ 'size', 1920, 1080 }` | The size of the window. |
| `{ 'screenshot', '/absolute/path.png' }` | A screenshot. |
| `{ 'dump', '/absolute/path.json' }` | The last view of the core, the state of the client, the set areas of the screen, the connection, and the frames. |
| `{ 'log', 'text' }`, `{ 'quit' }` | A line on stdout, and the end of the game. |

A `settle` or `screen` step stops the game with an error after 30 seconds.

The named controls are in the `control` function of each screen module:

- Title: `continueRun`, `newRun`, `upgrades`.
- Upgrades: `slot` (a number from 1 to 16, or an upgrade id), `buy`, `back`.
- Battle: `continue`, `giveUp`, `promo` (1 to 4: queen, knight, rook, bishop), `square`, `stash`, `medal`.
- Camp: `take` (1 to 3), `buy` (an item of the shop), `card` and `info` (`{ 'shop', 3 }` or `{ 'reward', 1 }`), `skip`, `reroll`, `start`, `home`, `medal`.
- End of a run: `newRun`, `upgrades`, `title`.
- Dialog: `ok`, `cancel`.

`test/run.sh [folder]` runs all the scripts of `test/`. It writes the screenshots, the dumps, and the log to the folder (default `/tmp/chrogue-love4/run`). Each run opens a window for some seconds.

- `test/flow.lua`: a full session from the title to the title, at 1280 by 720 and 1920 by 1080.
- `test/showcase.lua`: the largest content (the last boss, a full army, each relic) and a won run.
- `test/floor8.lua`: 12 moves against the boss of floor 8. It prints the longest frame while the core selects the enemy move.
- `test/reconnect-a.lua`, `test/reconnect-b.lua`: the reconnection with `--keep-alive` and `--connect`.
- `test/lost.lua`: the core stops, and the client starts a new core.
- `test/fps.lua`: the frames per second on the camp and the battle, in each effects mode.

With `CHROGUE_EFFECTS=off`, `flow.lua` and `showcase.lua` start with the effects off. Use this setting for screenshots that you compare pixel by pixel.

## Structure

| File | Function |
|---|---|
| `main.lua` | The window, the frame, the command line, the screen manager, the input, and the dump |
| `net.lua` | The core process, the socket, the queue of requests, and the reconnection |
| `json.lua` | The JSON reader and writer of the protocol |
| `text.lua` | The interface text for the codes of the protocol. The strings come from `src/ui` |
| `layout.lua` | The set layout of each screen: rectangles in stage units |
| `title.lua`, `upgrades.lua`, `battle.lua`, `camp.lua`, `over.lua` | One module for each screen of the view |
| `board.lua`, `plaques.lua`, `fan.lua`, `result.lua` | The areas of the battle. `fan.lua` is also in the camp |
| `ui.lua` | The shared elements: keys, amounts, medals, cards, the paper tip, pips, tallies, and the dialog |
| `shaders.lua` | The background, the foil, and the post pass |
| `gfx.lua`, `icons.lua`, `theme.lua` | Surfaces, text, pieces, icons, colors, and typefaces |
| `script.lua` | The test hook |
| `test/` | The test scripts and `run.sh` |

Each screen module has the same functions: `new`, `apply` (a new view of the same screen), `refused` (an error response), `update`, `draw`, `hit`, `activate`, `key`, `control`, `settled`, and `state`. A `screen` event opens a new screen object.

The stage is 80 by 45 units. One unit is 1rem of the web game. The stage becomes larger or smaller as one unit. At 1920 by 1080, one unit is 24 pixels. The web game keeps 1rem at 16 pixels at this size.

## Differences from the web game

Where the web game gives an area the size of its content, the client gives the area the size of its largest content (`DESIGN.md`, "The first rule: a set layout"). `layout.lua` names each of these areas:

- The wells of the title and the purses have the width of their largest value.
- The well of the next enemy in the camp has space for 16 pieces. The fan of traits has a set place before it.
- The start key of the camp stays in place when the text below it goes.
- The end of a run has the positions of a won run. The tally well keeps its height when a lost run has one row.
- The text of the upgrade panel has a slot for three lines, and the ladder has a slot for three levels.
- On the battle, the name "You" stays in its row when the lamp goes after the result.

Other differences:

- The relic medals, the relic cards, and the relic cards on the shelves have the foil shader. The background and the post pass are also only in this client.
- Body text uses Fira Sans. The web game uses the typeface of the system.
- The dialogs (Give up, a new run over a saved run) are in the game window. The web game uses the dialog of the browser.
- A screen comes into view with a fade of 0.2 seconds.
- The keyboard has no focus ring and no Tab order. Only `Escape` and `Enter` work.

## What this change removed

The rules of the earlier prototype came from TypeScript compiled to Lua with TypeScriptToLua. The client does not use them now. This change removed:

- `tools/prepare-rules.ts`, `tools/tstl-plugin.cjs`, `package.json`, `bun.lock`, `tsconfig.json`, and `.gitignore` (for `node_modules/`, `generated/`, and `.rules-src/`)
- `rules.lua` and `rules_runtime.lua`, which loaded the compiled rules
- `scenario.lua`, the prepared battles of the prototype
- `screen.lua` and `input.lua`. `battle.lua` and `main.lua` replace them
- `test/run.lua` (the Lua tests of the compiled rules) and the earlier app tests. The Rust core has its own tests

## Typefaces

`fonts/` has Fira Sans and Fira Sans Condensed (SIL Open Font License, `fonts/Fira-OFL.txt`) and DejaVu Sans (`fonts/DejaVu-LICENSE.txt`). The files have no changes. The pieces are the chess glyphs of DejaVu Sans, as in the web game.

## Not yet covered

- The phone layout.
- The client starts the core with `sh` and `kill`, thus it starts the core only on Linux and macOS. On other systems, start the core and use `--connect`.
- The client does not resize the window for a narrow screen. A window that is not 16 by 9 shows the stage in the center.
