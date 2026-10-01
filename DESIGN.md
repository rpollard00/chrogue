# Design

The visual and interaction intent of Chrogue. Raw values are in `style.css`. This file records what they mean and how to select among them.

Status words: **provisional** means the user approved the direction and the rendered game is under validation. **Established** means the user accepted the rendered result.

## Character

Chrogue is a game UI, not a web form. It has the layout and density of a chess platform, and each thing that holds state or takes a click has physical presence.

- Streamlined, not atmospheric. No table, felt, or wood scenery.
- Physical, not flat. Objects have one light source from above.
- The board is the largest thing on the battle screen. The cards are the most prominent objects in camp.

Decision source: the user selected mockup C, "Platform with objects", on 1 October 2026. Status: provisional.

## Objects

| Object | Meaning | Status |
|---|---|---|
| Card | One thing that the player can take, buy, or upgrade | Established |
| Medal | The picture of a card or of a token | Established |
| Token | A relic or an enemy trait | Established |
| Info button and paper tip | The text of a card or of a token | Established |
| Key | Each button. A key goes down when the player presses it. The primary key is amber. | Provisional |
| Plaque | A raised bar that holds the state of one side: the enemy, or the player | Provisional |
| Well | A recessed slot that holds a count or a group of pieces | Provisional |
| Shelf | A recessed area that holds a row of cards | Provisional |
| Lamp | The turn status. It is lit when the player can move. | Provisional |

Rules:

- A screen has at most two plaques and two shelves. If each group becomes a plaque, the cards do not stand out.
- A piece that is not on the board is on a board-brown lining. A black piece is not visible on a dark surface.
- An empty group has no heading and no "None" text.
- The display typeface is for names, headings, and the wordmark only. Body text uses the system typeface.

## Composition

- Battle: the state frames the board. The enemy plaque is above the board, and the player plaque is below it. There is no side rail.
- Camp: one table. The next enemy and the start action are at the top, the reward and the shop are in the middle, and the army, the relics, and the purse are at the bottom. Camp has the same frame as the battle: the enemy above, the player below.
- At 1440×900, camp shows all its content with no page scroll.

Status: provisional. Reference surfaces: the battle screen and the camp screen in `gallery.html`.

## Responsive and accessibility rules

- A phone width is a first-class target. The game must be fully playable at 390 pixels wide.
- On a phone, camp can scroll. The purse and the start action stay in view.
- Layouts reflow. They do not scale a desktop layout down.
- Each control is a `button` with a visible focus ring. Status text has `role="status"`. Motion stops when the player prefers reduced motion.

## Color

Each color is a custom property in `:root` of `style.css`. A component uses the property, not a raw value, so that a second theme can replace the properties. The game has one theme, dark. Themes are a possible later task.

## Sources

- Tokens and all styles: `style.css`
- Shared elements: `src/ui/widgets.ts`, `src/ui/icons.ts`, `src/ui/tip.ts`
- Rendered reference surfaces: `gallery.html` (run `bun run dev`, open `/gallery.html`)

## Not yet covered

- The title, upgrades, help, and end-of-run screens have the old panel look. They take the new objects after the user accepts the battle and camp screens.
- The end of a run does not show a summary of the run. This is a follow-up task.
