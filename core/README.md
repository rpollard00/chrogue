# Chrogue core

This directory has the Rust core of Chrogue. At this time it has two parts:

- The chess rules: the move generation, the functions that make and unmake a move, and the result of a battle.
- The enemy AI: a search with an evaluation that comes from the movement rules of the battle.

The TypeScript engine in `src/engine/` is the reference. The Rust engine gives the same moves and the same results for the rules that the game has today. A differential test compares the two engines on random games and on prepared positions.

The Rust engine is not a copy of the TypeScript engine. The movement of each kind is data: the steps and slides of the officers, the pawn moves, the first-move atoms, en passant, the promotion, the 50-move clock, and the castles. Thus a new movement rule needs no new engine code.

## Commands

Run the commands from the `core/` directory, unless the command shows a different directory.

- Build: `cargo build --release`
- Tests: `cargo test --release`
- Perft from the start position at depth 6 (a slow test): `cargo test --release -- --ignored`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt --check`
- Differential test, from the repository root: `bun core/difftest/run.ts`
- Speed of the Rust engine: `cargo run --release --bin perft -- 6`
- Speed of the TypeScript engine, from the repository root: `bun core/difftest/bench.ts 5`
- Type check of the scripts, from the repository root: `bunx tsc -p core/difftest`
- Tactics tests of the AI: `cargo test --release --test tactics`
- Derived piece values: `cargo run --release --bin ai -- values`
- Speed of the AI for each level: `cargo run --release --bin ai -- speed`
- Self-play match: `cargo run --release --bin arena -- --a floor8 --b reference`. See "Self-play" below.

## Structure

- `engine/`: The library crate `chrogue-engine`. It has no dependencies.
  - `src/types.rs`: Squares, colors, kinds, pieces, moves, and the move list. The move list holds 384 moves without the heap. A list with more moves puts its moves on the heap.
  - `src/rules.rs`: The movement rules as data.
  - `src/tables.rs`: The lookup tables that come from the rules. The engine builds them one time for each battle.
  - `src/movegen.rs`: The move generation and the attack detection. A kind with one plain group of atoms (see below) takes its leaps and slides as bitboards. Another kind (the pawn) takes a list of probes for each square that the tables compute from its atoms. A piece that can capture en passant goes group by group. The three paths give the same moves.
  - `src/state.rs`: The state of a battle, `make`, and `unmake`. Only `make`, `unmake`, and the null move change the side to move and the en passant square after construction.
  - `src/outcome.rs`: The result of a battle.
  - `src/perft.rs`: The count of move sequences. The tests compare it with the known counts of chess positions.
  - `src/zobrist.rs`: The Zobrist keys. `State::key` gives the key of a state.
  - `src/eval.rs`: The piece values that come from the rules, and the evaluation of a position.
  - `src/search.rs`: The search.
  - `src/level.rs`: The levels of the AI and `choose_move`.
  - `src/reference.rs`: The algorithm of the old TypeScript AI. It is a baseline opponent only.
  - `src/rng.rs`: A small seeded random number generator.
  - `src/fen.rs`: A reader for the piece field of a FEN string. The tests and the tools use it.
  - `tests/`: Perft counts, the cases of `test/engine.test.ts`, rules that the TypeScript engine does not have (`data_rules.rs` for the officers, `data_moves.rs` for pawns, en passant, castles, ranges, and first-move atoms), the Zobrist key, the derived values, and the tactics of the AI. `tests/property.rs` compares the engine with a naive move generator on 6000 random rule sets: random atoms for each kind (the pawn too) with random ranges, conditions, en passant properties, clock properties, promotions, and castle rows. It also walks the move tree of 1500 more rule sets, compares each `make` with a naive `make`, and makes sure that `unmake` gives back the state and the key.
- `tools/`: The crate `chrogue-tools`. It has the binaries `difftest` (the Rust side of the differential test), `perft` (a timer), `arena` (self-play matches), and `ai` (values, speed, and the move for one position). Only `difftest` uses `serde_json`.
- `difftest/`: The Bun scripts. `run.ts` is the differential test. `bench.ts` measures the TypeScript engine.

## Movement rules

Each side of a battle has its own `SideRules`. `Rules` holds the rules of White and of Black. A `SideRules` has a `KindRules` for each kind (pawn, knight, bishop, rook, queen, king), and a list of castles.

### Atoms

Each kind, the pawn too, moves by a list of atoms. An `Atom` has these fields:

- `offsets`: Steps of (file, rank) from the view of White. For Black, the engine mirrors the rank step. Thus (0, 1) is one square forward for each side.
- `max_steps`: The piece goes 1 to `max_steps` steps along an offset. Each step must end on an empty square, except the last step, which can capture. 1 is a leap: pieces inside one step do not block it. `Atom::MAX_STEPS` (7) is a slide that stops only at a piece or at the edge. A number between them is a slide with a range.
- `mode`: What the atom can do on its target square. `Mode::MoveOrCapture`, `Mode::MoveOnly` (the atom attacks no square, thus it does not give check), or `Mode::CaptureOnly` (the atom attacks its squares, thus it gives check).
- `condition`: `Condition::Always`, or `Condition::Unmoved`: only while the piece has not moved. Such an atom also attacks only while the piece has not moved.
- `makes_en_passant`: The squares that a move of the atom passes become the en passant squares of the next half move, and the piece that moved is the victim of an en passant capture there. A move of one step passes no square.
- `captures_en_passant`: The atom can go to an en passant square as if the square has the victim, and the victim is captured. The atom must be able to capture.
- `resets_clock`: A move of the atom that is not a capture resets the 50-move clock. A capture always resets it.

`Atom::leap(offsets, mode)` and `Atom::slide(dirs, mode)` make an atom with no condition and no property. The methods `max_steps(n)`, `if_unmoved()`, `makes_en_passant()`, `captures_en_passant()`, and `resets_clock()` add the others.

### The moves of a piece

The atoms of a kind with the same condition and the same three properties form one group. The groups come in the order of their first atom. The engine gives the moves of a piece group by group:

- A group gives its targets in ascending order, then its en passant captures.
- A target that an earlier group gave is not given again. Thus the first group decides the properties of a move.
- An en passant capture takes the place of a quiet move to the same square.
- A target is a `Special::DoubleStep` move if a slide of a group with `makes_en_passant` gives it after one square or more. Its en passant squares are the squares that it passes on each such slide of the kind that can be used.
- If a kind has an atom with `resets_clock`, a quiet move of a group without it is a `Special::Backward` move: it keeps the clock. The other quiet moves of the kind reset the clock. An atom with `makes_en_passant` in such a kind must also have `resets_clock` (`RulesError::EnPassantKeepsClock`), because a move has one special property only.

### Promotion

`KindRules::promotion` is `None` or a `Promotion`: the distance of the zone from the last rank (0 is ordinary chess, at most 6) and the `Promotions` (one to four different kinds, not the pawn or the king). A move that ends in the zone gives one move for each promotion kind, in the order of the list. A move that goes backward never promotes. A move that promotes makes no en passant squares.

### Castles

`SideRules::castles` is a list of `Castle` rows, written from the view of White and mirrored for Black. A row has the `from` and `to` squares of the king, the kind and the `from` and `to` squares of the partner, the squares that must be empty, and the squares that the enemy must not attack. The castle is possible when the king and the partner are on their squares and have not moved, the squares of `empty` and the two `to` squares are empty (a square of the king or the partner can be a `to` square), and the enemy attacks no square of `safe`. The legality filter checks the `to` square of the king, as for each move. `Castle::STANDARD` is the two castles of ordinary chess. `Rules::castle_partner` gives the partner move of a castle.

If the movement of the king can go to the `to` square of a castle, the engine gives only the castle when the castle is possible, and the king move when it is not. Two rows of a side cannot have the same king squares, and the king of a row must move.

### The rules of ordinary chess and the six flags

`SideRules::standard()` is ordinary chess:

- The pawn: `leap([(0, 1)], MoveOnly).resets_clock()`, `slide([(0, 1)], MoveOnly).max_steps(2).if_unmoved().makes_en_passant().resets_clock()`, and `leap([(-1, 1), (1, 1)], CaptureOnly).captures_en_passant().resets_clock()`, with `Promotion::STANDARD`.
- The officers: the knight leaps, the bishop, rook, and queen slides, and the king steps.
- `Castle::STANDARD`.

The six rule flags of the TypeScript engine are edits of it:

| Flag | Method | Edit |
| --- | --- | --- |
| `forcedMarch` | `forced_march()` | The pawn atoms with `makes_en_passant` lose their condition. |
| `backpedal` | `backpedal()` | The pawn gets `leap([(0, -1)], MoveOnly)`. It has no `resets_clock`, thus its moves are `Backward` moves. |
| `earlyPromo` | `early_promo()` | The promotion distance of the pawn becomes 1. |
| `kingKnight` | `king_knight()` | The king gets `leap(KNIGHT, MoveOrCapture)`. |
| `longLeap` | `long_leap()` | The knight gets `leap(CAMEL, MoveOrCapture)`. |
| `sidestep` | `sidestep()` | The bishop gets `leap(ORTHO, MoveOnly)`. |

`SideRules::from_flags(["kingKnight", "sidestep"])` makes the rules from the flag names.

`Tables::new`, `State::new`, and `fen::from_fen` give an error for rules or pieces that are not valid.

### Add a movement rule

1. Write the rule as an edit of `SideRules`. Do not change `tables.rs` or `movegen.rs`.
2. If the game needs a name for the rule, add a method to `SideRules` in `engine/src/rules.rs`.
3. Add a test to `engine/tests/data_rules.rs` or `engine/tests/data_moves.rs` for the moves and for the check that the rule gives.

The examples use these imports:

```rust
use chrogue_engine::fen::square;
use chrogue_engine::rules::{BACKWARD, CAMEL, DIAG, FORWARD, KNIGHT, ORTHO};
use chrogue_engine::{Atom, Castle, Kind, Mode, Promotion, Promotions, Rules, SideRules};
```

A new leap or slide. The rooks of White also jump as knights:

```rust
let white = SideRules::standard().with_atom(Kind::Rook, Atom::leap(&KNIGHT, Mode::MoveOrCapture));
let rules = Rules::new(white, SideRules::standard());
```

A mode. A queen that cannot capture:

```rust
let side = SideRules::standard()
    .with_kind(Kind::Queen, vec![Atom::slide(&ORTHO, Mode::MoveOnly), Atom::slide(&DIAG, Mode::MoveOnly)]);
