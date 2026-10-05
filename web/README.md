# Chrogue in a browser

This folder builds the LÖVE client (`../client`) and the Rust core (`../core`) for a browser. The result is one WebAssembly module, the game as one file, and one page.

The web build is the same game as the desktop game. It has no game rules of its own and no interface of its own. The page is only the window of the game.

## How it works

- LÖVE for a browser has no LuaJIT, thus the client cannot load the core with the FFI. The build links the library of the core (`../core/embed`) into LÖVE as the Lua module `chrogue_core`. `chrogue_core_lua.c` is that module.
- The client always has the core in its process in a browser (`../client/README.md`, "The core in the process of the game"). It sends the same requests as on the desktop.
- LÖVE comes from the source of a port of LÖVE 11.5 to Emscripten: `alexjgriffith/megasource` and `alexjgriffith/love`, at the commits in `build.sh`. This port uses WebAssembly exceptions, as the library of the core does. The usual `love.js` does not, thus it cannot have the core.
- The page has no threads. A page with threads needs HTTP headers that the server of the game (BusyBox `httpd`) does not send. Thus the core selects the enemy move in the frame, and the picture stops for 0.1 to 0.2 seconds at the strongest level of the AI. At the levels that the floors have by default, a move is usually done in less than 1 ms.
- The game draws with WebGL 1. See "Limits".

## Build

These tools are necessary:

- The Emscripten SDK. The build looks for it at `~/.cache/chrogue-tools/emsdk`. Set `EMSDK` for a different folder. The build was made with Emscripten 6.0.11.
- Rust with the target `wasm32-unknown-emscripten`: `rustup target add wasm32-unknown-emscripten`. The build was made with Rust 1.98.
- `bash`, `git`, `cmake`, `ninja`, `bsdtar`, and `sha256sum`.

The Emscripten version and the Rust version go together: the library of the core uses the Emscripten library of its Rust release. If the link step fails after an update of one of them, update the other.

To install the Emscripten SDK:

1. Run `git clone https://github.com/emscripten-core/emsdk.git ~/.cache/chrogue-tools/emsdk`.
2. In that folder, run `./emsdk install 6.0.11`.
3. Run `./emsdk activate 6.0.11`.

To build the game:

1. Run `web/build.sh`. The first build gets the source of LÖVE and takes some minutes. A later build makes only the parts that changed.
2. Read the last line. It gives the folder of the result: `web/dist`.

After a change of `client` only, run `web/build.sh game`. It makes `game.love` and the page again, in less than a second.

`build.sh` keeps the source of LÖVE and its build in `~/.cache/chrogue-tools/web`. Set `CHROGUE_WEB_WORK` for a different folder. `CHROGUE_WEB_DIST` sets a different folder for the result.

## The result

`web/dist` has four files. A server gives them as static files.

| File | Function |
|---|---|
| `index.html` | The page: a canvas that fills the window, and the start of the game. The addresses of the other files have a stamp of the build |
| `love.js`, `love.wasm` | LÖVE and the core |
| `game.love` | The files of `client`, without its tests |

The server must give `love.wasm` with the type `application/wasm`. `httpd.conf` does this for the container of the game.

A new build has a new stamp. Thus a browser does not use `love.wasm` of an old build with `love.js` of a new one. The address of `index.html` has no stamp: a browser can show the page of the old build for some time after a deployment.

To look at the game, give the folder to a static server, and open the page. For example, with Bun: `bunx serve web/dist`.

## Options

Five options of the client come from the address of the page: `seed`, `debug`, `no-save`, `script`, and `background`. `?seed=7&debug` is `--seed 7 --debug`. `../client/README.md` tells their functions. The page does not give the other options to the game: they are for a window or for a socket.

## Saved data

The core keeps `meta.json` and `run.json` in the save folder of LÖVE. In a browser, that folder is in the IndexedDB of the browser, for the address of the page. The page gets the saved files before the game starts, and each change of the folder goes to the browser.

- The saved data is for one browser and one address. A different browser, or a different address of the same game, has its own data.
- Two pages of the game in one browser do not see the changes of each other. The page that saves last sets the data.
- If the browser has no IndexedDB, or does not give the saved files in 10 seconds, the game runs with the save folder in memory, and the data that the browser keeps does not change. The game then starts with no saved data each time. The console of the page has the reason. The game does not tell the player.

## The size of the game

The canvas fills the window of the browser. The stage of the game stays 16 by 9 in the center, as on the desktop (`DESIGN.md`, "The first rule: a set layout"). The game has only the wide layout, also on a phone.

On a screen with a high pixel density, the canvas has the pixels of the screen.

## Tests

`bun web/test/run.ts` runs test scripts of the client (`client/test`) in the web build, in a Chromium with no window. Build the game first.

- `flow`: a full session from the title to the title, at 1440 by 900. `flow-2x`: the same on a screen with 2 pixels for each CSS pixel.
- `battle`, `input`, `floor8`: the scripts with these names.
- `saved-a`, `saved-b`: a page starts a run, and a second page on the same address continues it.

The run fails if a script stops with an error, if the page has an error, if the core refuses a command that a script did not expect, or if a script does not end in 4 minutes. The screenshots and the dumps of the scripts go to `/tmp/chrogue-web/run` (set `CHROGUE_OUT` for a different folder).

`bun web/test/run.ts flow` does only the run `flow`. `CHROME` sets the browser. The header of `run.ts` tells how the test starts the browser.

The test needs only Bun. For the type check of `run.ts`, run `bun install` in `web/test` one time, then run `bun run check` in that folder.

## Deployment

`Dockerfile`, `.dockerignore`, and `httpd.conf` make the image of the container that serves `dist`. The context of the image is this folder. `deploy-unraid` builds the game and the image, and replaces the container on the server. `README.md` in the repository root has the steps.

## The changes to LÖVE

`megasource.patch` and `love.patch` change the source of LÖVE before the build:

- Each library is a static library. With CMake 4.3.3 and later, a shared library is a second module that the page must load.
- LÖVE has the Lua module `chrogue_core`, and a stack of 4 MB. The search of the enemy AI overflows the default stack of 64 KB.
- The event loop does not use the audio module when the game has none (`t.modules.audio = false`). Without this change, the game stops at its first frame.
- A canvas has 8 bits for each color on WebGL 1. Without this change, a canvas has 4 bits, and the colors show bands.
- The script of the port that prepares the save folder runs after the game starts, which is too late, and it stops at a name that the page does not have (`LoveState`). The page prepares the folder (`index.html`).

The files of the patches have the line ends of their targets. Do not let an editor change them. If you change a patch, `build.sh` gets the source again and builds all of LÖVE.

## Limits

- WebGL 1 has no canvas with multisampling. The client then draws to a canvas with 2 pixels for each unit of the window, and the window shows it smaller (`../client/shaders.lua`). On a large window with a slow graphics card, the game can have less than 60 frames in each second. `F1` sets the effects off. On a Radeon 890M at 2560 by 1440, the camp with its largest content has 54 frames in each second, and 84 with the effects off.
- WebGL 2 does not work: this version of LÖVE uses a texture function that WebGL 2 does not have.
- The game has no phone layout. The web build was not tested on a phone.
- The port of LÖVE is the work of one person, and its branch can change. The build uses set commits.
- `love.js` and `love.wasm` have the debug checks of the port (`-sASSERTIONS`). The two files have 6.8 MB together, and a server that compresses them sends about a third of that.
