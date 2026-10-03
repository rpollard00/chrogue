# Chrogue

Chrogue is a roguelite chess game. Each battle is on a standard 8x8 chess board.

The game has two parts:

- `core/`: The Rust core. It has the chess engine, the enemy AI, the runs, and the saved data.
- `ports/love/`: The LÖVE client. It draws the screens and sends commands to the core.

`ports/web/` builds these two parts for a browser, as one WebAssembly module. This build is the web version of the game.

The TypeScript browser game in `src/` is deprecated. The last section of this file has its commands.

`DECISIONS.md` records the direction of the project and the reason for each decision.

## Run the game

Rust and LÖVE 11.5 are necessary.

1. In `core/`, run `cargo build --release`.
2. In `ports/love/`, run `love .`.

The client starts the core. `ports/love/README.md` has the options of the command line, the keys, and the location of the saved data.

### In a browser

The Emscripten SDK and the Rust target `wasm32-unknown-emscripten` are also necessary. `ports/web/README.md` has the steps to install them.

1. Run `ports/web/build.sh`.
2. Give the folder `ports/web/dist` to a static server, and open its page.

## Run the tests

- Core: In `core/`, run `cargo test --release`. `core/README.md` has the other commands: the lint, the speed of the engine, and the self-play matches of the AI.
- Client: Build the core, then run `ports/love/test/run.sh`. Each test opens a window for some seconds, and the full suite takes some minutes.
- Web build: Run `ports/web/build.sh`, then run `bun ports/web/test/run.ts`. The test runs scripts of the client in a Chromium with no window. It takes some minutes and uses the graphics card.

## Rules

- You play White. Your army stays with you from one battle to the next battle.
- A piece that the enemy captures is gone for the rest of the run.
- A pawn that you promote stays promoted.
- You win a battle when you checkmate the enemy king.
- You also win when the enemy king is alone, or when the enemy has no legal move.
- The same conditions apply to you. If you lose a battle, the run ends.
- If 50 moves pass with no capture, the battle is a draw. You continue without a reward.
- Castling and en passant follow the usual chess rules.

A run has 8 floors. Floor 4 and floor 8 are boss floors, and a boss has traits that change its moves.
After each battle, you select one of three rewards, buy pieces and relics, and arrange your first two ranks.
Each run gives crowns. Crowns buy permanent upgrades on the title screen.

## Structure

- `core/engine/`: Chess. It has the movement rules as data, the move generation, the result of a battle, and the enemy AI. It does not know about runs or relics.
- `core/game/`: The roguelite. It has runs, battles, relics, upgrades, offers, floors, saved data, and the session that runs the commands of a client.
- `core/server/`: The program `chrogue-core`. It gives the commands of `core/PROTOCOL.md` on stdio or on a local TCP socket.
- `core/embed/`: The library `chrogue_core`. It gives the same commands as a C interface, for a client that loads the core into its own process.
- `core/tools/`: Tools for the engine: a timer, self-play matches, and the move of the AI for one position.
- `ports/love/`: The client. It has the screens, the motion, and the shaders. It has no game rules.
- `ports/web/`: The build of the client and the core for a browser, its page, and its test.

A relic changes a battle in two ways. Its movement rules are data that the engine and the AI read. Its effect runs at a fixed point of a battle.

`core/README.md` describes the engine, the AI, and the game layer. `core/PROTOCOL.md` describes the commands, the views, and the events.

## Add content

The content is in `core/game/src/content.rs`.

- Relic: Add one entry to `RELICS`. Its movement rules are a list of `RuleEdit`. Its effect is one kind of `Effect`.
- Relic effect at a new point of a battle: Add a kind to `Effect`, and use it in `core/game/src/battle.rs`.
- Movement rule: Write the rule as an edit of `SideRules` in the engine (`core/README.md`, "Add a movement rule"), and add a kind to `RuleEdit` for it.
- Upgrade: Add one entry to `UPGRADES`.
- Floor: Add one entry to `FLOORS`.

Saved data keeps relic ids and upgrade ids. If you remove an id, the core removes it from saved data when it loads the data.

## Deploy to Unraid

The container on the Unraid server has the web build.

