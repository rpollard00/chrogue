# Chrogue

Chrogue is a roguelite chess game. The game has two parts:

- `core/`: The Rust core. It has the chess engine, the enemy AI, the runs, and the saved data.
- `ports/love/`: The LÖVE client. It draws the screens and sends commands to the core.

`ports/web/` builds the two parts for a browser as one WebAssembly module. It is the web version of the game, and it has no code of the game.

The TypeScript browser game in `src/` is deprecated. Do not add features to it. The core has the rules of the game.

`DECISIONS.md` records the direction of the project. `README.md` has the commands and the structure of the code. `core/PROTOCOL.md` is the contract between the core and a client.

## The rules of the game are in the core

- The client has no game rules. It draws the `view` of the core and plays its `events`.
- A rule that changes how a piece moves, or how a battle ends, is data of the engine in `core/engine`. Thus the enemy AI plays the changed rule. Do not put such a rule in the game layer or in the client.

## The interface has a set layout

Read `DESIGN.md` before you change the interface. Its first rule applies to each change:

- Chrogue is a game, not a web page. The game has two set layouts: one for a wide screen, and one for a phone.
- A layout does not change with its content or with the state of the game. No area moves, wraps, shows, hides, or changes its size. The board has the same size in each battle.
- Each area has a size for the largest content that the game can give it. An area with no content stays as an empty slot.
- Only an element above the layout (a relic card, a list, a tooltip, a dialog) can have a size that comes from its content.
- A layout becomes larger or smaller only as one unit: the stage unit of the client (`gfx.u`), or the size of `1rem` in `style.css` of the browser game. Do not add breakpoints, wraps, or code that measures elements.

If a change needs more space than its area has, change the design of the area. Do not let the layout move.

The client has only the wide layout at this time. `ports/love/layout.lua` has the areas of each screen.

## Other rules

- The game does not show a rules screen or a list of relics. The player finds these in a run.
- Use `jj`, not `git`. Make one change for each feature, with a Conventional Commit description.
- Core: In `core/`, run `cargo test --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check` before you finish.
- Client: Look at the result in the client at 1440×900 (`love . --size 1440x900` in `ports/love/`). Run `ports/love/test/run.sh` only when the user tells you to: it opens many windows and takes some minutes.
- Web build: If you change `ports/web/`, the library `core/embed`, or how the client draws, run `ports/web/build.sh` before you finish, and look at the result in a browser at 1440×900. `bun ports/web/test/run.ts` uses the graphics card for some minutes. Ask the user before you run it.
- Browser game: If you must change the deprecated game, use Bun and Vite. Run `bun run check` and `bun test`, and look at the result in a browser at 1440×900 and at 390×844. `gallery.html` has each screen with prepared data.
