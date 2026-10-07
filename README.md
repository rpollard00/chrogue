# Chrogue

Chrogue is a roguelite chess game. Each battle is on a standard 8x8 chess board.

The game has two parts:

- `core/`: The Rust core. It has the chess engine, the enemy AI, the runs, and the saved data.
- `client/`: The LÖVE client. It draws the screens and sends commands to the core.

`web/` builds these two parts for a browser, as one WebAssembly module. This build is the web version of the game.

`DECISIONS.md` records the direction of the project and the reason for each decision.

## Run the game

Rust and LÖVE 11.5 are necessary.

Run `./run.sh`. The script builds the core, then it starts the client. The client starts the core.

The options of the client come after the name of the script, for example `./run.sh --size 1440x900`. `client/README.md` has the options of the command line, the keys, and the location of the saved data.

The script uses `love` from the `PATH`, then the copy at `~/.cache/chrogue-tools/love.AppImage`. To use a different LÖVE, set `LOVE` to its path.

To do the two steps without the script:

1. In `core/`, run `cargo build --release`.
2. In `client/`, run `love .`.

### In a browser

The Emscripten SDK, the Rust target `wasm32-unknown-emscripten`, and Python 3 are also necessary. `web/README.md` has the build prerequisites and installation steps.

Run `web/run.sh`. It builds the game, serves `http://127.0.0.1:8000/`, and opens the page in your browser. Ctrl+C stops the server.

After a change of the client only, run `web/run.sh game`. Use `--no-open` to open the page yourself, or `--port 8001` for another port. `web/README.md` has the other local launch details.

## Measure relics and armies

Run `scripts/balance.sh`. It plays battles between AI levels, makes an HTML report, and opens the report in your browser. The files are `out/balance.json` and `out/balance.html`. Bun is necessary.

The options of the tool `balance` come after the name of the script, for example `scripts/balance.sh --player 2-4` or `scripts/balance.sh --name pairs --floor 4,6,8 --relics none,each,pairs`. `--name` sets the name of the two files, and `--no-open` does not open the browser. `scripts/balance.sh --help` shows the options. `core/README.md` describes them ("Measurements of relics and armies").

## Run the tests

- Core: In `core/`, run `cargo test --release`. `core/README.md` has the other commands: the lint, the speed of the engine, and the self-play matches of the AI.
- Client: Build the core, then run `client/test/run.sh`. Each test opens a window for some seconds, and the full suite takes some minutes.
- Web build: Run `web/build.sh`, then run `bun web/test/run.ts`. The test runs scripts of the client in a Chromium with no window. It takes some minutes and uses the graphics card.

## Rules

- You play White. Your army stays with you from one battle to the next battle.
- A piece that the enemy captures is gone for the rest of the run.
- A pawn that you promote stays promoted.
- You win a battle when you checkmate the enemy king.
- You also win when the enemy king is alone, or when the enemy has no legal move.
- The same conditions apply to you. If you lose a battle, the run ends.
- If 50 moves pass with no capture, the battle is a draw. You continue without a reward. With the upgrade Envoy, a draw before the last floor gives a reward.
- Castling and en passant follow the usual chess rules.

A run has 8 floors. Floor 4 and floor 8 are boss floors, and a boss has traits that change its moves.
After each battle, you select one of three rewards, buy pieces and relics, and arrange your first two ranks.
A run has 4 relic slots. In the camp, you can discard a relic to make a slot free. You get no gold for it.
Each run gives crowns. Crowns buy permanent upgrades and unlock relics. You also unlock relics with achievements in a battle. A run offers only the relics that you unlocked.

## Structure

- `core/engine/`: Chess. It has the movement rules as data, the move generation, the result of a battle, and the enemy AI. It does not know about runs or relics.
- `core/game/`: The roguelite. It has runs, battles, relics, upgrades, offers, floors, saved data, and the session that runs the commands of a client.
- `core/server/`: The program `chrogue-core`. It gives the commands of `core/PROTOCOL.md` on stdio or on a local TCP socket.
- `core/embed/`: The library `chrogue_core`. It gives the same commands as a C interface, for a client that loads the core into its own process.
- `core/tools/`: Tools for the engine and the game: a timer, self-play matches, the move of the AI for one position, and battles that measure relics and armies. `core/tools/report/` makes an HTML report from these battles.
- `client/`: The client. It has the screens, the motion, and the shaders. It has no game rules.
- `web/`: The build of the client and the core for a browser, its page, its test, and the files of the deployment.

A relic changes a battle in two ways. Its movement rules are data that the engine and the AI read. Its effect runs at a fixed point of a battle.

`core/README.md` describes the engine, the AI, and the game layer. `core/PROTOCOL.md` describes the commands, the views, and the events.

## Add content

The content is in `core/game/src/content.rs`.

- Relic: Add one entry to `RELICS`. Its movement rules are a list of `RuleEdit`. Its effect is one kind of `Effect`.
- Relic effect at a new point of a battle: Add a kind to `Effect`, and use it in `core/game/src/battle.rs`.
- Movement rule: `RuleEdit::Leap` gives a jump to a kind. `RuleEdit::Slide` gives it a slide of 1 to `steps` steps. For another rule, write the rule as an edit of `SideRules` in the engine (`core/README.md`, "Add a movement rule"), and add a kind to `RuleEdit` for it.
- Upgrade: Add one entry to `UPGRADES`.
- Floor: Add one entry to `FLOORS`.

Saved data keeps relic ids and upgrade ids. If you remove an id, the core removes it from saved data when it loads the data.

## Deploy to Unraid

The container on the Unraid server has the web build.

Run `web/deploy-unraid`. The script does these steps without a confirmation prompt:

1. It runs the tests of the core (`cargo test --release`) and builds the web build (`web/build.sh`) on this workstation. It does not run the test of the web build, which takes some minutes.
2. It builds the image on this workstation. The image has BusyBox `httpd` and the files of `web/dist`.
3. It sends the image to `root@media.media` through SSH. It does not use a registry.
4. It replaces the container `chrogue` on network `br0` at `192.168.88.13`, port 80.
5. It makes sure that the server gives the page and `love.wasm`.

The container name is always `chrogue`. If a container with that name exists and this script did not make it, the deployment stops.
To change the target, set `CHROGUE_SSH_TARGET`, `CHROGUE_NETWORK`, or `CHROGUE_ADDRESS`.

The saved data of a player is in the browser of the player, for the address of the game. The web build does not read the saved data of the TypeScript game that the server had before.

The web build has only the wide layout. The game has no layout for a phone at this time.

`web/Dockerfile`, `web/httpd.conf`, and `web/deploy-unraid` are the files of the deployment.
