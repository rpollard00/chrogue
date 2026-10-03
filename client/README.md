# Chrogue in LÖVE

This folder is a client of Chrogue for LÖVE 11.5. It has all the screens of a run: the title, the upgrades, the battle, the camp, and the end of a run.

The client has no game rules. The Rust core in `../core` has them. The client starts the core, sends commands to it, draws the `view` of each response, and plays the `events` of each response as motion. `../core/PROTOCOL.md` is the contract between the two.

The client has only the wide layout. The phone layout is a later task.

## Build and run

1. Build the core. In `../core`, run `cargo build --release`.
2. In this folder, run `love .`. LÖVE 11.5 is necessary. A copy is at `~/.cache/chrogue-tools/love.AppImage`.

The client looks for the core at `../core/target/release/chrogue-core`, from this folder. To use a different binary, set `CHROGUE_CORE` to its path. If the client does not find the core, the window shows the path that it tried.

The core keeps the saved data in the save folder of LÖVE (`~/.local/share/love/chrogue-love` on Linux): `meta.json` and `run.json`.

## Command line

The options come after the game folder. The client gives `--seed`, `--debug`, `--no-save`, and `--keep-alive` to the core.

- `--size 1440x900`: the size of the window. The default is 1280 by 720.
- `--seed 7`: the seed of the core. The same seed and the same clicks give the same game.
- `--debug`: the core accepts the debug commands of `PROTOCOL.md`. The test scripts use them to prepare positions.
- `--no-save`: the core keeps the saved data in memory only.
- `--save-dir PATH`: the core keeps the saved data in `PATH`, not in the save folder of LÖVE.
- `--keep-alive`: the core continues after the game stops. The game prints the address of the core.
- `--connect HOST:PORT`: the client connects to a core that runs, and does not start a core. Set `CHROGUE_TOKEN` to the token of that core.
- `--no-auth`: the client accepts a core that does not check the token of a connection. See "The token".
- `--embed`: the core is in the process of the game, and the client uses no socket. See "The core in the process of the game".
- `--script FILE`: a Lua file of test steps. See "Test scripts".
- `--novsync`: no limit for the frames per second, for a measurement.

## Keys

- `F1` changes the effects: all on, post pass off, all off. The corner of the window shows the mode.
- `Escape` closes the dialog, the promotion picker, and a pinned paper tip. Then it clears the selection of a piece.
- `Enter` does the function of the primary key: Continue run or New run on the title, Continue after a battle, Start the battle in the camp, Buy on the upgrades, and New run at the end of a run. In a dialog, `Enter` is OK.

A new screen takes no click and no key (other than `F1`) during its fade of 0.2 seconds. Thus the second click of a double click, or a second `Enter`, does not act on the next screen. A press of the button on one screen and its release on a different screen is not a click.

## The connection to the core

The client starts the core with `--listen 127.0.0.1:0`. It reads the address from the first line of the core, and connects with TCP. The socket does not block: the client reads and writes in each frame. Thus the window continues to draw while the core selects the enemy move.

The client sends one request at a time. The other requests wait in a queue. After each connection, the client sends `view` and opens the screen of the response.

- After the move of the player, the client waits 350 ms, then it sends `enemy_move`. While it waits, the lamp shows "The enemy thinks". If the player gives up during the pause, the client does not send `enemy_move`.
- If the core refuses `enemy_move`, the client sends it again one time. After a second refusal, the lamp shows "The enemy move failed.", and Give up stays available.
- A request has 5 seconds for its response (`enemy_move` has 15 seconds). After this time, the client closes the connection, as for a lost connection.
- If the connection is lost, the window shows a panel, and the client tries again each second. If the core that the client started stopped, the client starts a new core with the same save folder.
- When the game stops, it sends `quit` to the core that it started. With `--keep-alive`, the core continues. If the game stops with an error, the core stops when no client connects for 5 seconds.
- Before the game stops its core with `kill`, it checks that the process number is still the core: `/proc/PID/cmdline` on Linux, `ps -p PID -o comm=` on macOS. It does not stop a different process.
- A `save_failed` or `save_problem` event gives a notice at the top left corner of the window. The notice does not stop the game. It goes after 14 seconds, or when the player clicks it. The notice of `save_problem` gives the path of the file that the core kept aside.

### The core in the process of the game

With `--embed`, the client loads the library of the core (`../core/embed`) with the FFI of LuaJIT, and starts no program. In a browser (`../web`), the client always uses this mode, and the library is a part of the build.

1. Build the library. In `../core`, run `cargo build --release`.
2. In this folder, run `love . --embed`.

The client looks for the library at `../core/target/release/libchrogue_core.so`, from this folder (`libchrogue_core.dylib` on macOS, `chrogue_core.dll` on Windows). To use a different file, set `CHROGUE_CORE_LIB` to its path.

