# Chrogue core

This directory has the Rust core of Chrogue. It has these parts:

- The chess rules: the move generation, the functions that make and unmake a move, and the result of a battle.
- The enemy AI: a search with an evaluation that comes from the movement rules of the battle.
- The game layer and the command server: the runs, the saved data, and the commands of a client. See "Game layer and command server".
- The library for a client that loads the core into its own process. See "Game layer and command server".

The core is the source of truth for the rules of the game.

The movement of each kind is data: the steps and slides of the officers, the pawn moves, the first-move atoms, en passant, the promotion, the 50-move clock, and the castles. Thus a new movement rule needs no new engine code.

## Commands

Run the commands from the `core/` directory, unless the command shows a different directory.

- Build: `cargo build --release`
- Tests: `cargo test --release`
- Perft from the start position at depth 6 (a slow test): `cargo test --release -- --ignored`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt --check`
- Speed of the engine: `cargo run --release --bin perft -- 6`
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
  - `src/rng.rs`: A small seeded random number generator.
  - `src/fen.rs`: A reader for the piece field of a FEN string. The tests and the tools use it.
  - `tests/`: Perft counts, the six rule flags and the results of a battle (`relic_rules.rs`), other rules as data (`data_rules.rs` for the officers, `data_moves.rs` for pawns, en passant, castles, ranges, and first-move atoms), the Zobrist key, the derived values, and the tactics of the AI. `tests/property.rs` compares the engine with a naive move generator on 6000 random rule sets: random atoms for each kind (the pawn too) with random ranges, conditions, en passant properties, clock properties, promotions, and castle rows. It also walks the move tree of 1500 more rule sets, compares each `make` with a naive `make`, and makes sure that `unmake` gives back the state and the key.
- `tools/`: The crate `chrogue-tools`. It has the binaries `perft` (a timer), `arena` (self-play matches), and `ai` (values, speed, and the move for one position). Its library has `src/reference.rs`, the reference AI, and `Player`, a level of the engine or the reference AI. The reference AI is the algorithm of the AI that the first version of the game had. It is a baseline opponent only. The engine and the game do not use it.

## Movement rules

Each side of a battle has its own `SideRules`. `Rules` holds the rules of White and of Black. A `SideRules` has a `KindRules` for each kind (pawn, knight, bishop, rook, queen, king), and a list of castles.

### Atoms

Each kind, the pawn too, moves by a list of atoms. An `Atom` has these fields:

- `offsets`: Steps of (file, rank) from the view of White. For Black, the engine mirrors the rank step. Thus (0, 1) is one square forward for each side.
- `max_steps`: The piece goes 1 to `max_steps` steps along an offset. Each step must end on an empty square, except the last step, which can capture. 1 is a leap: pieces inside one step do not block it. `Atom::MAX_STEPS` (7) is a slide that stops only at a piece or at the edge. A number between them is a slide with a range.
- `mode`: What the atom can do on its target square. `Mode::MoveOrCapture`, `Mode::MoveOnly` (the atom attacks no square, thus it does not give check), or `Mode::CaptureOnly` (the atom attacks its squares, thus it gives check).
- `condition`: `Condition::Always`, or `Condition::Unmoved`: only while the piece has not moved. Such an atom also attacks only while the piece has not moved.
- `makes_en_passant`: The squares that a move of the atom passes become the en passant squares of the next half move, and the piece that moved is the victim of an en passant capture there. A move of one step passes no square. An atom of the king cannot have it (`RulesError::KingMakesEnPassant`): an en passant capture never removes a king.
- `captures_en_passant`: The atom can go to an en passant square as if the square has the victim, and the victim is captured. The atom must be able to capture.

`Atom::leap(offsets, mode)` and `Atom::slide(dirs, mode)` make an atom with no condition and no property. The methods `max_steps(n)`, `if_unmoved()`, `makes_en_passant()`, and `captures_en_passant()` add the others.

### The moves of a piece

The atoms of a kind with the same condition and the same three properties form one group. The groups come in the order of their first atom. The engine gives the moves of a piece group by group:

- A group gives its targets in ascending order, then its en passant captures.
- A target that an earlier group gave is not given again. Thus the first group decides the properties of a move.
- An en passant capture takes the place of a quiet move to the same square.
- A target is a `Special::DoubleStep` move if a slide of a group with `makes_en_passant` gives it after one square or more. Its en passant squares are the squares that it passes on each such slide of the kind that can be used.

### Promotion

`KindRules::promotion` is `None` or a `Promotion`: the distance of the zone from the last rank (0 is ordinary chess, at most 6; with 6 the zone starts on the second rank, thus a piece promotes on its first move forward) and the `Promotions` (one to four different kinds, not the pawn or the king). A move that ends in the zone gives one move for each promotion kind, in the order of the list. A move that goes backward never promotes. A move that promotes makes no en passant squares.

### Castles

`SideRules::castles` is a list of `Castle` rows, written from the view of White and mirrored for Black. A row has the `from` and `to` squares of the king, the kind and the `from` and `to` squares of the partner, the squares that must be empty, and the squares that the enemy must not attack. The castle is possible when the king and the partner are on their squares and have not moved, the squares of `empty` and the two `to` squares are empty (a square of the king or the partner can be a `to` square), and the enemy attacks no square of `safe`. The legality filter checks the `to` square of the king, as for each move. `Castle::STANDARD` is the two castles of ordinary chess. `Rules::castle_partner` gives the partner move of a castle.

If the movement of the king can go to the `to` square of a castle, the engine gives only the castle when the castle is possible and legal. When the castle is not possible, it gives the king move. When the castle is possible but not legal (the partner opens a line to the `to` square, for example), the pseudo moves have the castle and the king move, and the legality filter removes the castle. Two rows of a side cannot have the same king squares, and the king of a row must move.

### The rules of ordinary chess and the six flags

`SideRules::standard()` is ordinary chess:

- The pawn: `leap([(0, 1)], MoveOnly)`, `slide([(0, 1)], MoveOnly).max_steps(2).if_unmoved().makes_en_passant()`, and `leap([(-1, 1), (1, 1)], CaptureOnly).captures_en_passant()`, with `Promotion::STANDARD`.
- The officers: the knight leaps, the bishop, rook, and queen slides, and the king steps.
- `Castle::STANDARD`.

The six rule flags are edits of it:

| Flag | Method | Edit |
| --- | --- | --- |
| `forcedMarch` | `forced_march()` | The pawn atoms with `makes_en_passant` lose their condition. |
| `backpedal` | `backpedal()` | The pawn gets `leap([(0, -1)], MoveOnly)`. |
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
    Atom::leap(&FORWARD, Mode::MoveOrCapture),
    Atom::slide(&FORWARD, Mode::MoveOnly).max_steps(2).if_unmoved().makes_en_passant(),
]);
```

