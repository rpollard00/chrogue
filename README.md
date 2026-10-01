# Chrogue

Chrogue is a roguelite chess game for the browser. Each battle is on a standard 8x8 chess board.

## Run the game

The game has no build step and no dependencies. It uses ES modules, thus a browser must load it from a web server.

1. Run `npm start`.
2. Open `http://localhost:5188`.

## Run the tests

Run `npm test`. The tests need Node.js 20 or later.

## Deploy to Unraid

Run `npm run deploy:unraid`. The command does these steps without a confirmation prompt:

1. It runs the tests and builds the image on this workstation.
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

## Files

- `src/engine.js`: Chess rules, move generation, and the enemy AI. It has no DOM code.
- `src/content.js`: Relics, floors, upgrades, prices, and the random generators.
- `src/main.js`: Run state, screens, and saved data in `localStorage`.
- `Dockerfile`, `httpd.conf`: The container image. BusyBox `httpd` serves the static files.
- `scripts/deploy-unraid`: The deployment script.
- `test/engine.test.js`: Perft counts, relic moves, battle results, and AI checks.
