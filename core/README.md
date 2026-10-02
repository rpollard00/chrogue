# Chrogue core

This directory has the Rust core of Chrogue. At this time it has two parts:

- The chess rules: the move generation, the functions that make and unmake a move, and the result of a battle.
- The enemy AI: a search with an evaluation that comes from the movement rules of the battle.

The TypeScript engine in `src/engine/` is the reference. The Rust engine gives the same moves and the same results for the rules that the game has today. A differential test compares the two engines on random games and on prepared positions.

The Rust engine is not a copy of the TypeScript engine. The movement of the knight, bishop, rook, queen, and king is data, thus a new movement rule for these kinds needs no new engine code. The pawn moves, en passant, and castling are code that reads options. A new rule of that type needs a new option and new engine code.

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
  - `src/movegen.rs`: The move generation and the attack detection.
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
  - `tests/`: Perft counts, the cases of `test/engine.test.ts`, rules that the TypeScript engine does not have, the Zobrist key, the derived values, and the tactics of the AI. `tests/property.rs` compares the engine with a naive move generator on 6000 random rule sets, and walks the move tree of 1500 more rule sets to make sure that `unmake` gives back the state.
- `tools/`: The crate `chrogue-tools`. It has the binaries `difftest` (the Rust side of the differential test), `perft` (a timer), `arena` (self-play matches), and `ai` (values, speed, and the move for one position). Only `difftest` uses `serde_json`.
- `difftest/`: The Bun scripts. `run.ts` is the differential test. `bench.ts` measures the TypeScript engine.

## Movement rules

Each side of a battle has its own `SideRules`. `Rules` holds the rules of White and of Black.

A knight, bishop, rook, queen, or king moves by a list of atoms:

- `Atom::Leap { offsets, mode }`: The piece jumps to each offset. Pieces between the two squares do not block the jump.
- `Atom::Slide { dirs, mode }`: The piece moves in each direction until a piece blocks the line.

An offset is a step of (file, rank) from the view of White. For Black, the engine mirrors the rank step. Thus (0, 1) is one square forward for each side.

The mode tells what the atom can do on its target square:

- `Mode::MoveOrCapture`: Move to an empty square or capture an enemy piece.
- `Mode::MoveOnly`: Move to an empty square only. This atom attacks no square, thus it does not give check.
- `Mode::CaptureOnly`: Capture an enemy piece only. This atom attacks its squares, thus it gives check.

`PawnRules` has the options of the pawn: the double step, the start of the promotion zone, the backward step, and the promotion kinds. `Promotions::new` accepts one to four different kinds, and not the pawn or the king. `SideRules::castling` permits or prevents castling. The pawn moves, en passant, and castling are code in `movegen.rs` that reads these options.

`Tables::new`, `State::new`, and `fen::from_fen` give an error for rules or pieces that are not valid. `SideRules::with_atom` and `SideRules::with_kind` give an error for the pawn, because the pawn has no atoms.

`SideRules::standard()` is ordinary chess. The six rule flags of the TypeScript engine are edits of it:

| Flag | Method | Edit |
| --- | --- | --- |
| `forcedMarch` | `forced_march()` | `pawn.double_step = DoubleStep::Always` |
| `earlyPromo` | `early_promo()` | `pawn.promo_distance = 1` |
| `backpedal` | `backpedal()` | `pawn.backward_step = true` |
| `kingKnight` | `king_knight()` | The king gets `Leap { KNIGHT, MoveOrCapture }`. |
| `longLeap` | `long_leap()` | The knight gets `Leap { CAMEL, MoveOrCapture }`. |
| `sidestep` | `sidestep()` | The bishop gets `Leap { ORTHO, MoveOnly }`. |

`SideRules::from_flags(["kingKnight", "sidestep"])` makes the rules from the flag names.

### Add a movement rule

1. Write the rule as an edit of `SideRules`. Do not change `tables.rs` or `movegen.rs`.
2. If the game needs a name for the rule, add a method to `SideRules` in `engine/src/rules.rs`.
3. Add a test to `engine/tests/data_rules.rs` for the moves and for the check that the rule gives.

This example gives the rooks of White the knight jump:

```rust
use chrogue_engine::rules::KNIGHT;
use chrogue_engine::{Atom, Kind, Mode, Rules, SideRules};

let white = SideRules::standard().with_atom(Kind::Rook, Atom::leap(&KNIGHT, Mode::MoveOrCapture))?;
let rules = Rules::new(white, SideRules::standard());
```

This example makes a queen that cannot capture:

```rust
use chrogue_engine::rules::{DIAG, ORTHO};

let side = SideRules::standard()
    .with_kind(Kind::Queen, vec![Atom::slide(&ORTHO, Mode::MoveOnly), Atom::slide(&DIAG, Mode::MoveOnly)])?;
```

If the TypeScript engine also gets the rule, add its flag name to `SideRules::with_flag` and to `FLAGS` in `difftest/run.ts`. Then the differential test includes the rule.

A rule that changes the pawn moves, en passant, or castling in a new way needs a new option in `PawnRules` or `SideRules`, and code in `movegen.rs`.

## Behavior that the engine keeps from the TypeScript engine

- A piece has its own `moved` flag. Castling and the pawn double step read this flag. The state has no castling rights.
- With `DoubleStep::Always`, a pawn can do a double step from each rank, and the step makes an en passant square.
- A double step into the promotion zone promotes and makes no en passant square.
- A backward step is never a capture and does not reset the clock.
- A side with no king is never in check. If a side has two kings, only the king on the lowest square can be in check.
- A move can capture a king when the side that does not have the move is in check.
- `outcome` does its checks in this order: bare, rout, checkmate or stalemate, clock. A side with no legal move loses.

The TypeScript engine has no rule where a king move and a castle have the same squares. If the movement of the king can go to a castle square, the Rust engine gives only the castle when the castle is possible, and the king move when it is not.

## AI

`choose_move(&mut state, &level, seed)` gives the move of the side that has the move, or `None` if the side has no legal move. The result has the move, its score in centipawns, the depth, the number of nodes, and the expected line. The state is the same after the call. The same state, level, and seed give the same result, if the level has no time limit.

### Values that come from the rules

The engine has no table of piece values and no code for a specific relic. `Tables::new` computes the value of each kind for each side from the rules data. `engine/src/eval.rs` has the formula:

- The *reach* of an officer is the mean, over all squares, of `W_MOVE * (empty squares where it can move) + W_ATTACK * (attacked squares with no friend)` on a board where a square is empty with the probability `P_EMPTY`. A square of a slide counts only if the squares before it are empty.
- The *coverage* is the part of the board that the piece can get to in any number of moves.
- `value = (VALUE_PER_REACH * reach + VALUE_PER_REACH_SQUARED * reach^2) * (1 - BOUND * (1 - coverage))`
- A pawn has the first factor for its steps and captures, plus `PROMO_SHARE * (value of the best promotion kind) * PROMO_DECAY^(moves to the promotion zone)`.

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
