# Design

The visual and interaction intent of Chrogue. Raw values are in the client: `client/theme.lua` has the colors and the typefaces, and `client/layout.lua` has the areas. This file records what they mean and how to select among them.

Status words: **provisional** means the user approved the direction and the rendered game is under validation. **Established** means the user accepted the rendered result.

## The first rule: a set layout

Chrogue is a game, not a web page. Its interface has a set layout, as the interface of Balatro or of Diablo 2 has. This rule is before each other rule in this file.

- The game has two layouts: one for a wide screen, and one for a phone or a screen in portrait. There is no third layout and no layout between them.
- A layout does not change with its content or with the state of the game. Each area has a set position and a set size. The number of relics, the number of captured pieces, the length of a name, a reward, and the turn do not move or resize an area. The board has the same size in each battle.
- Each area has a size for the largest content that the game can give it. Design the area for that content first.
- When the content of an area changes, the content changes in its place. An area with no content stays as an empty slot. Do not hide the area, and do not let other areas take its space.
- Only an element above the layout can have a size that comes from its content: the card of a relic, the list of a stash, a tooltip, a dialog, a banner. Such an element does not move the layout below it.
- A layout becomes larger or smaller only as one unit. Each size is in stage units, and the size of one unit comes from the size of the window (`gfx.u` in the client). Do not add a breakpoint, a wrap, or a size from a container to make content fit.
- Do not measure elements with code to set a layout.

The client has only the wide layout at this time. See "Responsive and accessibility rules".

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
| Card | One thing that the player can take or buy in a run | Established |
| Medal | The picture of a card or of a relic | Established |
| Fan | The relics of the player, or the traits of the enemy: medals that overlap with a fixed step. The medal under the pointer shows the card of the relic. The fan of the player also shows the relic slots of the run. | Established. The slots and the selection: provisional |
| Info button and paper tip | The text of a reward or shop card | Established |
| Key | Each button. A key goes down when the player presses it. The primary key is amber. | Established |
| Plaque | A raised bar that holds the state of one side: the enemy, or the player | Established |
| Well | A recessed slot that holds a count or a group of pieces | Established |
| Shelf | A recessed area that holds a row of cards | Established |
| Lamp | The turn status. It is lit when the player can move. | Established |
| Menu | A raised bar of keys on a screen between runs. The primary key has the full width. | Established |
| Medal board | A shelf of 16 set slots, 4 by 4. Each slot is a well that holds the medal of one upgrade, its name, its levels, and the cost of its next level. | Established |
| Panel | A raised surface that shows the one upgrade that the player selects on the medal board: its text, its levels, and the key that buys it. | Established |
| Badge | A small dark plate on a piece that has a boon now: a thing that the piece gets from a near piece of its side. | Provisional |
| Zone | The squares of one aura, with a tint and a line at its edge. A zone shows only while its aura is in focus. | Provisional |

Rules:

- A screen has at most two plaques and two shelves. If each group becomes a plaque, the cards do not stand out.
- A piece that is not on the board is on a board-brown lining. A black piece is not visible on a dark surface.
- Each card has the same size, as a physical card has. The card of a relic in a fan has the size of a reward card, with its text in the place of the key. The card is above the medal of the player and below the medal of the enemy.
- On a touch screen, a tap on a medal shows its card, and a tap on a different place hides it.
- The fan of the player shows the relic slots of the run: 4 at the start of a run, and at most 10. The medals fill the slots from the left. A slot with no relic is an empty recessed ring at its place in the fan, as space for a medal. The area of the fan does not change.
- In the camp, the kicker of the fan has the count of the relics and of the slots. The count is amber when each slot has a relic, and a relic card then has the stamp "Relics full".
- In the camp, a click on a medal of the player selects it. The selected medal has an amber ring, and its card stays in view. A second click, a click on a different place, or `Escape` clears the selection. A unit on a home square and a medal are not selected at the same time.
- The Discard key of the camp is a quiet key with a set place below the fan. It is always there, and it is disabled while no medal is selected. The game asks before it discards a relic. Then the medal leaves the fan, and the medals after it go to their new slots.
- An empty area has no "None" text. It shows as an empty slot.
- On the board, the marks of a piece of the player are green and dark. The marks of an enemy piece that the player selects with Scout are red.
- A badge is a round plate with a rim and a solid picture in the color of its side: green for the player, and red for the enemy. Its diameter is 0.24 of a square. The picture tells the boon: a shield for a piece that the enemy cannot capture, and a star for a piece that has more moves.
- A square has two set slots for badges, in a column on its left side: the shield at (0.20, 0.36) of the square, and the star at (0.20, 0.64). A piece with one boon keeps the slot of that boon. The slots are away from the coordinates of the board, and a badge can be above a small part of its piece.
- A badge does not move while nothing changes. A new badge becomes smaller to its size with one ring, after its piece is at rest. A badge that a piece loses becomes larger and goes away. At the start of a battle, each badge comes into view one more time after the banner, and the medals of their relics flash.
- An aura is in focus while the pointer is on the medal of its relic, on a piece that gives it, or on a piece with a badge from it. The board then shows the zone of the aura: a tint on each of its squares, below the mark of the square, and a line where the zone ends. A square in the zone is the square of a piece that gives the boon, a place where a piece has the boon, or a place where a piece that can have it gets it. The zones in focus of one side show as one shape, with one tint and one line. The zones of the two sides stay two shapes.
- The pointer on such a piece, and the selection of such a piece, also light the medals of its relics in the fan of its side. A lit medal has the ring of its side. It does not go up, and it shows no card.
- In a battle, each plaque is a grid with set rows. A stash shows the last piece that a side captured and the number of the pieces. Its list of all the pieces shows above the layout.
- An upgrade is not a card. A card is a thing of one run. An upgrade stays between runs, and it has a set slot on the medal board.
- On the medal board, an upgrade keeps its slot in each visit. A slot with no upgrade stays as an empty well. The slot of the upgrade that the player selects has an amber ring.
- The panel has a set size. It has space for a text of three lines on a phone. On a wide screen, the panel also shows the cost of each level.
- The display typeface is for names, headings, the wordmark, the primary key, the boss badge, and the purse, as in mockup C. Body text uses Fira Sans.