- `--seed`, `--debug`, `--no-save`, and `--save-dir` have the same function. `--connect`, `--keep-alive`, and `--no-auth` have no function.
- The client sends one request in each frame, and the core answers in the call. No token, no time limit, and no reconnection are necessary.
- While the core selects the enemy move, the window does not draw. On floor 8, this time is 0.1 to 0.2 seconds.
- If the library is missing, or if another core holds the lock of the save folder, the window shows the reason.

### The token

Each connection starts with a token, thus a different program on the computer cannot send commands to the core.

1. When the client starts the core, it makes a token of 64 hex digits from `/dev/urandom` (if it can read it) and from `love.math.newRandomGenerator` with the time as its seed.
2. The client gives the token to the core only in the environment variable `CHROGUE_TOKEN`. The shell reads the token from its input, thus the token is not in the command line of a process.
3. The first line of each connection is `{"auth":"TOKEN"}`. The client waits for `{"ok":true,"auth":true}` before it sends a command. If the core answers with an error, the client stops with a panel that shows the error.
4. With `--connect`, the client reads the token from its own `CHROGUE_TOKEN`.
5. With `--no-auth`, the client continues when the core answers the token with an error. Use it only with a core that does not check tokens. With `--no-auth` and no `CHROGUE_TOKEN`, the client sends no token.

To continue a battle in a new window:

1. Start the game with `--keep-alive`. Play, then close the window.
2. Read the address and the token in the output, for example `127.0.0.1:43201` and `CHROGUE_TOKEN=3f9a…`.
3. Start the game with `CHROGUE_TOKEN=3f9a…` in its environment and `--connect 127.0.0.1:43201`. The same battle continues.

## Test scripts

`--script FILE` gives a Lua file that returns a list of steps. The file is a path on the disk, or a path in the game folder (`test/flow.lua`) for a game in one file. A click goes through `love.mousemoved`, `love.mousepressed`, and `love.mousereleased`, thus it uses the same path as a click of a person. With a script, an error stops the game with exit code 1.

| Step | Function |
|---|---|
| `{ 'click', 'e2' }` | A square of the battle board, or a home square of the camp (`a1` to `h2`). An index from 0 is also correct. |
| `{ 'press', NAME, ARG }` | A named control of the screen or of the dialog. |
| `{ 'play', FUNCTION }` | The function gets the view and gives a move of `view.moves`. The step clicks its two squares. In the promotion picker, it clicks the piece of the move (`promo`). |
| `{ 'again' }` | A click at the point of the last click, for a double click. |
| `{ 'down', NAME, ARG }`, `{ 'up' }` | A press of the button on a named control, and its release at the same point. |
| `{ 'refusal', 'enemy_move', 'internal', 2 }` | The core can refuse this command with this code (here 2 times). Each other refusal stops the script with an error. |
| `{ 'events', LIST }` | Events as in a response, for the events that the core cannot make in a test. |
| `{ 'respond', FUNCTION }` | The function gets the app and gives a response and its request, as from the core. |
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
| `{ 'log', 'text' }`, `{ 'quit' }` | A line on stdout, and the end of the game. `quit` writes the line "The script is at its end." |

A `settle` or `screen` step stops the game with an error after 30 seconds.

The named controls are in the `control` function of each screen module:

- Title: `continueRun`, `newRun`, `upgrades`.
- Upgrades: `slot` (a number from 1 to 16, or an upgrade id), `buy`, `back`.
- Battle: `continue`, `giveUp`, `promo` (1 to 4, or a kind: `'q'`, `'n'`, `'r'`, `'b'`), `square`, `stash`, `medal`.
- Camp: `take` (1 to 3), `buy` (an item of the shop), `card` and `info` (`{ 'shop', 3 }` or `{ 'reward', 1 }`), `skip`, `reroll`, `start`, `home`, `medal`.
- End of a run: `newRun`, `upgrades`, `title`.
- Dialog: `ok`, `cancel`.
- Notice: `notice` (the number of the notice, from 1).

`test/run.sh [folder]` runs all the scripts of `test/`. It writes the screenshots, the dumps, and the log to the folder (default `/tmp/chrogue-love4/run`). Each run opens a window for some seconds. With `CHROGUE_NO_AUTH=1`, each run has `--no-auth`.

The run fails if a game stops with an error, if the core refused a command that a script did not expect, or if a core of this game continues after the tests.

