# Chrogue

Chrogue is a roguelite chess game for the browser. `README.md` has the commands and the structure of the code.

## The interface has a set layout

Read `DESIGN.md` before you change the interface. Its first rule applies to each change:

- Chrogue is a game, not a web page. The game has two set layouts: one for a wide screen, and one for a phone.
- A layout does not change with its content or with the state of the game. No area moves, wraps, shows, hides, or changes its size. The board has the same size in each battle.
- Each area has a size for the largest content that the game can give it. An area with no content stays as an empty slot.
- Only an element above the layout (a relic card, a list, a tooltip, a dialog) can have a size that comes from its content.
- A layout becomes larger or smaller only as one unit, through the size of `1rem` in `style.css`. Do not add breakpoints, wraps, or script that measures elements.

If a change needs more space than its area has, change the design of the area. Do not let the layout move.

## Other rules

- The game does not show a rules screen or a list of relics. The player finds these in a run.
- Use `jj`, not `git`. Make one change for each feature, with a Conventional Commit description.
- Use Bun and Vite. Run `bun run check` and `bun test` before you finish.
- Look at the result in a browser at 1440×900 and at 390×844. `gallery.html` has each screen with prepared data.
