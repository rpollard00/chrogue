# Chrogue

Chrogue is a roguelite chess game for the browser. Each battle is on a standard 8x8 chess board.

## Run the game

The project uses Bun and Vite.

1. Run `bun install`.
2. Run `bun run dev`.
3. Open `http://localhost:5188`.

`bun run build` writes the production files to `dist/`. `bun run preview` serves those files.

## See the animations

The animation gallery shows each screen with prepared data and plays its motion.

1. Run `bun run dev`.
2. Open `http://localhost:5188/gallery.html`.
3. Select a demo. Select **Replay** to see the demo again.

The gallery saves nothing, and the production build does not include it.

## Run the tests

Run `bun test`. Run `bun run check` for the TypeScript type check.

## Deploy to Unraid

Run `bun run deploy:unraid`. The command does these steps without a confirmation prompt:

1. It runs the type check and the tests, and builds the image on this workstation.
2. It sends the image to `root@media.media` through SSH. It does not use a registry.
3. It replaces the container `chrogue` on network `br0` at `192.168.88.13`, port 80.

The container name is always `chrogue`. If a container with that name exists and this script did not make it, the deployment stops.
To change the target, set `CHROGUE_SSH_TARGET`, `CHROGUE_NETWORK`, or `CHROGUE_ADDRESS`.

## Rules

- You play White. Your army stays with you from one battle to the next battle.
- A piece that the enemy captures is gone for the rest of the run.
- A pawn that you promote stays promoted.
- You win a battle when you checkmate the enemy king.
- You also win when the enemy king is alone, or when the enemy has no legal move.
- The same conditions apply to you. If you lose a battle, the run ends.
- If 50 moves pass with no capture and no pawn advance, the battle is a draw. You continue without a reward.
- Castling and en passant follow the usual chess rules.

A run has 8 floors. Floor 4 and floor 8 are boss floors, and a boss has traits that change its moves.
After each battle, you select one of three rewards, buy pieces and relics, and arrange your first two ranks.
Each run gives crowns. Crowns buy permanent upgrades on the title screen.

## Structure

The code has three layers. Each layer imports only from the layers before it in this list.

- `src/engine/`: Chess. It has the rules, the move generation, and the enemy AI. It does not know about runs or relics.
- `src/game/`: The roguelite. It has runs, battles, relics, upgrades, offers, floors, and saved data. It has no DOM code.
- `src/ui/`: The screens. `src/main.ts` owns the state and changes the screen.

A relic changes a battle in two ways. Its `rules` add movement rules for the engine. Its `hooks` run at fixed points of a battle.

## Add content

- Relic: Add one entry to `defs` in `src/game/relics.ts`. If the entry has a `foeText`, a boss can have it as a trait.
- Relic effect at a new point of a battle: Add a hook to `RelicHooks`, and call it in `src/game/battle.ts`.
- Movement rule: Add a flag to `MoveRules` in `src/engine/types.ts`, and use it in `src/engine/rules.ts`.
- Upgrade: Add one entry to `defs` in `src/game/upgrades.ts`.
- Kind of reward or shop item: Add its data to `Offer` in `src/game/types.ts`. Then obey the steps at the top of `src/game/offers.ts`.
- Floor: Add one entry to `FLOORS` in `src/game/floors.ts`.

Saved data keeps relic ids and upgrade ids. If you remove an id, `src/game/storage.ts` removes it from saved data when the game loads.

## Files

- `Dockerfile`, `httpd.conf`: The container image. Bun and Vite build the files. BusyBox `httpd` serves them.
- `scripts/deploy-unraid`: The deployment script.
- `test/engine.test.ts`: Perft counts, movement rules, battle results, and AI checks.
- `test/game.test.ts`: Relics, upgrades, offers, battle results in a run, and saved data.
