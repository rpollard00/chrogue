# The Chrogue command protocol

A client plays the full game through this protocol. The client does not have game rules. It draws the `view`, plays the `events` as animation and sound, and sends commands.

The protocol has one entry point: `Session::command(request) -> response` in the crate `chrogue-game`. The binary `chrogue-core` sends each line of its input to that function and writes each response as one line. A test (`core/game/tests/protocol_doc.rs`) fails if a command, an event, an error code, or a field name of a response is not in this file.

Protocol version: 1. `hello` gives the version.

## Transport

- `chrogue-core --stdio`: one request on each line of stdin, one response on each line of stdout. The process stops at the end of the input or after `quit`.
- `chrogue-core --listen 127.0.0.1:PORT`: the first line on stdout is `{"listening":"127.0.0.1:N"}`. Then the process serves one TCP client at a time with the same lines. PORT 0 selects a free port. The process accepts only a loopback address.
  - The session continues when a client disconnects. A client that connects again gets the same screen and the same battle.
  - When a client disconnects, the process waits for a new client for `--idle-ms` milliseconds (5000 by default), and then stops. Before the first client, the limit is 6 times longer. `--keep-alive` removes the limit. `quit` always stops the process.
  - A second client that connects while one client is connected waits until the first client disconnects.
- `--save-dir PATH` keeps the saved data in `PATH/meta.json` and `PATH/run.json`. `--no-save` keeps it in memory. One of the two is necessary.
- `--seed N` sets the random numbers. All randomness comes from the seed: enemy armies, rewards, shop items, and the noise of the AI. The same seed and the same requests give the same responses, byte for byte.
- `--debug` permits the debug commands.
- A line that is longer than 65536 bytes gets the error `too_long`. An empty line gets no response.

## Requests and responses

A request is one JSON object: `{"id": 7, "cmd": "move", "from": 12, "to": 28}`.

- `id` is optional and can be any JSON value. The response has the same `id`, or `null`.
- `cmd` is the name of the command. The other fields are the arguments of the command. The core ignores fields that it does not know.

A response is one of these two objects:

```json
{"ok":true,"id":7,"events":[...],"view":{...}}
{"ok":false,"id":7,"error":{"code":"illegal_move","message":"..."},"view":{...}}
```