```

A range. A king that also slides two squares on files and ranks:

```rust
let side = SideRules::standard().with_atom(Kind::King, Atom::slide(&ORTHO, Mode::MoveOrCapture).max_steps(2));
```

A condition. A knight that also jumps as a camel on its first move:

```rust
let side = SideRules::standard().with_atom(Kind::Knight, Atom::leap(&CAMEL, Mode::MoveOrCapture).if_unmoved());
```

Pawn moves. Pawns that capture straight ahead and not diagonally: replace the pawn atoms.

```rust
let side = SideRules::standard().with_kind(Kind::Pawn, vec![
    Atom::leap(&FORWARD, Mode::MoveOrCapture).resets_clock(),
    Atom::slide(&FORWARD, Mode::MoveOnly).max_steps(2).if_unmoved().makes_en_passant().resets_clock(),
]);
```

A sideways step that only moves, a backward capture, and a diagonal move are added the same way:

```rust
let side = SideRules::standard()
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, 0), (1, 0)], Mode::MoveOnly).resets_clock())
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, -1), (1, -1)], Mode::CaptureOnly).resets_clock())
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, 1), (1, 1)], Mode::MoveOnly).resets_clock());
```

En passant. A first step of up to three squares. A step of three squares makes two en passant squares, and an enemy pawn can capture on each of them:

```rust
let mut side = SideRules::standard();
side.kinds[Kind::Pawn.index()].atoms[1].max_steps = 3;
```

The clock. A move of an atom without `resets_clock` keeps the clock if another atom of the kind resets it, as the backward step does:

```rust
let side = SideRules::standard().with_atom(Kind::Pawn, Atom::leap(&BACKWARD, Mode::MoveOnly));
```

A promotion. Pawns of one side promote one rank earlier, and only to a knight:

```rust
let promotion = Promotion { distance: 1, kinds: Promotions::new(&[Kind::Knight])? };
let white = SideRules::standard().with_promotion(Kind::Pawn, Some(promotion));
```

A castle. A queen in the corner of the rook castles to the queen side. With `with_castling(false)` or an empty list, the side has no castle:

```rust
let queen_castle = Castle { partner: Kind::Queen, ..Castle::QUEEN_SIDE };
let side = SideRules::standard().with_castles(vec![queen_castle, Castle::KING_SIDE]);
let custom = Castle {
    king_from: square("e1"), king_to: square("b1"), partner: Kind::Rook,
    partner_from: square("a1"), partner_to: square("c1"),
    empty: 1 << square("b1") | 1 << square("c1") | 1 << square("d1"),
    safe: 1 << square("e1") | 1 << square("d1") | 1 << square("c1"),
};
```

If the TypeScript engine also gets the rule, add its flag name to `SideRules::with_flag` and to `FLAGS` in `difftest/run.ts`. Then the differential test includes the rule.

## Behavior that the engine keeps from the TypeScript engine

- A piece has its own `moved` flag. Castles and atoms with `Condition::Unmoved` read this flag. The state has no castling rights.
- With `forcedMarch`, a pawn can do a double step from each rank, and the step makes an en passant square.
- A double step into the promotion zone promotes and makes no en passant square. In general: a move that promotes makes no en passant squares.
- A backward step is never a capture, never promotes, and does not reset the clock. In general: a move that goes backward never promotes.
- An en passant capture removes the piece that made the en passant squares. With the six flags, only a pawn double step makes them. `State::with_en_passant` takes the square of a double step, and its pawn is the victim.
- A side with no king is never in check. If a side has two kings, only the king on the lowest square can be in check.
- A move can capture a king when the side that does not have the move is in check.
- `outcome` does its checks in this order: bare, rout, checkmate or stalemate, clock. A side with no legal move loses.

The TypeScript engine has no rule where a king move and a castle have the same squares. If the movement of the king can go to a castle square, the Rust engine gives only the castle when the castle is possible, and the king move when it is not.

The order of the moves is the same as before the rules became data, thus the search gives the same results. The Zobrist key has the `moved` flag of the pawn, the rook, and the king, as in ordinary chess. If the rules read the flag of another kind, the key has the flag of each kind (`zobrist.rs`).

## AI

`choose_move(&mut state, &level, seed)` gives the move of the side that has the move, or `None` if the side has no legal move. The result has the move, its score in centipawns, the depth, the number of nodes, and the expected line. The state is the same after the call. The same state, level, and seed give the same result, if the level has no time limit.

### Values that come from the rules

The engine has no table of piece values and no code for a specific relic. `Tables::new` computes the value of each kind for each side from the rules data. `engine/src/eval.rs` has the formula:

- The *reach* of a kind is the mean, over its squares, of `W_MOVE * (empty squares where it can move) + W_ATTACK * (attacked squares with no friend)` on a board where a square is empty with the probability `P_EMPTY`. A square of a slide counts only if the squares before it are empty, and a slide stops after `max_steps`. An atom with `Condition::Unmoved` counts only on the first two ranks.
- The *coverage* is the part of the board that the piece can get to in any number of moves.
- `value = (VALUE_PER_REACH * reach + VALUE_PER_REACH_SQUARED * reach^2) * (1 - BOUND * (1 - coverage))`
- A kind with a promotion (the pawn) has the same reach from its atoms, over the squares from rank 2 to the last rank before its zone. It has no coverage factor, because it leaves the board as itself when it promotes; with the factor, the pawn of ordinary chess would be worth 90 and the pawn of `backpedal` (which can get to each square) 115. It gets `PROMO_SHARE * (value of the best promotion kind) * PROMO_DECAY^(moves to the promotion zone)`, with the moves on an empty board from rank 2 by the atoms that can move.

The constants are `W_MOVE = 1`, `W_ATTACK = 3`, `P_EMPTY = 0.7`, `VALUE_PER_REACH = 18.157`, `VALUE_PER_REACH_SQUARED = 0.03456`, `BOUND = 0.178`, `PROMO_SHARE = 0.2`, `PROMO_DECAY = 0.5`. They were set one time so that ordinary chess gives values near 100, 320, 330, 500, and 900. The evaluation of the game does not read those five numbers.

| Rules | Pawn | Knight | Bishop | Rook | Queen | King |
| --- | --- | --- | --- | --- | --- | --- |
| Ordinary chess | 102 | 320 | 330 | 502 | 900 | 403 |
| `forcedMarch` | 125 | | | | | |
| `backpedal` | 115 | | | | | |
| `earlyPromo` | 108 | | | | | |
| `kingKnight` | | | | | | 748 |
| `longLeap` | | 602 | | | | |
| `sidestep` | | | 411 | | | |

The value of the king is not a part of the material. The move order uses it.

### Evaluation

`Evaluator::evaluate` gives the score for the side that has the move. Its terms are:

- Material, with the values of the battle.
- Mobility: for each officer, its reach in the position against the mean reach of its kind.
- Pawn advance: a bonus from the number of moves to the promotion zone of that side.
- King safety: the part of the squares that the king can reach that the enemy attacks, and a penalty for a king with no safe square.
- Threats: a piece that a less valuable enemy piece attacks, or that an enemy piece attacks with no defender.
- Rout: a penalty that grows when the material of a side gets small. Thus the side that is ahead wants trades.
- Clock: from 70 half moves on the clock, the score goes linearly to 0 at 100.

### Search

The search has iterative deepening, negamax with alpha-beta and principal variation search, a transposition table with the Zobrist key, a move order (the table move, captures by victim and attacker with the values of the battle, two killer moves, history), late-move reductions, a check extension, and a quiescence search of captures and promotions.

The ends of a battle have these scores: a side with no legal move loses (`-MATE + ply`), a rout is a loss with the same score, bare kings and the limit of the clock are draws. The game has no rule for a repeated position, thus the search has none. The capture of a king is possible only from a state where the side that does not have the move is in check; the search gives it the score of a mate.

Null-move pruning is in the code and is off: it did not help in self-play.

### Levels

`Level::LADDER` has one level for each floor. `Level::floor(n)` gives the level of floor `n`. A level has a depth limit, a node limit, and noise. With noise, each root move gets a random bonus from 0 to `noise_cp` centipawns. The bonus comes from the seed and the move.

| Floor | Name | Depth limit | Node limit | Noise |
| --- | --- | --- | --- | --- |
| 1 | Border Patrol | 1 | 300 | 150 |
| 2 | Scouts | 2 | 1 000 | 90 |
| 3 | Garrison | 3 | 2 500 | 50 |
| 4 | The Warden | none | 6 000 | 25 |
| 5 | Cavalry | none | 15 000 | 12 |
| 6 | Royal Guard | none | 36 000 | 6 |
| 7 | Vanguard | none | 100 000 | 0 |
| 8 | The Black King | none | 320 000 | 0 |

`Level::reference()` is the algorithm of `src/engine/ai.ts` at depth 2 with no noise. It is a baseline for the self-play tool only.

### Self-play

`arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N] [--seed N] [--max-plies N] [--threads N]` plays two configurations against each other and prints the wins, the draws, the losses, the score, and a 95% interval.

- A CONFIG is `floor1` to `floor8`, `reference`, or `nodes=N`, and then options with `,` between them: `eval=derived|fixed|blind`, `noise=CP`, `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`, `threats=0|1`.
- `eval=fixed` has the usual piece values of chess for each side. `eval=blind` has an evaluation that knows only the rules of chess.
- The starts are the start position of chess and `--positions` armies in the style of the game: the base army of the player plus recruits against the enemy army of floor 3 to 8 with the same total value.
- The rule sets are: ordinary chess, each flag for White only, each flag for Black only, each flag for the two sides, and eight mixed combinations.
- Each start is played two times from the same position and rules: A as White against B, then B as White against A.
- A game that gets to `--max-plies` half moves (300) is a draw.

The same arguments give the same output.

## Differential test

`bun core/difftest/run.ts [--seed N] [--playouts N]` plays random legal games with the TypeScript engine. The games start from the chess start position, Kiwipete, armies of the game against the enemy army of a floor, random positions, and prepared positions. Each game has a rule combination for each side.

For each sampled position, the script compares these results of the two engines:

- The legal moves, with all the fields of each move.
- `movesFrom` for each occupied square.
- The pseudo moves of the two colors, with and without `capturesOnly`.
- `isAttacked` for each square and each color, and `inCheck` for each color.
- `outcome`.
- Perft at depth 1 and depth 2, and at depth 3 on one position of six.

The script also sends each game to the Rust tool. The tool plays the moves and the script compares the full state after each move. After each move, the tool compares its bitboards with its mailbox and its Zobrist key with the key from all the pieces. Then the tool takes back each move and compares the state with the state before that move.

The game starts use the ids of the game: numbers for the army of the player, `"e0"`, `"e1"`, and so on for the enemy, and `"conscript"`. The engine has `u16` ids. The Rust tool gives each id of a request its own number, and writes the ids back in its answer. Thus the script compares the ids of the game.

The run also has these games and positions:

- 64 short games from a start with white pawns on rank 1 and earlyPromo for Black. These games give en passant captures that promote. The script compares each position of these games.
- Prepared endings where more than one end is true: checkmate and stalemate with 100 or more on the clock, and rout and bare kings with a high clock. These positions compare the order of the checks in `outcome`.

If the engines disagree, the script prints the position, the rules, and the moves that only one engine has. The exit code is 1. The exit code is also 1 if the run has fewer than 3000 positions, if a rule combination has fewer than 20 positions, or if a kind of move or a result occurs fewer times than its minimum in `MINIMUMS`. The minimums are for en passant captures, en passant captures that promote, backward steps, checks, promotions to each kind, castles of each side to each wing, and each result.

The script uses its own random numbers, thus the same seed gives the same run.

## Game layer and command server

The crate `game/` (`chrogue-game`) has the roguelite layer of `src/game/`: runs, relics, upgrades, offers, floors, battles, and saved data. The crate `server/` has the binary `chrogue-core`. A client, a test, or an agent plays the full game through one JSON protocol with no interface. `PROTOCOL.md` documents the protocol.

The TypeScript game in `src/game/` is the reference. `gametest/parity.ts` proves that the two give the same content and the same results.

### Commands

Run the commands from the `core/` directory, unless the command shows a different directory.

- Server on stdio: `cargo run --release --bin chrogue-core -- --stdio --no-save`
- Server on TCP: `cargo run --release --bin chrogue-core -- --listen 127.0.0.1:0 --save-dir /path/to/saves`
- Tests of the game layer and of the server: `cargo test --release -p chrogue-game -p chrogue-server`. With `-- --nocapture`, the fuzz test prints the error codes and the TCP test prints the round-trip times.
- Parity with the TypeScript game, from the repository root: `bun core/gametest/parity.ts [--seed N] [--battles N]`
- Whole runs with a bot, two times, with a comparison of the transcripts, from the repository root: `bun core/gametest/play.ts [--sessions N] [--runs N] [--seed N]`. The default (36 runs, two passes) takes about four minutes.
- Type check of the scripts, from the repository root: `bunx tsc -p core/gametest`

### Structure

- `game/src/content.rs`: The relics, upgrades, floors, prices, and constants as data. The text is a copy of the TypeScript text.
- `game/src/run.rs`: `Meta`, `Run`, `Unit`, `Enemy`, `Offer`, the enemy of each floor, rewards, the shop, and upgrades.
- `game/src/battle.rs`: One battle on the engine: relic effects, gold, lost and rescued units, the reward, and `settle`.
- `game/src/save.rs`: The `Storage` trait, a file storage and a memory storage, and the check of saved data.
- `game/src/session.rs`: `Screen`, `Session::command`, and the commands.
- `game/src/view.rs`: The views of the screens and the content tables of `hello`.
- `game/src/protocol.rs`: The names of the commands, events, and error codes.
- `game/src/chess.rs`: The one module that calls the engine. A change of the engine API changes only this file.
- `game/tests/`: The cases of `test/game.test.ts` (`game.rs`), saved data (`saved.rs`), random requests (`fuzz.rs`), and the check of `PROTOCOL.md` (`protocol_doc.rs`).
- `server/src/main.rs`: The line transport. `server/tests/tcp.rs` starts the binary and tests TCP and stdio.
- `gametest/`: The Bun scripts `parity.ts` and `play.ts`, and the client `core.ts` that they share.

### Relics as data

A relic has movement rules and an effect. The movement rules are a list of `RuleEdit`: edits of `SideRules::standard()` that the engine and the AI read. The effect is one kind of `Effect`, at a fixed point of a battle. `Effect` has a kind for each hook of `RelicHooks` in `src/game/relics.ts`.

To add a relic, add one entry to `RELICS` in `game/src/content.rs` and the same entry to `src/game/relics.ts`. A new kind of effect needs a kind in `Effect` and code in `battle.rs`. Then run `gametest/parity.ts`.

### Behavior that differs from the TypeScript game

- A draw on the last floor stays on the last floor. The TypeScript game goes to floor 9 and throws.
- The AI of floor `n` is `Level::floor(n)` of the engine, not the `ai` field of `src/game/floors.ts`.
- Saved data: a unit id is at most 19999 and appears one time, and a relic id appears one time in a list.
