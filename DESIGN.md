# Design

The visual and interaction intent of Chrogue. Raw values are in `style.css`. This file records what they mean and how to select among them.

Status words: **provisional** means the user approved the direction and the rendered game is under validation. **Established** means the user accepted the rendered result.

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
| Medal | The picture of a card or of a token | Established |
| Token | A relic or an enemy trait | Established |
| Info button and paper tip | The text of a card or of a token | Established |
| Key | Each button. A key goes down when the player presses it. The primary key is amber. | Established |
| Plaque | A raised bar that holds the state of one side: the enemy, or the player | Established |
| Well | A recessed slot that holds a count or a group of pieces | Established |
| Shelf | A recessed area that holds a row of cards | Established |
| Lamp | The turn status. It is lit when the player can move. | Established |
| Menu | A raised bar of keys on a screen between runs. The primary key has the full width. | Provisional |

Rules:

- A screen has at most two plaques and two shelves. If each group becomes a plaque, the cards do not stand out.
- A piece that is not on the board is on a board-brown lining. A black piece is not visible on a dark surface.
- An empty group has no heading and no "None" text.
- The display typeface is for names, headings, the wordmark, the primary key, the boss badge, and the purse, as in mockup C. Body text uses the system typeface.

## Composition

- Battle: the state frames the board. The enemy plaque is above the board, and the player plaque is below it. There is no side rail.
- Camp: one table. The next enemy and the start action are at the top, the reward and the shop are in the middle, and the army, the relics, and the purse are at the bottom. Camp has the same frame as the battle: the enemy above, the player below.
- At 1440×900, camp shows all its content with no page scroll.
- The camp layout is stable during a visit. After the player takes or skips the reward, the reward shelf stays: the card that the player took keeps its color, and the other cards are dim. A visit that starts with no reward has no reward shelf.

- Screens between runs (the title, the end of a run) are one column in the center: a heading, a menu, and wells for the numbers. The upgrades screen is a shelf of cards.
- The game has no rules screen. The player finds the rules and the relics in a run. The result of a battle gives its cause.

Status: the battle and the camp are established. The title, the upgrades, and the end of a run are provisional. Reference surfaces: the demos in `gallery.html`, and the title of the game.

## Responsive and accessibility rules

- A phone width is a first-class target. The game must be fully playable at 390 pixels wide.
- On a phone, camp can scroll. The purse and the start action stay in view.
- Layouts reflow. They do not scale a desktop layout down.
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