## Composition

- Battle: the state frames the board. The enemy plaque is above the board, and the player plaque is below it. There is no side rail.
- Camp: one table. The next enemy and the start action are at the top, the reward and the shop are in the middle, and the army, the relics, and the purse are at the bottom. Camp has the same frame as the battle: the enemy above, the player below.
- On a wide screen, each screen shows all its content with no page scroll.
- The camp has the reward shelf and the shop shelf on each visit. After the player takes or skips the reward, the card that the player took keeps its color, and the other cards are dim. A visit with no reward has an empty reward shelf.

- Screens between runs (the title, the end of a run) are one column in the center: a heading, a menu, and wells for the numbers. The upgrades screen is different: see below.
- Upgrades: the crowns are at the top, and the key that goes back is at the bottom. Between them are the medal board and the panel. On a wide screen, the panel is at the right of the board. On a phone, the panel is below the board. Nothing on this screen scrolls.
- The game has no rules screen. The player finds the rules and the relics in a run. The result of a battle gives its cause.

Status: established. The relic slots, the selection of a medal, and the Discard key are provisional: the user decided them on 3 October 2026. The badge and the zone are provisional: on 5 October 2026, the user selected the quiet badge, its column on the left side of the square, its diameter, and the zone in the place of lines to the source piece. The user accepted the rendered screens of the TypeScript game on 1 October 2026. For the upgrades screen, the user selected mockup C, "a board of medals and one panel", and accepted the rendered screen on 1 October 2026. The client draws the same screens. Reference surfaces: the screens of the client.

## The debug menu

The debug menu is a development tool (`client/README.md`, "The debug menu"). It is not a part of the interface of the game.

- The menu is above the layout, as a dialog is. It does not move or resize an area of a screen.
- The menu is the one place that lists the relics. The game shows no such list ("Composition").
- The panel of the menu has a set size, and each tab has a set layout for its largest content: 28 relics, 8 floors, 5 kinds of pieces, the 16 upgrades of the medal board, and 11 backgrounds. A row with no content stays empty.
- The menu uses the objects of the game: keys, wells, medals, and the paper tip. A toggle is a key that has an amber ring, an amber lamp, and an amber label when it is on.

Decision source: the user asked for the menu on 3 October 2026. Status: provisional.

## Responsive and accessibility rules

- A phone width is a first-class target. The game must be fully playable at 390 pixels wide.
- On a phone, only the camp scrolls. In the camp, the purse and the start action stay in view.
- The phone layout is a second set layout. It is not the wide layout at a smaller size.
- Each control has a visible focus ring. A screen reader gets the status text. Motion stops when the player prefers reduced motion.

The client does not obey these rules at this time. It has only the wide layout, and it does not have the properties of the last rule. The TypeScript game obeyed them, and the removal of that game removed its phone layout. A new plan for the phone layout is necessary (`DECISIONS.md`, "Open").

## Color

Each color has a name in `client/theme.lua`. A module uses the name, not a raw value, so that a second theme can replace the colors. The game has one theme, dark. Themes are a possible later task.

## Sources

- Colors and typefaces: `client/theme.lua`
- Areas of each screen: `client/layout.lua`
- Typeface files and their licenses: `client/fonts/`. The display typeface is Fira Sans Condensed ExtraBold.
- Shared elements: `client/ui.lua`, `client/gfx.lua`, `client/icons.lua`
- Rendered reference surfaces: the client with the script `test/showcase.lua`, which shows the largest content of the battle, the camp, and the end of a run (`client/README.md`, "Test scripts")

## Not yet covered

- The end of a run does not show a summary of the run. This is a follow-up task.
- The medal board holds 16 upgrades. The design for more upgrades (a second page, or a board that scrolls) is not decided. A test stops a 17th upgrade.
- A slot on a phone holds a name of 13 characters. A test stops a longer name.
- The Relics tab of the debug menu holds 28 relics. A 29th relic does not show.
- The phone layout, the focus ring, the text for a screen reader, and the stop of motion for a player who prefers reduced motion ("Responsive and accessibility rules").