- `test/flow.lua`: a full session from the title to the title, at 1280 by 720 and 1920 by 1080, and one time with `--embed`.
- `test/showcase.lua`: the largest content (the last boss, a full army, each relic) and a won run.
- `test/floor8.lua`: 12 moves against the boss of floor 8. It prints the longest frame while the core selects the enemy move.
- `test/reconnect-a.lua`, `test/reconnect-b.lua`: the reconnection with `--keep-alive` and `--connect`.
- `test/lost.lua`: the core stops, and the client starts a new core.
- `test/fps.lua`: the frames per second on the camp and the battle, in each effects mode.
- `test/input.lua`: a double click on Continue and on Buy, two presses of `Enter` after a draw, and a release on a new screen. The shop cards keep their slots after a purchase.
- `test/battle.lua`: a battle that starts with no legal move, Give up during the pause before the enemy move, refusals of `enemy_move`, and a promotion to a knight.
- `test/saves.lua`: the notices of `save_failed` (a save folder that cannot take a file) and of `save_problem`.
- `test/icons.lua`: the SVG path reader of `icons.lua`.
- `test/saved-a.lua`, `test/saved-b.lua`: with `--embed`, a game starts a run and quits, and a second game on the same save folder continues the run.

With `CHROGUE_EFFECTS=off`, `flow.lua` and `showcase.lua` start with the effects off. Use this setting for screenshots that you compare pixel by pixel.

## Structure

| File | Function |
|---|---|
| `main.lua` | The window, the frame, the command line, the screen manager, the input, and the dump |
| `net.lua` | The core process, the socket, the queue of requests, and the reconnection. With `--embed`, the queue and the calls to `core.lua` |
| `core.lua` | The core in the process of the game: the library through the FFI of LuaJIT, or the linked module of the WebAssembly build |
| `json.lua` | The JSON reader and writer of the protocol |
| `text.lua` | The interface text for the codes of the protocol |
| `layout.lua` | The set layout of each screen: rectangles in stage units |
| `title.lua`, `upgrades.lua`, `battle.lua`, `camp.lua`, `over.lua` | One module for each screen of the view |
| `board.lua`, `plaques.lua`, `fan.lua`, `result.lua` | The areas of the battle. `fan.lua` is also in the camp |
| `ui.lua` | The shared elements: keys, amounts, medals, cards, the paper tip, pips, tallies, and the dialog |
| `shaders.lua` | The background, the foil, and the post pass |
| `gfx.lua`, `icons.lua`, `theme.lua` | Surfaces, text, pieces, icons, colors, and typefaces |
| `script.lua` | The test hook |
| `test/` | The test scripts and `run.sh` |

Each screen module has the same functions: `new`, `apply` (a new view of the same screen), `refused` (an error response), `update`, `draw`, `hit`, `activate`, `key`, `control`, `settled`, and `state`. A `screen` event opens a new screen object.

The stage is 80 by 45 units. The stage becomes larger or smaller as one unit. At 1280 by 720, one unit is 16 pixels. At 1920 by 1080, one unit is 24 pixels.

## Pixels and precision

- The scene and the foil go to canvases with 4 samples for each pixel. A system with no such canvas (WebGL 1) gets canvases with 2 pixels or more for each unit of the window, and the window shows them smaller. Thus the edges are smooth on each system.
- On a window with a high pixel density, the canvases and the fonts have the pixels of the screen. Only the build for a browser asks for such a window (`conf.lua`).
- The shaders ask for floats of high precision. OpenGL ES (a browser, a phone) gives medium precision without this, and the background then has no noise.

## Set areas

Each area has the size of its largest content (`DESIGN.md`, "The first rule: a set layout"). `layout.lua` names each of these areas:

- The wells of the title and the purses have the width of their largest value.
- The well of the next enemy in the camp has space for 16 pieces. The fan of traits has a set place before it.
- The start key of the camp stays in place when the text below it goes.
- The reward shelf has 3 set card slots, and the shop has 4. A card keeps its slot when the player buys a different card, and a bought card leaves an empty slot. After a new connection, the cards fill the slots from the left. When a shelf has no card, its heading line tells it.
- The end of a run has the positions of a won run. The tally well keeps its height when a lost run has one row.
- The text of the upgrade panel has a slot for three lines, and the ladder has a slot for three levels.
- On the battle, the name "You" stays in its row when the lamp goes after the result.

## Other properties of the interface

- The relic medals, the relic cards, and the relic cards on the shelves have the foil shader.
- The dialogs (Give up, a new run over a saved run) are in the game window.
- A screen comes into view with a fade of 0.2 seconds. During the fade, the screen takes no input.
- A battle that starts with no legal move for the player shows "No legal move. Give up." in the lamp (core/PROTOCOL.md, "Open issues").
- The keyboard has no focus ring and no Tab order. Only `Escape` and `Enter` work.

## Typefaces

`fonts/` has Fira Sans and Fira Sans Condensed (SIL Open Font License, `fonts/Fira-OFL.txt`) and DejaVu Sans (`fonts/DejaVu-LICENSE.txt`). The files have no changes. Body text uses Fira Sans. The pieces are the chess glyphs of DejaVu Sans.

## Not yet covered

- The phone layout.
- The client starts the core with `sh` and `kill`, thus it starts the core only on Linux and macOS. On other systems, start the core and use `--connect`.
- The client does not resize the window for a narrow screen. A window that is not 16 by 9 shows the stage in the center.
