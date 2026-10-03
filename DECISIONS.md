# Decisions

This file records the decisions of the user about the direction of Chrogue, and the reason for each one. `DESIGN.md` records the decisions about the interface.

The game is at an early stage. A decision here is the direction at this time. It can change.

Decision source: the user gave these decisions on 2 October 2026, unless an entry gives a different date.

## Direction

### The game is a Rust core and a LÖVE client

- The Rust core in `core/` has the chess engine, the enemy AI, the runs, and the saved data.
- The LÖVE client in `ports/love/` draws the screens. It has no game rules.

Reasons:

- A release on Steam is a goal for a later time.
- The game needs shaders for a look of its own. The browser game looked generic.
- The interface is apart from the game logic. Thus a dedicated phone interface is possible, and a test or an agent can run the game with no interface.
- The game needs a language that is good for a strong chess engine.

A full toolkit (Godot, Unity) is not necessary. The battle screen was built in PixiJS and in LÖVE for a comparison. The two looked the same after some work, and the user selected LÖVE.

### Chess stays the core of the game

A relic can change the rules of chess. The enemy AI must play good chess, or weak chess where the game wants a weak enemy, with the rules that the battle has.

Thus the movement rules are data that the engine and the AI read. A new movement rule does not need new engine code. The Rust engine is built for this. It is not a line-by-line copy of the TypeScript engine.

### The core and the client talk through commands

The core takes JSON commands and gives the state of the game (`core/PROTOCOL.md`).

- In development, the core is a separate program on a local socket. A test or an agent can send the same commands.
- A release build loads the core into LÖVE as a library. The socket stays for development.

### The TypeScript browser game is deprecated

Two codebases are too much to maintain. The Rust core is the source of truth for the rules of the game.

The TypeScript game was the reference for the first version of the core. It is not a specification: where the two are different, the core is correct. The scripts in `core/difftest/` and `core/gametest/` compare the core with the TypeScript game. They go away when the TypeScript game goes away.

### The game also goes to the web through WebAssembly

The LÖVE game will have a WebAssembly build, thus the game can stay on the web. This build does not exist at this time.

## Rules of the game

### Only a capture resets the draw count

A battle is a draw after 50 moves with no capture. Each move that is not a capture adds to the count, standard or not. A pawn move does not reset the count.

Reason: with Tactical Retreat, a pawn can step back and forward again and again. When a pawn advance reset the count, such a battle did not end.

### The result of a battle is final

The game saves the result of a battle with the move that ends the battle. If the player starts the game again on the result screen, the game continues after the battle. The player cannot play the battle a second time.

## Deferred

The user decided to wait on these items.

- A battle can start with the enemy king in check (for example, a rook of the player on an open e-file). The decision waits for more playtests. The user has ideas for the start positions.
- The enemy slot of the camp is long and looks empty for a small army. It needs a better design at a later time.

## Open

No decision at this time.

- The phone layout of the client. The client has only the wide layout.
- A battle in progress is not saved. If the player starts the game again in a battle, the battle starts again from its first move.
- The core describes the pawn moves and the castles as data, as it does for the other pieces. The user did not confirm this part.
- The time to remove the TypeScript game: before or after the WebAssembly build.
- `core/PROTOCOL.md` has a list of open issues in the behavior of the core.
