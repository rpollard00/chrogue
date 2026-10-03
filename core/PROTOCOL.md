# The Chrogue command protocol

A client plays the full game through this protocol. The client does not have game rules. It draws the `view`, plays the `events` as animation and sound, and sends commands.

The protocol has one entry point: `Session::command(request) -> response` in the crate `chrogue-game`. The binary `chrogue-core` sends each line of its input to that function and writes each response as one line. A test (`core/game/tests/protocol_doc.rs`) fails if a command, an event, an error code, or a field name of a response is not in this file.

Protocol version: 1. `hello` gives the version.

## Transport

- `chrogue-core --stdio`: one request on each line of stdin, one response on each line of stdout. The process stops at the end of the input or after `quit`. Stdio needs no token.
- `chrogue-core --listen 127.0.0.1:PORT`: the first line on stdout is `{"listening":"127.0.0.1:N"}`. Then the process serves one TCP client at a time with the same lines. PORT 0 selects a free port. The process accepts only a loopback address.
  - Each connection must authenticate first (see [Authentication](#authentication)).
  - The session continues when a client disconnects. A client that connects again gets the same screen and the same battle.
  - A newer client takes over: when a client authenticates while another client is connected, the core closes the older connection and serves the newer one. The session stays. Thus a client that hangs, or a connection that a client forgot, does not block the next client.
  - The core writes each response with a limit of 5 seconds. If a client does not read its responses in that time, the core closes the connection. The session stays.
  - Idle exit: the idle time counts from the last line that the core received from the connected client, or from the moment that the last connection closed, whichever is later. With no client connected, the process stops after `--idle-ms` milliseconds (5000 by default; before the first client, 6 times longer). With a client connected, it stops after `--client-idle-ms` milliseconds (1800000, 30 minutes, by default) with no line from that client. A client that waits longer than that for the player sends a request such as `view` from time to time. `--keep-alive` removes both limits. `quit` always stops the process. Each limit must be more than 0.
- In the process of a client: the library `chrogue_core` of the crate `chrogue-embed` (`embed/include/chrogue_core.h`). The desktop client can load it, and the WebAssembly build of the game has it.
  - `chrogue_open` takes a JSON object with the fields `save_dir` (as `--save-dir`; with no `save_dir`, the saved data stays in memory, as `--no-save`), `seed` (as `--seed`, a number or a string of digits), and `debug` (as `--debug`). If the core cannot open, the function returns no core, and `chrogue_open_error` gives the reason (for example, another core holds the lock of the save directory).
  - `chrogue_command` takes one request and returns its response, with no newline. The call blocks until the response is ready. The limit of 65536 bytes applies. A call with no core, and a fault that `Session::command` did not catch, give an error response with the code `internal` and no `view`. A client stops when it gets such a response.
  - No token is necessary, and no time limit applies. `quit` gives its event, but the core stays open until the client calls `chrogue_close`.
- `--save-dir PATH` keeps the saved data in `PATH/meta.json` and `PATH/run.json`. `--no-save` keeps it in memory. One of the two is necessary. See [Saved data](#saved-data) for the lock and for files that the core cannot read.
- `--seed N` sets the random numbers. N is a whole number from 0 to 18446744073709551615, with digits only. All randomness comes from the seed: enemy armies, rewards, shop items, and the noise of the AI. The same seed and the same requests give the same responses, byte for byte.
- `--debug` permits the debug commands.
- A line that is longer than 65536 bytes gets the error `too_long`. An empty line gets no response.
- Exit codes: 0 after `quit`, at the end of stdin, or at the idle limit; 1 for an I/O error (for example, the port is in use); 2 for a usage error (a bad argument, a missing or bad `CHROGUE_TOKEN`); 3 if another live `chrogue-core` holds the lock of the save directory. The reason is on stderr.

### Authentication

With `--listen`, the core reads a token from the environment variable `CHROGUE_TOKEN`. The token is not a command-line flag, thus it does not show in a list of the processes. The variable is necessary with `--listen`: if it is missing, has fewer than 32 characters, has more than 1024 characters, or has a character that is not printable ASCII (a space, `"`, `\`, a control character, or a character above ASCII), the core writes the reason on stderr and exits with code 2. The program that starts the core makes a random token (for example 32 random bytes as 64 hex digits) and gives the same token to its client.

1. The first line of each TCP connection must be exactly `{"auth":"<token>"}`: these bytes, with no spaces and no other fields, and then `\n` (a `\r` before the `\n` is permitted). The core compares the line with the expected line in constant time.
2. The core answers `{"ok":true,"auth":true}`. Then the normal requests follow on the same connection. The client can send its first request immediately after the auth line, with no wait for the answer.
3. If the first line is anything else, the core closes the connection with no answer, and it runs no command. Examples: a wrong token, an auth line with spaces or more fields, a request, an empty line, and a line that starts with an HTTP method (`GET `, `POST `, `PUT `, `OPTIONS `, `HEAD `, `DELETE `, `PATCH `, `CONNECT `, `TRACE `). The core closes an HTTP request as soon as it sees the method, thus a web page that sends a request to the port gets no answer.
4. A connection that does not send its complete auth line within 3 seconds of the connect is closed. A connection that waits for its auth line does not delay another connection. The core keeps at most 8 connections that wait for their auth line; it closes more at once.

### Saved data

- The lock: with `--save-dir`, the core creates the file `PATH/chrogue-core.lock` (exclusively, if it is not there) and holds an operating-system lock on it for the life of the process (`flock` on Linux and macOS, `LockFileEx` on Windows, through `std::fs::File::try_lock`). The file has the PID of the core. If a live core holds the lock, a second core on the same directory writes `another chrogue-core (PID N) uses this save directory` on stderr and exits with code 3; the first core continues. The system releases the lock when a process ends in any way, thus a lock of a core that crashed or was killed is stale: no process holds it, and the next core takes it over (it writes `took over the stale lock` on stderr). A clean exit empties the file; the file stays. The check is the same on each system, and it does not depend on the PID, thus a PID that a new process uses again does not matter.
- The WebAssembly build has no lock. A page of a browser has a file system of its own, with no file locks and no second process. Two pages of the same game each keep their own copy of the saved data, and the page that saves last sets the data that the browser keeps.
- A save writes a temporary file with a name of its own (`meta.json.<pid>-<n>.tmp`), syncs it to the disk, and renames it over the document. On Unix, the core then syncs the directory. A start of the core removes the temporary files that a crash left.
- A saved file that the core cannot use is never deleted or written over. That is a file that the core cannot read, that is not JSON (for example, a file that a crash cut, or JSON nested too deep), that is not a document of its format, whose data is not valid as a whole (see below), or that has a version newer than the version of the core. Before it writes anything, the core renames such a file to `<name>.bad-<unix time>` (for example `meta.json.bad-1759420800`; `-2`, `-3`, ... if that name is taken), loads no data from it, and puts a `save_problem` event in its first successful response. If the rename fails, the core does not write or remove that document in the session; each save of it then gives `save_failed`.
- The checks of saved data are those of `src/game/storage.ts`, and more: a count (a floor, gold, an id, a level, an amount) is at most 2^53 - 1; a unit id is at most 19999 and appears one time; a relic id counts one time in a list; the enemy has at most 64 pieces; no two pieces are on one square (two enemy pieces, an enemy piece on the home square of a unit, or two units with one home); and each side has exactly one king. A start with the enemy king in check is valid (see [Open issues](#open-issues)). A run that fails a check is not valid as a whole. A field of the meta that is not valid counts as 0, and an unknown relic id, upgrade id, or offer is dropped, as in the browser game.

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
| `enemy_move` | | `enemy` | The AI of the floor selects and plays the enemy move. The command blocks until the move is done. On floor 8 a move usually takes less than 100 ms; a board with many queens (an army of 7 queens against 23 queens) takes up to about 190 ms. |
| `give_up` | | `player`, `enemy` | Ends the run as a loss. Goes to `over`. |
| `continue` | | `over` | Goes to `camp`, or to `over` if the run is won or lost. The core settled the battle when it ended (see below). |
| `to_title` | | any | Goes to the title. A battle with no result is not saved: `continue_run` starts it again from its start, as a reload of the browser game does. After the result, the title has the run in the camp, or no run if the run ended. |

The core settles a battle with the move that ends it (the move that gives the `result` event): the gold, the lost units, and the promoted pieces go to the run, the run goes to the camp before its next floor or it ends, and the core saves this at once. The view of the battle shows the run of the battle until `continue`. Thus a client that starts again after the result continues in the camp, or has no run. It cannot play the battle a second time.

The client owns the pause before the enemy move. The browser game waits 350 ms after the move of the player, then sends the equivalent of `enemy_move`.

### Camp

| Command | Arguments | Effect |
| --- | --- | --- |
| `take_reward` | `index`: a card of `reward.offers` | Takes the reward. Errors `reward_closed`, `bad_index`, `blocked`. |
| `skip_reward` | | Takes no reward. Error `reward_closed`. |
| `buy` | `index`: an item of `shop.offers` | Buys the item. Errors `bad_index`, `blocked`, `not_affordable`. |
| `reroll` | | Pays `shop.reroll_cost` gold for new shop items. Error `not_affordable`. |
| `place` | `unit`: a unit id, `square`: 0 to 15 | Moves the unit to the home square. If a unit is there, the two units swap. |
| `start_battle` | | Starts the battle of the next floor. Error `reward_pending` while the reward is open. Error `blocked` if a unit is on the square of an enemy piece (only a debug enemy can be on the first two ranks); move the unit. |
| `to_title` | | Goes to the title. The run is saved. |

`take_reward`, `skip_reward`, `buy`, and `reroll` give the event `camp_action`. `place` gives `unit_placed`. `start_battle` gives `screen` (`battle`) and `battle_start`. `to_title` gives `screen` (`title`).

### Over

| Command | Effect |
| --- | --- |
| `new_run` | Starts a new run. |
| `open_upgrades` | Goes to the upgrades screen. |
| `to_title` | Goes to the title. |

### Debug commands

These commands work only in a session with `--debug`. Else the error is `debug_disabled`. They are the debug menu of the browser game (`src/ui/debug.ts`), and some more commands for tests. A change of the meta or of the run is saved immediately. If the screen is a battle with no result, the battle starts again, as the debug menu does when it closes. After the result, the change goes to the run after the battle (error `no_run` if the run ended), and the screen stays. Each change gives the event `debug_changed`.

A command that changes the pieces or their rules (`debug_set_army`, `debug_set_enemy`, `debug_add_unit`, `debug_remove_unit`, `debug_set_floor`, `debug_set_relic`, `debug_set_trait`) must leave a board that the engine can take as it is. Else the error is `bad_args` with a message that starts with `The board is not valid`, and the run does not change: two pieces on one square (two enemy pieces, or an enemy piece on the home square of a unit), or a side with no king or with two kings. An enemy piece on an empty home square is permitted. A start with the enemy king in check is permitted, as in the camp and in saved data (see [Open issues](#open-issues)). The pawn of Conscription goes to the first square of ranks 2 and 3 that no unit and no enemy piece has.

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
- `special`: `none`, `double_step`, `en_passant`, or `castle`.
- `scout`: true if the player has the Scout upgrade. `enemy_moves`: with Scout in the phase `player`, the moves of each enemy piece as if the enemy had the move, with the field `preview` set to true. A client shows them as marks only.
- `taken`: the kinds that each side captured: `{"w": [...], "b": [...]}`. `w` has the enemy pieces that the player captured.
- `gold`: the gold of the run. `capture_gold`: the gold from captures in this battle, rounded as the game rounds it. `capture_gold_exact`: the same before rounding (Bounty gives halves).
- `lost`: the ids of the units that the enemy captured. `rescued`: the ids of captured units that return after the battle (Second Wind).
- `relics`: the relics of the player: `id`, `name`, `text`. `traits`: the traits of the enemy: `id`, `name`, `text` (the text for the enemy).
- `clock`: the half moves since the last capture. Only a capture resets it. At 100 the battle is a draw.
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
| `save_problem` | `what` (`meta` or `run`), `reason`, `message`, `kept` | A saved file at the start of the core could not be used (see [Saved data](#saved-data)). The session started with no data from it. The first successful response of the session has one event for each such file, before the events of the command; later responses do not have it. `reason`: `unreadable` (the core cannot read the file, or it is not JSON, for example a file that a crash cut), `invalid` (not a document of its format, an older version, or data that is not valid as a whole), or `newer_version` (a newer core wrote it; the core never writes over it). `kept`: the name of the copy in the save directory (`meta.json.bad-1759420800`), or `null` if the storage keeps no copy (`--no-save`) or the rename failed (then `message` tells it, and the core does not save that document in this session). A client tells the player that the saved progress or run could not be loaded and where the file is. |
| `quit` | | The session stops. |

## Codes for the client text

The client has the interface text (key labels, status lines, result sentences). It selects the text with these codes:

- `phase`: `player`, `enemy`, `over`.
- `reason`: `checkmate`, `stalemate` (the side that has no legal move loses), `rout` (the loser has only its king), `bare` (only the kings remain, a draw), `clock` (50 moves with no capture, a draw).
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
| `blocked` | The run cannot take the offer (see `blocked`), a debug limit, or the battle cannot start because a unit is on the square of an enemy piece. |
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

- A draw on the last floor: the run stays on floor 8, in the two games. `continue` opens the camp before floor 8 again, with no reward.
- The camp can give a start where the enemy king is in check: a rook or a queen of the player on an open e-file against an enemy with no pawn on e7. As in the TypeScript game, `start_battle` starts that battle, and White can capture the king. The debug commands and saved data permit such a start too (`gametest/parity.ts` compares random starts of this kind with the TypeScript game). A rule that refuses it belongs in `start_battle`, and it changes the game: it is a decision for the design.
- A battle can start in a position where the player has no legal move (for example, a debug army that is in checkmate at the start). As in the TypeScript game, the result is checked only after a move: the phase is `player` with no moves, and only `give_up` and `to_title` work.