- `ok`: true if the command had an effect.
- `events`: the things that the command did, in order. See [Events](#events).
- `view`: the full data of the current screen, after the command. See [Views](#views). It is a snapshot. A client can draw the screen from it with no other data.
- `data`: only for `hello`, and for `view` with `"run": true` in a debug session.
- `error`: the `code` is for the client, and the `message` is for a person.

The core never stops on a bad request. A refused command changes nothing: the view in the error response is the view from before the request.

Squares are numbers from 0 (a1) to 63 (h8). The file is `square % 8` and the rank is `square / 8`. White is the player and moves toward rank 8. A piece kind is one letter: `k`, `q`, `r`, `b`, `n`, `p`. A color is `w` or `b`.

## Screens and commands

The screens are `title`, `upgrades`, `battle`, `camp`, and `over`. A command on a screen that does not have it gets `wrong_screen`.

```
title ──new_run──> battle ──continue──> camp ──start_battle──> battle ...
  │  └─continue_run─> battle or camp       └──> over (won or lost)
  └─open_upgrades─> upgrades ──back──> title
over ──new_run──> battle;  over ──open_upgrades──> upgrades;  over ──to_title──> title
```

### Any screen

| Command | Arguments | Effect |
| --- | --- | --- |
| `hello` | | `data` has the version, the names of the protocol, and the content tables. See [hello](#hello). |
| `view` | `run` (debug only) | No change. With `"run": true` in a debug session, `data.run` is the run in the format of the saved data. |
| `quit` | | Event `quit`. The server stops after the response. |

### Title

| Command | Arguments | Effect |
| --- | --- | --- |
| `new_run` | | Starts a new run and its first battle. A saved run is replaced, with no crowns for it, as in the browser game. |
| `continue_run` | | Continues the saved run: the camp, or the battle of the run from its start. Error `no_run` if no run is saved. |
| `open_upgrades` | | Goes to the upgrades screen. |

### Upgrades

| Command | Arguments | Effect |
| --- | --- | --- |
| `buy_upgrade` | `upgrade`: an upgrade id | Buys the next level. Errors `max_level`, `not_affordable`. |
| `back` | | Goes to the title. |

### Battle

The `phase` of the battle view tells who acts: `player`, `enemy`, or `over`.

| Command | Arguments | Phase | Effect |
| --- | --- | --- | --- |
| `move` | `from`, `to`, `promo` (optional: `q`, `r`, `b`, `n`) | `player` | Plays the move of the player. A promotion needs `promo` (error `promo_required`). After the move the phase is `enemy`, or `over`. |
| `enemy_move` | | `enemy` | The AI of the floor selects and plays the enemy move. The command blocks until the move is done (up to about 100 ms on floor 8). |
| `give_up` | | `player`, `enemy` | Ends the run as a loss. Goes to `over`. |
| `continue` | | `over` | Settles the battle: the gold, the lost units, and the promoted pieces go to the run. Goes to `camp`, or to `over` if the run is won or lost. |
| `to_title` | | any | Goes to the title. The battle is not saved. `continue_run` starts it again from its start, as a reload of the browser game does. |

The client owns the pause before the enemy move. The browser game waits 350 ms after the move of the player, then sends the equivalent of `enemy_move`.

### Camp

| Command | Arguments | Effect |
| --- | --- | --- |
| `take_reward` | `index`: a card of `reward.offers` | Takes the reward. Errors `reward_closed`, `bad_index`, `blocked`. |
| `skip_reward` | | Takes no reward. Error `reward_closed`. |
| `buy` | `index`: an item of `shop.offers` | Buys the item. Errors `bad_index`, `blocked`, `not_affordable`. |
| `reroll` | | Pays `shop.reroll_cost` gold for new shop items. Error `not_affordable`. |
| `place` | `unit`: a unit id, `square`: 0 to 15 | Moves the unit to the home square. If a unit is there, the two units swap. |
| `start_battle` | | Starts the battle of the next floor. Error `reward_pending` while the reward is open. |
| `to_title` | | Goes to the title. The run is saved. |

Each camp command except `place` and `to_title` gives the event `camp_action`. `place` gives `unit_placed`.

### Over

| Command | Effect |
| --- | --- |
| `new_run` | Starts a new run. |
| `open_upgrades` | Goes to the upgrades screen. |
| `to_title` | Goes to the title. |

### Debug commands

These commands work only in a session with `--debug`. Else the error is `debug_disabled`. They are the debug menu of the browser game (`src/ui/debug.ts`), and some more commands for tests. A change of the meta or of the run is saved immediately. If the screen is a battle, the battle starts again, as the debug menu does when it closes. Each change gives the event `debug_changed`.

| Command | Arguments | Effect |
| --- | --- | --- |
| `debug_set_crowns` | `crowns` | Sets the crowns. |
| `debug_set_upgrade` | `upgrade`, `level` | Sets the level of an upgrade at no cost. The level stays from 0 to the maximum level. |
| `debug_set_floor` | `floor`: 1 to 8 | Moves the run to the floor and makes a new enemy for it. |
| `debug_set_gold` | `gold` | Sets the gold of the run. |
| `debug_add_unit` | `kind`: `p`, `n`, `b`, `r`, `q` | Adds a unit on a free home square. Error `blocked` if the army is full. |
| `debug_remove_unit` | `unit`: a unit id | Removes a unit. Error `blocked` for the king. |
| `debug_set_army` | `units`: a list of `{"id"?, "kind", "home"}` | Replaces the army. One king, distinct ids and homes. A unit with no id gets its position in the list plus 1. |
| `debug_set_relic` | `relic`, `on` | Adds or removes a relic of the player. |
| `debug_set_trait` | `relic`, `on` | Adds or removes a trait of the enemy. The relic must have a text for the enemy. The enemy has at most `traits_max` traits (error `blocked`). |
| `debug_set_enemy` | `pieces`: a list of `{"kind", "square"}`, `traits` (optional) | Replaces the enemy army. One king, distinct squares. |
| `debug_bar_relic` | `relic`, `barred` | A barred relic is not a reward, not a shop item, and not a boss trait. The session keeps this set; it is not saved. |
| `debug_set_shop` | `offers`: a list of offers | Replaces the shop items. An offer is `{"kind":"piece","type":"n"}`, `{"kind":"relic","id":"bounty"}`, or `{"kind":"gold","amount":12}`. |
| `debug_set_draft` | `offers`: a list of offers, or `null` | Replaces the reward cards. In the camp, the reward shelf shows them. |
| `debug_enemy_move` | `from`, `to`, `promo` | Plays this enemy move in the phase `enemy`, instead of the AI. |
| `debug_ai_move` | `level` (optional, 1 to 8) | The AI plays for the side that has the move, at the level of a floor. The default is the floor of the run. A bot uses it to play White. |

## Views

Each view has `screen`. The other fields depend on the screen.

### `title`

- `meta`: `crowns`, `best` (the most floors that a run cleared), `runs`.
- `floors`: the number of floors (8).
- `run`: the saved run as `{"floor", "phase"}`, or `null`. `phase` is `battle` or `camp`.
- `can_continue`: true if a run is saved.

### `upgrades`

- `meta`: as on the title.
- `slots`: 16 items. An item is `null` (an empty slot) or an upgrade: `id`, `name`, `text`, `level`, `max_level`, `costs` (the crowns of each level), `next_cost` (`null` at the maximum level), `affordable`.

### `battle`

- `floor`: `number`, `name` (the name of the enemy), `boss`, `total`.
- `phase`: `player`, `enemy`, or `over`. `turn`: the color that has the move.
- `pieces`: each piece on the board: `id`, `kind`, `color`, `square`. The id of a piece stays the same when it moves and when it promotes, thus a client can tween it. Unit ids are from 1 to 19999 and stay the same for the full run. The pawn of Conscription has the id 20000. The enemy piece with index `i` in the army of the floor has the id `30000 + i`.
- `check`: the square of the king of the side to move if it is in check, else `null`.
- `last`: the last move as `{"from", "to"}`, or `null`.
- `moves`: in the phase `player`, each legal move of the player: `id` (the piece), `from`, `to`, `promo` (only on a promotion; a promotion has one move for each kind), `capture`, and `special`. A client does the selection, the target marks, and the promotion picker with this list. In the other phases the list is empty.
- `special`: `none`, `double_step`, `en_passant`, `backward` (a pawn step back with Tactical Retreat), or `castle`.
- `scout`: true if the player has the Scout upgrade. `enemy_moves`: with Scout in the phase `player`, the moves of each enemy piece as if the enemy had the move, with the field `preview` set to true. A client shows them as marks only.
- `taken`: the kinds that each side captured: `{"w": [...], "b": [...]}`. `w` has the enemy pieces that the player captured.
- `gold`: the gold of the run. `capture_gold`: the gold from captures in this battle, rounded as the game rounds it. `capture_gold_exact`: the same before rounding (Bounty gives halves).
- `lost`: the ids of the units that the enemy captured. `rescued`: the ids of captured units that return after the battle (Second Wind).
- `relics`: the relics of the player: `id`, `name`, `text`. `traits`: the traits of the enemy: `id`, `name`, `text` (the text for the enemy).
- `clock`: the half moves with no capture and no pawn advance. At 100 the battle is a draw.
- `result`: `null` until the battle ends. Then:
  - `winner`: `w`, `b`, or `null` for a draw.
  - `reason`: see the codes below.
  - `outcome`: `victory`, `defeat`, or `draw`.
  - `next`: what `continue` does: `camp`, `won` (the run is won), or `lost` (the run ends).
  - `reward`: `captures`, `clear`, `bonuses` (each with `id`, `label`, `gold`), and `total`.
  - `rows`: the rows of the result panel of the browser game, in order. A row has `row` (`captures`, `clear`, or `bonus`) and `gold`. A `bonus` row also has `id` and `label` (the name of the relic). A defeat has no rows.
  - `total`: the sum of the gold. `rescued`: the number of units that return (0 on a defeat).

### `camp`

- `floor`: the next floor, as in the battle view.
- `enemy`: the next enemy: `name`, `traits` (as in the battle view), `pieces` (`kind`, `square`), and `kinds` (the kinds in the order of the camp screen: the king first, then by value).
- `reward`: `null` if this visit has no reward (after a draw, or after a reload when the reward was taken). Else `offers`, `taken` (the index of the card that the player took, or `null`), `open` (true while the player can take or skip), and `state`: `open`, `taken`, or `skipped`.
- `shop`: `offers`, `reroll_cost`, and `can_reroll`. A shop offer also has `price` and `affordable`.
- An offer has `kind` (`piece`, `relic`, or `gold`), `name`, `text` (`null` when the name tells all), and `blocked` (`army_full`, `owned`, or `null`). A piece offer has `piece` (the kind). A relic offer has `id`. A gold offer has `amount`.
- `army`: the units: `id`, `kind`, `home` (a square from 0 to 15), in the order of the squares. `army_max`: 16.
- `relics`, `gold`, and `can_start` (false while the reward is open).

### `over`

- `meta`: as on the title, after the run.
- `summary`: `won`, `cleared` (floors), `floors` (8), `bonus` (the crowns for a won run), `crowns` (all the crowns of the run), and `new_best`.
- `rows`: the rows of the tally: `{"row": "floors", "crowns"}`, and on a won run `{"row": "win", "crowns"}`.

## Events

Each event is an object with `type`. The other fields depend on the type.

| Event | Fields | When |
| --- | --- | --- |
| `screen` | `name` | The screen changed or started again. It is the first event of the new screen. |
| `run_start` | | A new run started. |
| `battle_start` | `floor`, `name`, `boss` | A battle started. The browser game shows the floor banner here. |
| `move` | `id`, `color`, `kind` (before a promotion), `from`, `to` | A piece moved. It is the first event of a move. |
| `castle` | `id` (the rook), `from`, `to` | The rook move of a castle. |
| `en_passant` | `square` (of the captured pawn) | The move is an en passant capture. |
| `capture` | `id`, `color`, `kind`, `square`, `gold` | A piece was captured. `gold` is the gold of a capture by the player (it can have a half), else 0. |
| `unit_lost` | `id` | The enemy captured a unit. It leaves the army after the battle. |
| `unit_rescued` | `id` | The enemy captured a unit, and a relic returns it after the battle. |
| `promote` | `id`, `square`, `kind` | A pawn promoted. |
| `relic` | `ids` | These relics of the player had an effect. The browser game flashes them. |
| `check` | `square`, `color` | The king of the side to move is in check. |
| `result` | `winner`, `reason` | The battle ended. The view has the full result. |
| `camp_enter` | `floor`, `reward` (true if the visit has a reward) | The camp opened. The browser game deals the cards here. |
| `camp_action` | `action` (the command), `gold_before`, `gold`, `units` (`id`, `kind`, `home` of each new unit), `relics` (new relic ids), `rolled` | A camp command succeeded. These are the fields of `CampCue` in `src/ui/app.ts`. `rolled` is true when the shop has new items. |
| `unit_placed` | `id`, `from`, `to`, `swapped` (the id of the unit that swapped, or `null`) | A unit moved to a new home square. |
| `upgrade_bought` | `id`, `level`, `crowns_before`, `crowns` | An upgrade got a level. |
| `run_end` | `won`, `cleared`, `bonus`, `crowns`, `new_best`, and `run` in a debug session | The run ended. `run` is the last state of the run in the format of the saved data. |
| `debug_changed` | `what` | A debug command changed the data. |
| `save_failed` | `what` (`meta` or `run`), `message` | The storage could not save. The game continues. |
| `quit` | | The session stops. |

## Codes for the client text

The client has the interface text (key labels, status lines, result sentences). It selects the text with these codes:

- `phase`: `player`, `enemy`, `over`.
- `reason`: `checkmate`, `stalemate` (the side that has no legal move loses), `rout` (the loser has only its king), `bare` (only the kings remain, a draw), `clock` (50 moves with no capture and no pawn advance, a draw).
- `outcome`: `victory`, `defeat`, `draw`. `next`: `camp`, `won`, `lost`.
- `blocked`: `army_full` (the browser text is "Army full"), `owned` ("Owned").
- reward `state`: `open`, `taken`, `skipped`. Result `row`: `captures`, `clear`, `bonus`. Over `row`: `floors`, `win`.

The content text (the names and texts of relics, upgrades, floors, pieces, and offers) comes from the core in the views and in `hello`.

## Error codes

| Code | Meaning |
| --- | --- |
| `bad_json` | The line is not JSON. |
| `bad_request` | The request is not an object, or it has no `cmd` string. |
| `too_long` | The line is longer than 65536 bytes. |
| `unknown_command` | No command has this name. |
| `debug_disabled` | A debug command in a session with no `--debug`. |
| `bad_args` | An argument is missing, has the wrong type, or is out of range (for example a square that is not 0 to 63). |
| `wrong_screen` | The current screen does not have the command. |
| `wrong_phase` | The battle phase does not permit the command. |
| `illegal_move` | The move is not legal. |
| `promo_required` | The move is a promotion and has no `promo`. |
| `no_run` | The command needs a run, and no run is in progress or saved. |
| `reward_pending` | The reward is open. Take it or skip it first. |
| `reward_closed` | The camp has no open reward. |
| `blocked` | The run cannot take the offer (see `blocked`), or a debug limit. |
| `not_affordable` | Not sufficient gold or crowns. |
| `max_level` | The upgrade has its maximum level. |
| `bad_index` | No card or item has this index. |
| `internal` | A fault in the core. The state did not change. Report it. |

## hello

`data` of `hello` has:

- `protocol`: the version. `debug`: true if the debug commands work.
- `commands`, `events`, `errors`: the names in this file.
- `content`:
  - `relics`: `id`, `name`, `text`, `foe_text` (`null` if a boss cannot have it), `trait`, `rule_flags`, and `hooks`. `rule_flags` are the names of the TypeScript movement flags that give the same engine rules as the relic (`null` for a rule that only the Rust engine has). `hooks` are the names of the TypeScript hooks that the relic uses.
  - `upgrades`: `id`, `name`, `text`, `costs`, `hooks`.
  - `floors`: `number`, `name`, `budget` (the value of the enemy army), `traits` (the number of boss traits), `boss`, `level` (the name of the AI level), and `draft_gold` (the gold card of the reward before this floor).
  - `pieces`: `kind`, `name`, `value` (the gold of a capture), and `price` (in the shop, before upgrades).
  - `recruits`: `kind`, `weight`, `min_floor` of the pieces in rewards and in the shop.
  - `relic_price`, `reroll_cost`, `win_crowns`, `army_max`, `upgrade_slots`, `upgrade_name_max`, `traits_max`, `relic_count`.

The format of the saved data (`data.run` of `view`, and `run` of `run_end`) is the format of the browser game: `floor`, `gold`, `army` (`id`, `type`, `home`), `nextId`, `relics`, `enemy` (`pieces` with `type` and `square`, and `traits`), `phase`, `draft`, and `shop`. An offer there has `kind` and `type`, `id`, or `amount`.

## Example session

The responses are short here: `…` replaces some fields and list items.

```
> {"id":1,"cmd":"view"}
< {"ok":true,"id":1,"events":[],"view":{"can_continue":false,"floors":8,"meta":{"best":0,"crowns":0,"runs":0},"run":null,"screen":"title"}}

> {"id":2,"cmd":"new_run"}
< {"ok":true,"id":2,"events":[{"type":"run_start"},{"name":"battle","type":"screen"},{"boss":false,"floor":1,"name":"Border Patrol","type":"battle_start"}],
   "view":{"screen":"battle","phase":"player","turn":"w","floor":{"boss":false,"name":"Border Patrol","number":1,"total":8},
           "pieces":[{"color":"w","id":2,"kind":"r","square":0},{"color":"w","id":1,"kind":"k","square":4},…,{"color":"b","id":30000,"kind":"k","square":60}],
           "moves":[{"capture":false,"from":10,"id":4,"special":"none","to":18},{"capture":false,"from":10,"id":4,"special":"double_step","to":26},…],
           "check":null,"last":null,"clock":0,"gold":0,"capture_gold":0,"capture_gold_exact":0,"lost":[],"rescued":[],"taken":{"b":[],"w":[]},
           "relics":[],"traits":[],"scout":false,"enemy_moves":[],"result":null}}

> {"id":3,"cmd":"move","from":11,"to":27}
< {"ok":true,"id":3,"events":[{"color":"w","from":11,"id":5,"kind":"p","to":27,"type":"move"}],"view":{…,"phase":"enemy","turn":"b","moves":[],"last":{"from":11,"to":27},…}}

  (The client waits 350 ms, then asks for the enemy move.)
> {"id":4,"cmd":"enemy_move"}
< {"ok":true,"id":4,"events":[{"color":"b","from":51,"id":30002,"kind":"p","to":43,"type":"move"}],"view":{…,"phase":"player","last":{"from":51,"to":43},…}}

> {"id":5,"cmd":"move","from":11,"to":27}
< {"ok":false,"id":5,"error":{"code":"illegal_move","message":"No legal move from 11 to 27"},"view":{…the same view as in response 4…}}

> {"id":6,"cmd":"give_up"}
< {"ok":true,"id":6,"events":[{"name":"over","type":"screen"},{"bonus":0,"cleared":0,"crowns":0,"new_best":false,"type":"run_end","won":false}],
   "view":{"meta":{"best":0,"crowns":0,"runs":1},"rows":[{"crowns":0,"row":"floors"}],"screen":"over","summary":{"bonus":0,"cleared":0,"crowns":0,"floors":8,"new_best":false,"won":false}}}

> {"id":7,"cmd":"to_title"}
< {"ok":true,"id":7,"events":[{"name":"title","type":"screen"}],"view":{"can_continue":false,"floors":8,"meta":{"best":0,"crowns":0,"runs":1},"run":null,"screen":"title"}}
```

A won battle and a camp action (from the test `the_camp_example_is_real`: the army is a king and a rook, the run has Bounty, Interest, and 20 gold, and the reward cards are 14 gold and a knight):

```
< …"events":[{"type":"move",…},{"type":"capture","color":"b","id":30001,"kind":"r","square":8,"gold":7.5},{"type":"relic","ids":["bounty","interest"]},{"type":"result","winner":"w","reason":"rout"}],
   "view":{…,"phase":"over","result":{"winner":"w","reason":"rout","outcome":"victory","next":"camp",
     "reward":{"captures":8,"clear":4,"bonuses":[{"id":"interest","label":"Interest","gold":6}],"total":18},
     "rows":[{"row":"captures","gold":8},{"row":"clear","gold":4},{"row":"bonus","id":"interest","label":"Interest","gold":6}],"total":18,"rescued":0}}
> {"cmd":"continue"}
< …"events":[{"type":"screen","name":"camp"},{"type":"camp_enter","floor":2,"reward":true}],"view":{"screen":"camp",…}
> {"cmd":"take_reward","index":1}
< …"events":[{"type":"camp_action","action":"take_reward","gold_before":38,"gold":38,"units":[{"id":3,"kind":"n","home":3}],"relics":[],"rolled":false}],…
```

## Open issues

- With Tactical Retreat (`backpedal`), a pawn can step back and forward again and again. The forward step is a pawn advance, thus it resets the 50-move clock, and the game has no rule for a repeated position. A battle can then continue with no end. The core keeps this rule of the TypeScript game and does not add a repetition rule.
- A draw on the last floor: the TypeScript game goes to floor 9 and fails there (`generateEnemy(9)` throws). The core stays on floor 8: `continue` opens the camp before floor 8 again, with no reward.
- A battle can start in a position where the player has no legal move (for example, a debug army that is in checkmate at the start). As in the TypeScript game, the result is checked only after a move: the phase is `player` with no moves, and only `give_up` and `to_title` work.
