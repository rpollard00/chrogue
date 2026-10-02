# Chrogue core

This directory has the Rust core of Chrogue. At this time it has the chess rules: the move generation, the functions that make and unmake a move, and the result of a battle. It has no AI search.

The TypeScript engine in `src/engine/` is the reference. The Rust engine gives the same moves and the same results for the rules that the game has today. A differential test proves this.

The Rust engine is not a copy of the TypeScript engine. The movement rules are data, thus a new movement rule for a relic needs no new engine code.

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

## Structure

- `engine/`: The library crate `chrogue-engine`. It has no dependencies.
  - `src/types.rs`: Squares, colors, kinds, pieces, moves, and the move list.
  - `src/rules.rs`: The movement rules as data.
  - `src/tables.rs`: The lookup tables that come from the rules. The engine builds them one time for each battle.
  - `src/movegen.rs`: The move generation and the attack detection.
  - `src/state.rs`: The state of a battle, `make`, and `unmake`.
  - `src/outcome.rs`: The result of a battle.
  - `src/perft.rs`: The count of move sequences. It proves the move generation.
  - `src/fen.rs`: A reader for the piece field of a FEN string. The tests and the tools use it.
  - `tests/`: Perft counts, the cases of `test/engine.test.ts`, and rules that the TypeScript engine does not have.
- `tools/`: The crate `chrogue-tools`. It has the binary `difftest` (the Rust side of the differential test) and the binary `perft` (a timer). It uses `serde_json` for the JSON lines.
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

`PawnRules` has the options of the pawn: the double step, the start of the promotion zone, the backward step, and the promotion kinds. `SideRules::castling` permits or prevents castling. The pawn moves, en passant, and castling are code in `movegen.rs` that reads these options.

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

let white = SideRules::standard().with_atom(Kind::Rook, Atom::leap(&KNIGHT, Mode::MoveOrCapture));
let rules = Rules::new(white, SideRules::standard());
```

This example makes a queen that cannot capture:

```rust
use chrogue_engine::rules::{DIAG, ORTHO};

let side = SideRules::standard()
    .with_kind(Kind::Queen, vec![Atom::slide(&ORTHO, Mode::MoveOnly), Atom::slide(&DIAG, Mode::MoveOnly)]);
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

## Differential test

`bun core/difftest/run.ts [--seed N] [--playouts N]` plays random legal games with the TypeScript engine. The games start from the chess start position, Kiwipete, armies of the game against the enemy army of a floor, random positions, and prepared positions. Each game has a rule combination for each side.

For each sampled position, the script compares these results of the two engines:

- The legal moves, with all the fields of each move.
- `movesFrom` for each occupied square.
- The pseudo moves of the two colors, with and without `capturesOnly`.
- `isAttacked` for each square and each color, and `inCheck` for each color.
- `outcome`.
- Perft at depth 1 and depth 2, and at depth 3 on one position of six.

The script also sends each game to the Rust tool. The tool plays the moves and the script compares the full state after each move. Then the tool takes back each move and reports if the first state returned.

If the engines disagree, the script prints the position, the rules, and the moves that only one engine has. The exit code is 1. The exit code is also 1 if the run has fewer than 3000 positions or if a rule combination has fewer than 20 positions.

The script uses its own random numbers, thus the same seed gives the same run.