Run `bun run deploy:unraid`. The command does these steps without a confirmation prompt:

1. It runs the tests of the core (`cargo test --release`) and builds the web build (`ports/web/build.sh`) on this workstation. It does not run the test of the web build, which takes some minutes.
2. It builds the image on this workstation. The image has BusyBox `httpd` and the files of `ports/web/dist`.
3. It sends the image to `root@media.media` through SSH. It does not use a registry.
4. It replaces the container `chrogue` on network `br0` at `192.168.88.13`, port 80.
5. It makes sure that the server gives the page and `love.wasm`.

The container name is always `chrogue`. If a container with that name exists and this script did not make it, the deployment stops.
To change the target, set `CHROGUE_SSH_TARGET`, `CHROGUE_NETWORK`, or `CHROGUE_ADDRESS`.

The saved data of a player is in the browser of the player, for the address of the game. The web build does not read the saved data of the TypeScript game.

The web build has only the wide layout. The TypeScript game also had a layout for a phone.

`Dockerfile`, `httpd.conf`, and `scripts/deploy-unraid` are the files of the deployment.

## The browser game (deprecated)

The TypeScript game in `src/` is the first version of Chrogue. The core came from it. Do not add features to it.

The scripts in `core/difftest/` and `core/gametest/` compare the core with this game. They stay only while this game is in the repository.

### Run the game

The project uses Bun and Vite.

1. Run `bun install`.
2. Run `bun run dev`.
3. Open `http://localhost:5188`.

`bun run build` writes the production files to `dist/`. `bun run preview` serves those files. The deployment does not use them: the container has the web build.

### See the animations

The animation gallery shows each screen with prepared data and plays its motion.

1. Run `bun run dev`.
2. Open `http://localhost:5188/gallery.html`.
3. Select a demo. Select **Replay** to see the demo again.

The gallery saves nothing, and the production build does not include it.

### Change the saved data

The debug menu changes the saved data at no cost. The development server always has it.

1. Run `bun run dev`.
2. Select **Debug** in the top right corner, or press the `` ` `` key.
3. Change the items. The menu saves each change immediately.
4. Select **Close**. The screen shows the changes. If you are in a battle, the battle starts again.

The menu has three sections:

- Progress: The crowns and the level of each upgrade.
- Run: The floor, the gold, the army, the relics of the player, and the traits of the enemy.
- Offers: The relics that the game can offer. A relic that is off is not a reward, a shop item, or a boss trait.

The game applies the Offers section only while the menu is on the page.

A production build shows the menu only when the address has `?debug`.

### Run the tests

Run `bun test`. Run `bun run check` for the TypeScript type check.

### Structure

The code has three layers. Each layer imports only from the layers before it in this list.

- `src/engine/`: Chess. It has the rules, the move generation, and the enemy AI. It does not know about runs or relics.
- `src/game/`: The roguelite. It has runs, battles, relics, upgrades, offers, floors, and saved data. It has no DOM code.
- `src/ui/`: The screens. `src/main.ts` owns the state and changes the screen. `src/ui/debug.ts` is the debug menu.

A relic changes a battle in two ways. Its `rules` add movement rules for the engine. Its `hooks` run at fixed points of a battle.

### Add content

- Relic: Add one entry to `defs` in `src/game/relics.ts`. If the entry has a `foeText`, a boss can have it as a trait.
- Relic effect at a new point of a battle: Add a hook to `RelicHooks`, and call it in `src/game/battle.ts`.
- Movement rule: Add a flag to `MoveRules` in `src/engine/types.ts`, and use it in `src/engine/rules.ts`.
- Upgrade: Add one entry to `defs` in `src/game/upgrades.ts`.
- Kind of reward or shop item: Add its data to `Offer` in `src/game/types.ts`. Then obey the steps at the top of `src/game/offers.ts`.
- Floor: Add one entry to `FLOORS` in `src/game/floors.ts`.

Saved data keeps relic ids and upgrade ids. If you remove an id, `src/game/storage.ts` removes it from saved data when the game loads.

### Files

- `test/engine.test.ts`: Perft counts, movement rules, battle results, and AI checks.
- `test/game.test.ts`: Relics, upgrades, offers, battle results in a run, and saved data.
