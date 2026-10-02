# Design

The visual and interaction intent of Chrogue. Raw values are in `style.css`. This file records what they mean and how to select among them.

Status words: **provisional** means the user approved the direction and the rendered game is under validation. **Established** means the user accepted the rendered result.

## The first rule: a set layout

Chrogue is a game, not a web page. Its interface has a set layout, as the interface of Balatro or of Diablo 2 has. This rule is before each other rule in this file.

- The game has two layouts: one for a wide screen, and one for a phone or a screen in portrait. There is no third layout and no layout between them.
- A layout does not change with its content or with the state of the game. Each area has a set position and a set size. The number of relics, the number of captured pieces, the length of a name, a reward, and the turn do not move or resize an area. The board has the same size in each battle.
- Each area has a size for the largest content that the game can give it. Design the area for that content first.
- When the content of an area changes, the content changes in its place. An area with no content stays as an empty slot. Do not hide the area, and do not let other areas take its space.
- Only an element above the layout can have a size that comes from its content: the card of a relic, the list of a stash, a tooltip, a dialog, a banner. Such an element does not move the layout below it.
- A layout becomes larger or smaller only as one unit. Each size is in `rem`, and the size of `1rem` comes from the size of the screen (see `html` in `style.css`). Do not add a breakpoint, a wrap, or a size from a container to make content fit.
- Do not measure elements with script to set a layout.

Before you add or change an interface element, answer two questions. Where is its set area in each of the two layouts? What is the largest content of that area?

Decision source: the user gave this rule on 1 October 2026. Status: established.

## Character

Chrogue is a game UI, not a web form. It has the layout and density of a chess platform, and each thing that holds state or takes a click has physical presence.

- Streamlined, not atmospheric. No table, felt, or wood scenery.
- Physical, not flat. Objects have one light source from above.
- The board is the largest thing on the battle screen. The cards are the most prominent objects in camp.

Decision source: the user selected mockup C, "Platform with objects", on 1 October 2026, and accepted the rendered battle and camp screens. Status: established.

## Objects

| Object | Meaning | Status |
|---|---|---|
| Card | One thing that the player can take, buy, or upgrade | Established |
| Medal | The picture of a card or of a relic | Established |
| Fan | The relics of the player, or the traits of the enemy: medals that overlap with a fixed step. The medal under the pointer shows the card of the relic. | Established |
| Info button and paper tip | The text of a reward, shop, or upgrade card | Established |
| Key | Each button. A key goes down when the player presses it. The primary key is amber. | Established |
| Plaque | A raised bar that holds the state of one side: the enemy, or the player | Established |
| Well | A recessed slot that holds a count or a group of pieces | Established |
| Shelf | A recessed area that holds a row of cards | Established |
| Lamp | The turn status. It is lit when the player can move. | Established |
| Menu | A raised bar of keys on a screen between runs. The primary key has the full width. | Established |

Rules:

- A screen has at most two plaques and two shelves. If each group becomes a plaque, the cards do not stand out.
- A piece that is not on the board is on a board-brown lining. A black piece is not visible on a dark surface.
- Each card has the same size, as a physical card has. The card of a relic in a fan has the size of a reward card, with its text in the place of the key. The card is above the medal of the player and below the medal of the enemy.
- On a touch screen, a tap on a medal shows its card, and a tap on a different place hides it.
- An empty area has no "None" text. It shows as an empty slot.
- In a battle, each plaque is a grid with set rows. A stash shows the last piece that a side captured and the number of the pieces. Its list of all the pieces shows above the layout.
- The display typeface is for names, headings, the wordmark, the primary key, the boss badge, and the purse, as in mockup C. Body text uses the system typeface.

## Composition

- Battle: the state frames the board. The enemy plaque is above the board, and the player plaque is below it. There is no side rail.
- Camp: one table. The next enemy and the start action are at the top, the reward and the shop are in the middle, and the army, the relics, and the purse are at the bottom. Camp has the same frame as the battle: the enemy above, the player below.
- On a wide screen, each screen shows all its content with no page scroll.
- The camp has the reward shelf and the shop shelf on each visit. After the player takes or skips the reward, the card that the player took keeps its color, and the other cards are dim. A visit with no reward has an empty reward shelf.

- Screens between runs (the title, the end of a run) are one column in the center: a heading, a menu, and wells for the numbers. The upgrades screen is a shelf of cards.
- The game has no rules screen. The player finds the rules and the relics in a run. The result of a battle gives its cause.

Status: established. The user accepted the rendered screens on 1 October 2026. Reference surfaces: the demos in `gallery.html`, and the title of the game.

## Responsive and accessibility rules

- A phone width is a first-class target. The game must be fully playable at 390 pixels wide.
- On a phone, the camp is the one screen that scrolls. The purse and the start action stay in view.
- The phone layout is a second set layout. It is not the wide layout at a smaller size.
- Each control is a `button` with a visible focus ring. Status text has `role="status"`. Motion stops when the player prefers reduced motion.

## Color

Each color is a custom property in `:root` of `style.css`. A component uses the property, not a raw value, so that a second theme can replace the properties. The game has one theme, dark. Themes are a possible later task.

## Sources

- Tokens and all styles: `style.css`
- Display typeface: `src/fonts/ChrogueDisplay.woff2`. It is a Latin subset of Fira Sans Condensed ExtraBold. The license reserves the name "Fira", thus the subset has a different name. The license is in `src/fonts/OFL.txt`.
- Shared elements: `src/ui/widgets.ts`, `src/ui/icons.ts`, `src/ui/tip.ts`
- Rendered reference surfaces: `gallery.html` (run `bun run dev`, open `/gallery.html`)

## Not yet covered

- The end of a run does not show a summary of the run. This is a follow-up task.