A sideways step that only moves, a backward capture, and a diagonal move are added the same way:

```rust
let side = SideRules::standard()
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, 0), (1, 0)], Mode::MoveOnly))
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, -1), (1, -1)], Mode::CaptureOnly))
    .with_atom(Kind::Pawn, Atom::leap(&[(-1, 1), (1, 1)], Mode::MoveOnly));
```

En passant. A first step of up to three squares. A step of three squares makes two en passant squares, and an enemy pawn can capture on each of them:

```rust
let mut side = SideRules::standard();
side.kinds[Kind::Pawn.index()].atoms[1].max_steps = 3;
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

## Details of the behavior of the engine

- A piece has its own `moved` flag. Castles and atoms with `Condition::Unmoved` read this flag. The state has no castling rights.
- With `forcedMarch`, a pawn can do a double step from each rank, and the step makes an en passant square.
- A double step into the promotion zone promotes and makes no en passant square. In general: a move that promotes makes no en passant squares.
- A backward step is never a capture and never promotes. In general: a move that goes backward never promotes.
- The clock counts the half moves since the last capture. Only a capture resets it: no other move does, standard or not. At 100 the battle is a draw.
- An en passant capture removes the piece that made the en passant squares. With the six flags, only a pawn double step makes them. `State::with_en_passant` takes the square of a double step, and its pawn is the victim.
- A side with no king is never in check. If a side has two kings, only the king on the lowest square can be in check.
- A move can capture a king when the side that does not have the move is in check.
- `outcome` does its checks in this order: bare, rout, checkmate or stalemate, clock. A side with no legal move loses.

If the movement of the king can go to a castle square, the engine gives only the castle when the castle is possible and legal, and the king move when it is not.

The order of the moves is the same as before the rules became data, thus the search gives the same results. The Zobrist key has the `moved` flag of the pawn, the rook, and the king, as in ordinary chess. If the rules read the flag of another kind, the key has the flag of each kind (`zobrist.rs`). When the state has en passant squares, the key also has the square of their victim: two atoms can make the same en passant squares with different victims.

## AI

`choose_move(&mut state, &level, seed)` gives the move of the side that has the move, or `None` if the battle has ended (`outcome` is not `None`): bare kings, a rout, no legal move, or the limit of the clock. The result has the move, its score in centipawns, the depth, the number of nodes, and the expected line. The state is the same after the call. The same state, level, and seed give the same result, if the level has no time limit.

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
| `leap(DABBABA, MoveOnly)` for the rook | | | | 514 | | |
| `leap(DIAG, CaptureOnly)` for the rook | | | | 660 | | |
| `leap(ORTHO, CaptureOnly)` for the knight | | 495 | | | | |
| `leap(ALFIL, MoveOrCapture)` for the bishop | | | 369 | | | |
| `leap(KNIGHT, MoveOnly)` for the queen | | | | | 978 | |
| `leap(KNIGHT, CaptureOnly)` for the queen | 104 | | | | 1191 | |
| `slide(KNIGHT, MoveOrCapture).max_steps(2)` for the knight | | 453 | | | | |
| `slide(FORWARD, MoveOrCapture)` for the bishop | | | 493 | | | |
| `slide(KING, MoveOrCapture).max_steps(2)` for the king | | | | | | 642 |

The rows after the six flags add one atom to one kind. A more valuable queen also makes the pawn more valuable, because the pawn can promote to the queen.

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

The score is at most `EVAL_LIMIT` (`MATE_BOUND - 1`) in each direction, thus a score of the evaluation is never the score of a forced win, also with very strong kinds.

### Search

The search has iterative deepening, negamax with alpha-beta and principal variation search, a transposition table with the Zobrist key, a move order (the table move, captures by victim and attacker with the values of the battle, two killer moves, history), late-move reductions, a check extension, and a quiescence search of captures and promotions.

The ends of a battle have these scores: a side with no legal move loses (`-MATE + ply`), a rout is a loss with the same score, bare kings and the limit of the clock are draws. Each node tests the ends before it looks at its depth, in the order of `outcome`; the quiescence search does too. The game has no rule for a repeated position, thus the search has none.

The quiescence search does not stand pat while the side to move is in check: it searches each capture and promotion of that side, and two of its legal quiet moves (`QUIET_EVASIONS`). It searches more quiet moves only while each move so far is a loss, thus it still finds a mate. With no legal move the side loses. `movegen::evasion_moves` gives the moves that can end the check: the king moves, and the moves of the other pieces to a square of `movegen::evasion_squares` (the squares of the pieces that give the check, and for a slide the squares between it and the king). A side with 3 men or fewer that is not in check also loses there if it has no legal move. Thus the search sees a mate or a stalemate after a capture at its horizon.

After a move inside the search, the search calls `in_check` only if `movegen::may_give_check` is true: the moved piece can attack the king from its `to` square on an empty board (`SideTables::attack_zone`), or the move leaves a square of a line toward the king (`SideTables::slide_zone`). This is valid because the side to move was not in check before the move; it is not valid for the moves of the root.

The capture of a king is possible only from a state where the side that does not have the move is in check, thus only at the root. A legal capture of the royal king (the king on the lowest square) at the root wins with the score of a mate. A capture of the king that leaves the own king in check is not legal, and the search never gives it. Inside the search, the move before each node is legal, thus no move can capture the royal king there.

The search always completes depth 1 for each root move, also when the node limit is smaller than the nodes that depth 1 needs. The node limit applies from depth 2. Thus the result is never a move that the search did not look at. The quiescence search has no depth limit, thus depth 1 can need many nodes in a position with many captures: in 1000 positions of random games (a third with the six flags for each side), the median is about 200 nodes, and the largest number is about 540 000 (about 0.13 s).

Null-move pruning is in the code and is off: it did not help in self-play.

### Levels

`Level::LADDER` has one level for each floor. `Level::floor(n)` gives the level of floor `n`. A level has a depth limit, a node limit (from depth 2, see "Search"), and noise. With noise, each root move gets a random bonus from 0 to `noise_cp` centipawns. The bonus comes from the seed and the move.

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

`Player::reference()` in `tools/src/lib.rs` is the reference AI (`tools/src/reference.rs`) at depth 2 with no noise. It is a baseline for the self-play tool and for `ai speed` only.

### Self-play

`arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N] [--seed N] [--max-plies N] [--threads N]` plays two configurations against each other and prints the wins, the draws, the losses, the score, and a 95% interval.

- A CONFIG is `floor1` to `floor8`, `reference`, or `nodes=N`, and then options with `,` between them: `eval=derived|fixed|blind`, `noise=CP`, `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`, `threats=0|1`. The reference AI reads only `noise` and `depth`.
- `eval=fixed` has the usual piece values of chess for each side. `eval=blind` has an evaluation that knows only the rules of chess.
- The starts are the start position of chess and `--positions` armies in the style of the game: the base army of the player plus recruits against the enemy army of floor 3 to 8 with the same total value.
- The rule sets are: ordinary chess, each flag for White only, each flag for Black only, each flag for the two sides, and eight mixed combinations.
- Each start is played two times from the same position and rules: A as White against B, then B as White against A.
- A game that gets to `--max-plies` half moves (300) is a draw.

The same arguments give the same output.

## Game layer and command server

The crate `game/` (`chrogue-game`) has the roguelite layer: runs, relics, upgrades, offers, floors, battles, and saved data. The crate `server/` has the binary `chrogue-core`. A client, a test, or an agent plays the full game through one JSON protocol with no interface. `PROTOCOL.md` documents the protocol.

The crate `embed/` (`chrogue-embed`) gives the same protocol as a C interface. A client loads this library into its own process and needs no socket. The WebAssembly build of the game (`web`) links it into LÖVE.

### Commands

Run the commands from the `core/` directory, unless the command shows a different directory.

- Server on stdio: `cargo run --release --bin chrogue-core -- --stdio --no-save`
- Server on TCP: `CHROGUE_TOKEN=$(openssl rand -hex 32) cargo run --release --bin chrogue-core -- --listen 127.0.0.1:0 --save-dir /path/to/saves`. The first line of a client is `{"auth":"<the token>"}` (see "Authentication" in `PROTOCOL.md`). Exit codes: 2 for a usage error or a bad token, 3 if another core holds the lock of the save directory.
- Library for a client: `cargo build --release -p chrogue-embed`. The result is `target/release/libchrogue_core.so` (`.dylib` on macOS, `chrogue_core.dll` on Windows) and the static library `libchrogue_core.a`.
- Library for the WebAssembly build: `cargo build --release -p chrogue-embed --target wasm32-unknown-emscripten`. `web/build.sh` runs this command.
- Tests of the game layer and of the server: `cargo test --release -p chrogue-game -p chrogue-server`. With `-- --nocapture`, the fuzz test prints the error codes and the TCP test prints the round-trip times.

### Structure

- `game/src/content.rs`: The relics, upgrades, floors, enemy kinds, prices, and constants as data.
- `game/src/run.rs`: `Meta`, `Run`, `Unit`, `Enemy`, `Offer`, the enemy of each floor, rewards, the shop, and upgrades.
- `game/src/random.rs`: The dice, and the streams of random numbers of a run.
- `game/src/tuning.rs`: The tuning: the debug settings of a session. It has the fixed seed of new runs, the barred relics, the budget, the traits, and the AI level of each floor, and the cap, the weight, and the first floor of each kind in an enemy army. Its defaults come from the content.
- `game/src/battle.rs`: One battle on the engine: relic effects, gold, lost and rescued units, the reward, and `settle`.
- `game/src/save.rs`: The `Storage` trait, a file storage (with the lock of the directory and safe writes) and a memory storage, and the check of saved data. A file that the core cannot use is set aside as `<name>.bad-<unix time>`, never written over.
- `game/src/session.rs`: `Screen`, `Session::command`, and the commands.
- `game/src/view.rs`: The views of the screens and the content tables of `hello`.
- `game/src/protocol.rs`: The names of the commands, events, and error codes.
- `game/src/chess.rs`: The one module that calls the engine. A change of the engine API changes only this file.
- `game/tests/`: The game layer (`game.rs`), saved data (`saved.rs`), the tuning (`tuning.rs`), random requests (`fuzz.rs`), and the check of `PROTOCOL.md` (`protocol_doc.rs`).
- `server/src/main.rs`: The line transport: the command line, the auth line, the takeover by a newer client, the timeouts, and the idle exit. `server/tests/tcp.rs` starts the binary and tests TCP, stdio, the lock, and the command line.
- `embed/include/chrogue_core.h`: The C interface: `chrogue_open`, `chrogue_open_error`, `chrogue_command`, and `chrogue_close`.
- `embed/src/lib.rs`: The functions of the C interface. They move text to `Session::command` and back, as the server does for a socket. `embed/tests/c_interface.rs` calls the functions as a client does.

### Relics as data

A relic has movement rules and an effect. The movement rules are a list of `RuleEdit`: edits of `SideRules::standard()` that the engine and the AI read. The effect is one kind of `Effect`, at a fixed point of a battle.

To add a relic, add one entry to `RELICS` in `game/src/content.rs`. A new kind of effect needs a kind in `Effect` and code in `battle.rs`.

### Details of the behavior of the game layer

- The AI of floor `n` is the level of the floor in the tuning. The default is `Level::floor(n)` of the engine. `debug_tune` gives a floor another level of `Level::LADDER`.
- The enemy army of a floor comes from the tuning too: the budget and the number of traits of the floor, and the cap, the weight, and the first floor of each kind. The defaults are `FLOORS` and `ENEMY_KINDS` in `game/src/content.rs`.
- A run has a seed from 0 to 999999999. Each random result of the run has dice of its own (`Dice::stream`) from the seed, the kind of the result, and two numbers: the enemy of a floor (the floor), the reward before a floor (the floor), the shop before a floor (the floor and the number of rerolls), and the move of the AI (the floor and the number of moves that the battle played). Thus the same run seed gives the same armies, rewards, and shop items, also when the battles have different numbers of moves. The dice of the session (`--seed`) make only the seed of each new run.
- Saved data: a unit id is at most 19999 and appears one time, the seed of a run is at most 999999999, a relic id appears one time in a list, a count is at most 2^53 - 1, and the board must be valid (no two pieces on one square, and one king on each side). A file that the core cannot use is kept as `<name>.bad-<unix time>`. The comment of `game/src/save.rs` has the list.
- The pawn of Conscription does not go to the square of an enemy piece (only a debug enemy can be on rank 2 or 3).
- The debug commands refuse two pieces on one square and a side with no king or two kings (see `PROTOCOL.md`).
