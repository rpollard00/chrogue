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
- Self-play match: `cargo run --release --bin arena -- --a level11 --b reference`. See "Self-play" below.
- Mistakes of the levels: `cargo run --release --bin levels -- mistakes level1 level2 level3 level4`. See "Measurements of the levels" below.
- A run floor by floor: `cargo run --release --bin levels -- floors --player level2 level1 level1 level1 level2 level2 level2 level2 level3`
- Relics and armies in battles: `cargo run --release --bin balance -- --player 2-4 --out balance.json`. See "Measurements of relics and armies" below.

## Structure

- `engine/`: The library crate `chrogue-engine`. It has no dependencies.
  - `src/types.rs`: Squares, colors, kinds, pieces, moves, and the move list. The move list holds 384 moves without the heap. A list with more moves puts its moves on the heap.
  - `src/rules.rs`: The movement rules as data.
  - `src/tables.rs`: The lookup tables that come from the rules. The engine builds them one time for each battle.
  - `src/movegen.rs`: The move generation and the attack detection. A kind with one plain group of atoms (see below) takes its leaps and slides as bitboards. Another kind (the pawn) takes a list of probes for each square that the tables compute from its atoms. A piece that can capture en passant goes group by group. The three paths give the same moves.
  - `src/state.rs`: The state of a battle, `make`, and `unmake`. Only `make`, `unmake`, and the null move change the side to move and the en passant square after construction.
  - `src/outcome.rs`: The result of a battle, and `wins_in`: a test for a forced win in a small number of moves.
  - `src/perft.rs`: The count of move sequences. The tests compare it with the known counts of chess positions.
  - `src/zobrist.rs`: The Zobrist keys. `State::key` gives the key of a state.
  - `src/eval.rs`: The piece values that come from the rules, and the evaluation of a position.
  - `src/search.rs`: The search.
  - `src/level.rs`: The levels of the AI and `choose_move`.
  - `src/rng.rs`: A small seeded random number generator.
  - `src/fen.rs`: A reader for the piece field of a FEN string. The tests and the tools use it.
  - `tests/`: Perft counts, the six rule flags and the results of a battle (`relic_rules.rs`), other rules as data (`data_rules.rs` for the officers, `data_moves.rs` for pawns, en passant, castles, ranges, and first-move atoms), the Zobrist key, the derived values, and the tactics of the AI. `tests/property.rs` compares the engine with a naive move generator on 6000 random rule sets: random atoms for each kind (the pawn too) with random ranges, conditions, en passant properties, clock properties, promotions, hooks, and castle rows. It also walks the move tree of 1500 more rule sets, compares each `make` with a naive `make`, and makes sure that `unmake` gives back the state and the key.
- `tools/`: The crate `chrogue-tools`. It has the binaries `perft` (a timer), `arena` (self-play matches), `levels` (the mistakes of a level, and a run floor by floor), `ai` (values, speed, and the move for one position), and `balance` (relics and armies in battles of the game). Its library has `src/reference.rs` (the reference AI), `Player` (a level of the engine or the reference AI, and the reader of a CONFIG), `src/armies.rs` (armies in the style of the game), `src/balance.rs` (the battles of `balance`), and `src/cli.rs` (the command line of the binaries). `balance` uses the crate `chrogue-game`. The other binaries use only the engine. `tools/report/` makes an HTML report from the data of `balance`. The reference AI is the algorithm of the AI that the first version of the game had. It is a baseline opponent only. The engine and the game do not use it.

## Movement rules

Each side of a battle has its own `SideRules`. `Rules` holds the rules of White and of Black. A `SideRules` has a `KindRules` for each kind (pawn, knight, bishop, rook, queen, king), and a list of castles.

### Atoms

Each kind, the pawn too, moves by a list of atoms. An `Atom` has these fields:

- `offsets`: Steps of (file, rank) from the view of White. For Black, the engine mirrors the rank step. Thus (0, 1) is one square forward for each side.
- `max_steps`: The piece goes 1 to `max_steps` steps along an offset. Each step must end on an empty square, except the last step, which can capture. 1 is a leap: pieces inside one step do not block it. `Atom::MAX_STEPS` (7) is a slide that stops only at a piece or at the edge. A number between them is a slide with a range.
- `mode`: What the atom can do on its target square. `Mode::MoveOrCapture`, `Mode::MoveOnly` (the atom attacks no square, thus it does not give check), or `Mode::CaptureOnly` (the atom attacks its squares, thus it gives check).
- `condition`: When the piece can use the atom. An atom also attacks only while its condition is true.
  - `Condition::Always`.
  - `Condition::Unmoved`: only while the piece has not moved.
  - `Condition::Near { kind, range }`: only while another piece of the same side and of this kind is `range` squares away or less, as a king counts squares. The range is from 1 to 7. Such an atom has no en passant property (`RulesError::BadCondition`). A move of the other piece can give or end a check.
- `makes_en_passant`: The squares that a move of the atom passes become the en passant squares of the next half move, and the piece that moved is the victim of an en passant capture there. A move of one step passes no square. An atom of the king cannot have it (`RulesError::KingMakesEnPassant`): an en passant capture never removes a king.
- `captures_en_passant`: The atom can go to an en passant square as if the square has the victim, and the victim is captured. The atom must be able to capture.

`Atom::leap(offsets, mode)` and `Atom::slide(dirs, mode)` make an atom with no condition and no property. The methods `max_steps(n)`, `if_unmoved()`, `if_near(kind, range)`, `makes_en_passant()`, and `captures_en_passant()` add the others.

### The moves of a piece

The atoms of a kind with the same condition and the same three properties form one group. The groups come in the order of their first atom. The engine gives the moves of a piece group by group:

- A group gives its targets in ascending order, then its en passant captures.
- A target that an earlier group gave is not given again. Thus the first group decides the properties of a move.
- An en passant capture takes the place of a quiet move to the same square.
- A target is a `Special::DoubleStep` move if a slide of a group with `makes_en_passant` gives it after one square or more. Its en passant squares are the squares that it passes on each such slide of the kind that can be used.

### Hooks

A kind can also have hooks (`KindRules::hooks`). A `Hook` is a slide that turns: the piece goes `min_leg` to `max_leg` squares along a leg, and then one last step in another direction. Each square of the leg must be empty. The last step moves or captures by the `mode` of the hook. A hook has these fields:

- `bends`: Pairs of (the direction of the leg, the last step), from the view of White. For Black, the engine mirrors the rank steps.
- `min_leg`, `max_leg`: The number of squares of the leg, from 1 to `Atom::MAX_STEPS`.
- `mode`: What the last step can do on its target square.

`Hook::right_angle(dirs, min_leg, mode)` makes a hook with a leg along each direction and a last step to the left or to the right of the leg. With the directions of the rook and a `min_leg` of 2, the hook is a knight move with a long leg that a piece can block.

- A hook gives a target that no atom of the kind gives. The move has no special property.
- A hook that can capture attacks its targets, thus it gives check. A piece on a square of the leg stops the check.
- Only a kind with no promotion, and with atoms that have no condition and no en passant property, can have a hook (`RulesError::BadHook`).

### Shields

A side can have shields (`SideRules::shields`). A `Shield` has a `protector` kind, a `protected` kind, and a `range` from 1 to 7. The enemy cannot capture a piece of the protected kind while another piece of its side of the protector kind is `range` squares away or less, as a king counts squares. This is also true for an en passant capture.

- The king cannot be the protected kind (`RulesError::BadShield`).
- A shield does not change the attacked squares. `is_attacked` is true for the square of a piece with a shield that an enemy piece could capture with no shield. Thus a piece gives check through a piece with a shield, and a king cannot go next to such a piece on a square that the piece attacks.
- `movegen::shielded` gives the pieces of a side that have a shield. The threat term of the evaluation does not count such a piece.

### Auras

A shield and an atom with `Condition::Near` have the same shape: a piece gets something while another piece of its side is near it. `SideRules::auras` gives the two as one list, for a client that shows which pieces have such a rule now. An `Aura` has a `boon` (`Boon::Shield`, or `Boon::Moves` for the atoms), a `source` kind, the target kinds, and a `range`. Rules with the same boon, source, and range are one aura.

`movegen::aura_holders` gives the pieces of a side that have the boon of an aura now. `movegen::aura_zone` gives the squares in the range of a source piece. The move generation and the evaluation do not read the auras. A property test makes sure that the auras agree with `shielded` and `condition_holds`.

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

1. Write the rule as an edit of `SideRules`: atoms, hooks, shields, a promotion, or castles. Do not change `tables.rs` or `movegen.rs`.
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
- Formation: for each atom with `Condition::Near` and each `Shield` of a side, a bonus for each piece that is near its piece of the rule. The row of an atom pays `NEAR_SHARE = 1/4` of the difference between the value of the kind with the atom always on and the value of the kind without the atom, and one row pays at most half of the value of the kind. This limit is for each row of an atom, not for the sum of the rows, and the row of a shield has no limit. Each distinct condition of a kind has its own row, and the rows add. The row of a shield pays `SHIELD_SHARE = 1/4` of the value of the protected kind. A piece that is one square too far gets 8/16 of the bonus, and a piece that is two squares too far gets 4/16 (`RING_16THS`). Thus the search moves a piece toward its formation. The mobility term pays for the squares that the atom gives now; this term pays for the formation also when the lines are not open. A side with no such rule has no row, and its score does not change.
- Rout: a penalty that grows when the material of a side gets small. Thus the side that is ahead wants trades.
- Clock: from 70 half moves on the clock, the score goes linearly to 0 at 100.

The score is at most `EVAL_LIMIT` (`MATE_BOUND - 1`) in each direction, thus a score of the evaluation is never the score of a forced win, also with very strong kinds.

### Search

The search has iterative deepening, negamax with alpha-beta and principal variation search, a transposition table with the Zobrist key, a move order (the table move, captures by victim and attacker with the values of the battle, two killer moves, history), late-move reductions, a check extension, and a quiescence search of captures and promotions.

The ends of a battle have these scores: a side with no legal move loses (`-MATE + ply`), a rout is a loss with the same score, bare kings and the limit of the clock are draws. Each node tests the ends before it looks at its depth, in the order of `outcome`; the quiescence search does too. The game has no rule for a repeated position, thus the search has none.

The quiescence search does not stand pat while the side to move is in check: it searches each capture and promotion of that side, and two of its legal quiet moves (`QUIET_EVASIONS`). It searches more quiet moves only while each move so far is a loss, thus it still finds a mate. With no legal move the side loses. A quiet move out of check can give check, thus a line of such moves can have no end. One line of the quiescence search has 6 quiet moves out of check at most (`QUIET_EVASION_LINE`). After these, the search handles a side in check as a side that is not in check. For the same reason, the check extension applies only in the first `2 * depth + 8` half moves of a line (`CHECK_EXTENSION_PLIES`), where `depth` is the depth of the iteration. `movegen::evasion_moves` gives the moves that can end the check: the king moves, and the moves of the other pieces to a square of `movegen::evasion_squares` (the squares of the pieces that give the check, and for a slide the squares between it and the king). A side with 3 men or fewer that is not in check also loses there if it has no legal move. Thus the search sees a mate or a stalemate after a capture at its horizon.

After a move inside the search, the search calls `in_check` only if `movegen::may_give_check` is true: the moved piece can attack the king from its `to` square on an empty board (`SideTables::attack_zone`), or the move leaves a square of a line toward the king (`SideTables::slide_zone`). This is valid because the side to move was not in check before the move; it is not valid for the moves of the root.

The capture of a king is possible only from a state where the side that does not have the move is in check, thus only at the root. A legal capture of the royal king (the king on the lowest square) at the root wins with the score of a mate. A capture of the king that leaves the own king in check is not legal, and the search never gives it. Inside the search, the move before each node is legal, thus no move can capture the royal king there.

The search always completes depth 1 for each root move, also when the node limit is smaller than the nodes that depth 1 needs. The node limit applies from depth 2. Thus the result is never a move that the search did not look at. The quiescence search has no depth limit other than these limits, thus depth 1 can need many nodes in a position with many captures: in 1000 positions of random games (a third with the six flags for each side), the median is about 200 nodes, and the largest number is about 540 000 (about 0.13 s).

Null-move pruning is in the code and is off: it did not help in self-play.

### Levels

`Level::LADDER` has the levels of the AI, from the weakest to the strongest. `Level::number(n)` gives level `n`. `FLOORS` in `game/src/content.rs` gives each floor its level. A level has a depth limit, a node limit (from depth 2, see "Search"), and flaws (`Flaws`).

A flaw makes the search play worse on purpose. Each random number of a flaw comes from the seed of the search, thus the same seed gives the same flaws.

- Noise (`noise_cp`): Each root move gets a random bonus from 0 to `noise_cp` centipawns. The search selects the move with the best sum of score and bonus. The bonus comes from the seed and the move.
- Overlook (`overlook`): Each legal root move has this chance in percent that the search does not see it. The search selects from the moves that it sees. If it sees no move, it sees each move. The search always sees a legal capture of the royal king.
- Careless (`careless`): Each search has this chance in percent that it is careless. A careless search gives each root move the score of the position right after the move. That score is the score of an end of the battle (a rout, bare kings, or the clock), or the evaluation. A careless search does not look at the reply of the opponent. It does one iteration, and the depth limit and the node limit do not apply. The noise and the overlook apply.

A careless search does not see that a move gives checkmate, unless the clock is at its limit. Only there, the score of the position tests if the opponent has a legal move.

The first three levels are for a player who knows only how the pieces move. They look one half move ahead, as level 4 does, and they have overlook and careless. Thus they miss some captures, and they leave pieces where the opponent can capture them. From level 4, a level has no overlook and is not careless.

| Level | Name | Depth limit | Node limit | Noise | Overlook | Careless |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Recruit | 1 | 300 | 150 | 40 | 50 |
| 2 | Cadet | 1 | 300 | 150 | 25 | 25 |
| 3 | Private | 1 | 300 | 150 | 12 | 10 |
| 4 | Corporal | 1 | 300 | 150 | 0 | 0 |
| 5 | Sergeant | 2 | 1 000 | 90 | 0 | 0 |
| 6 | Lieutenant | 3 | 2 500 | 50 | 0 | 0 |
| 7 | Captain | none | 6 000 | 25 | 0 | 0 |
| 8 | Major | none | 15 000 | 12 | 0 | 0 |
| 9 | Colonel | none | 36 000 | 6 | 0 | 0 |
| 10 | General | none | 100 000 | 0 | 0 | 0 |
| 11 | Marshal | none | 320 000 | 0 | 0 | 0 |

`Player::reference()` in `tools/src/lib.rs` is the reference AI (`tools/src/reference.rs`) at depth 2 with no noise. It is a baseline for the self-play tool and for `ai speed` only.

### Self-play

`arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N] [--seed N] [--max-plies N] [--threads N]` plays two configurations against each other and prints the wins, the draws, the losses, the score, and a 95% interval.

- A CONFIG is `levelN` (level `N` of the ladder, where `level1` is the weakest), `reference`, or `nodes=N`, and then options with `,` between them: `eval=derived|fixed|blind`, `noise=CP`, `overlook=N` and `careless=N` (percent), `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`, `threats=0|1`, `formation=0|1`. The reference AI reads only `noise` and `depth`.
- `eval=fixed` has the usual piece values of chess for each side. `eval=blind` has an evaluation that knows only the rules of chess.
- The starts are the start position of chess and `--positions` armies in the style of the game: the base army of the player plus recruits against the enemy army of floor 3 to 8 with the same total value.
- The rule sets are: ordinary chess, each flag for White only, each flag for Black only, each flag for the two sides, and eight mixed combinations.
- Each start is played two times from the same position and rules: A as White against B, then B as White against A.
- A game that gets to `--max-plies` half moves (300) is a draw.

The same arguments give the same output.

### Measurements of the levels

`levels` measures a level in battles between armies in the style of the game, with the rules of ordinary chess. A CONFIG is that of `arena`. The same arguments give the same output.

`levels mistakes [--white CONFIG] [--games N] [--judge NODES] [--max-plies N] [--seed N] [--threads N] CONFIG...` counts the mistakes of each CONFIG.

- The CONFIG plays Black, the enemy, in `--games` battles (100) against `--white` (`nodes=20000,noise=40`). The battles are on the armies of floors 2 to 6. Each CONFIG gets the same battles.
- The judge is a search with `--judge` nodes (30000) and no flaw. It gives a score to the position before and after each move of Black.
- `moves`: the moves that the judge scored. A move has no score if the judge sees a forced win or a forced loss before it, or if the move ends the battle.
- `>=100`, `>=300`: the part of the moves that are this number of centipawns, or more, below the best move.
- `hung`: the move is 250 centipawns or more below the best move, and the best reply captures an officer.
- `lost`: after the move, the judge sees a forced win of White.
- `not taken`: the best move captures an officer, and the move is 250 centipawns or more below it. The column also gives the number of such best moves.
- `wins`, `draws`: the battles that Black won, and the draws.
- A weak White (for example `--white level4,noise=600`) leaves more officers where Black can capture them, thus `not taken` has more data.

`levels floors --player CONFIG [--games N] [--max-plies N] [--seed N] [--threads N]` and then 8 CONFIGs plays a run floor by floor. `--player` plays White in `--games` battles (100) on the armies of each floor, against the CONFIG of that floor. The output has the wins, the draws, and the losses of the player on each floor.

- From floor 3, the army of the player has the value of the enemy army. On floors 1 and 2, the player has the base army, which has more value.
- The battles have no relics and no boss traits, and the gold of a run does not set the army of the player. Thus the numbers compare levels. They do not tell if a person wins a run.

### Measurements of relics and armies

`balance` plays battles of the game (`Battle` of `chrogue-game`). Thus a battle has the relics of the player, the traits and the formation of the enemy, and the rewards. An AI level plays each side. The player is White.

`balance [--floor LIST] [--player LIST] [--enemy LIST] [--army LIST] [--traits LIST] [--relics LIST] [--games N] [--gold N] [--seed N] [--max-plies N] [--threads N] [--out FILE]` plays `--games` battles (40) for each combination of the six lists. A combination is a cell. A list has its values with `,` between them. A list of numbers can have ranges, such as `1-3,8`.

- `--floor`: The floors, from 1 to 8. The default is `1-8`.
- `--player`: The AI levels of the player, from 1 to 11. The default is `3`.
- `--enemy`: The AI levels of the enemy. `floor` is the level of the floor in `FLOORS`. The default is `floor`.
- `--army`: The armies of the player. The default is `auto`.
  - `auto`: The base army plus random recruits with the weights of `RECRUITS`. The value of the army is the budget of the floor, or the value of the base army (12) if the budget is less.
  - `auto+N`, `auto-N`: An `auto` army with `N` more or less value. The army is not smaller than the base army. An army of 16 units can have less than the value.
  - Letters, such as `KRNPPPPBB`: These kinds. The king is on e1, and each other unit gets the next free home square.
- `--traits`: The traits of the enemy. `floor` gives the traits that the game gives to the enemy of the floor. `none` gives no traits. Relic keys with `+` between them give these traits on each floor. The default is `floor`.
- `--relics`: The relic sets of the player. The default is `none,each`.
  - `none`: No relics.
  - Relic keys with `+` between them, such as `gallop+longLeap`: One set.
  - `each` in the place of a key: One set for each relic. Thus `each` gives each relic alone, and `gallop+each` gives Gallop with each other relic.
  - `pairs`: Each set of two relics.
- `--gold`: The gold of the run before each battle (0). Interest reads it.
- A battle that gets to `--max-plies` half moves (300) is a draw with no reward.

The number of battles is the product of the sizes of the six lists and `--games`. The tool prints this number before it starts. A battle between two of the first four levels takes some milliseconds on one processor. A battle between two of level 9 takes some seconds.

Battle `i` of each cell has the same run seed. Thus two cells with the same floor and the same army have the same units and the same enemy kinds in battle `i`, and the difference between two relic sets is a difference battle by battle. The squares of the enemy can differ: the formation reads the movement rules of the two sides and the pieces of the player. The same arguments give the same data.

The tool prints one row for each relic set: the wins, the draws, and the losses of the player, the difference of the win rate to the first relic set with its 95% interval (from the mean difference of each run seed), the mean gold reward, and the mean value of the units that the player lost.

- An AI level is not a person. The numbers compare relics and armies for each pair of levels.
- The AI does not play for gold. A relic that gives gold shows its gold, not a different way to play.
- A battle has no run before it. The gold, the shop, and the rewards of a run do not set the army or the relics of the player.

#### The data

`--out` (`balance.json`) is one JSON object:

- `format` (`chrogue-balance`), `version` (1), `command`, `seed`, `games`, `max_plies`, `gold`.
- `content`: The relics (`key`, `name`, `text`, `kind`: `rule` or `effect`, `trait`), the levels (`number`, `name`), and the floors (`number`, `name`, `level`, `budget`, `traits`, `boss`) of the game.
- `axes`: The values of the six lists: `floors`, `players`, `enemies` (a number or `floor`), `armies` and `traits` (the text of each value), and `relics` (a list of relic keys for each set).
- `cells`: One object for each cell. `floor`, `player`, `enemy`, `army`, `traits`, and `relics` are indexes into `axes`. The other fields have one item for each battle, in the order of the battles:
  - `results`: A text with `w` (the player won), `d` (a draw), or `l` (the player lost).
  - `ends`: A text with `m` (checkmate), `s` (stalemate), `r` (rout), `b` (bare kings), `c` (the clock), or `x` (`max_plies`).
  - `plies`: The half moves.
  - `gold`: The gold reward.
  - `lost`: The piece value of the units that the enemy captured and that did not return.
  - `recruits` is one number: the units that the relics added to the army after the battles of the cell.

#### The report

`scripts/balance.sh` in the root of the repository runs `balance` and then makes the report (`README.md`, "Measure relics and armies"). For a file of data that you have, `tools/report/` makes the HTML file: `bun install` one time, then `bun run report balance.json` in `tools/report/`. The file has the data in it and opens from the disk. `tools/report/README.md` describes its views, its statistics, and the TypeScript module that reads the data in a script.

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
- `game/src/formation.rs`: The squares of the enemy army at the start of a battle.
- `game/src/save.rs`: The `Storage` trait, a file storage (with the lock of the directory and safe writes) and a memory storage, and the check of saved data. A file that the core cannot use is set aside as `<name>.bad-<unix time>`, never written over.
- `game/src/session.rs`: `Screen`, `Session::command`, and the commands.
- `game/src/view.rs`: The views of the screens and the content tables of `hello`.
- `game/src/protocol.rs`: The names of the commands, events, and error codes.
- `game/src/chess.rs`: The one module that calls the engine. A change of the engine API changes only this file.
- `game/tests/`: The game layer (`game.rs`), the relics (`relics.rs`), the upgrades (`upgrades.rs`), the formations of the enemy (`formation.rs`), saved data (`saved.rs`), the tuning (`tuning.rs`), random requests (`fuzz.rs`), and the check of `PROTOCOL.md` (`protocol_doc.rs`).
- `server/src/main.rs`: The line transport: the command line, the auth line, the takeover by a newer client, the timeouts, and the idle exit. `server/tests/tcp.rs` starts the binary and tests TCP, stdio, the lock, and the command line.
- `embed/include/chrogue_core.h`: The C interface: `chrogue_open`, `chrogue_open_error`, `chrogue_command`, and `chrogue_close`.
- `embed/src/lib.rs`: The functions of the C interface. They move text to `Session::command` and back, as the server does for a socket. `embed/tests/c_interface.rs` calls the functions as a client does.

### Relics as data

A relic has movement rules and an effect. The movement rules are a list of `RuleEdit`: edits of `SideRules::standard()` that the engine and the AI read. The effect is one kind of `Effect`, at a fixed point of a battle.

`RuleEdit::Leap` and `RuleEdit::Slide` add one atom to one kind: a leap, or a slide of 1 to `steps` steps. `RuleEdit::Hook` adds one hook to one kind: a line of `min_leg` or more empty squares, and then one square to the side. `RuleEdit::SlideNear` adds a slide that a piece has only near another piece of its side, and `RuleEdit::Shield` adds a shield. Thus a relic with such a rule needs no code. A relic with a text for the enemy (`foe_text`) can be a boss trait.

The effects have these points of a battle:

- The start: `ExtraPawn`.
- A capture by the player: `CaptureGold`.
- A capture by the enemy: `RescueFirst`.
- The end: `VictoryGold`, `CheckmateGold`, `LossGold`, and `PromotionRecruit`. These effects run in the order of the relics of the run.

To add a relic, add one entry to `RELICS` in `game/src/content.rs`. A new kind of effect needs a kind in `Effect` and code in `battle.rs`.

### Details of the behavior of the game layer

- The AI of a floor is the level of the floor in the tuning. The default is the `level` of the floor in `FLOORS`. `debug_tune` gives a floor another level of `Level::LADDER`. The default levels are the three beginner levels: level 1 on floors 1 to 3, level 2 on floors 4 to 7, and level 3 on floor 8.
- The enemy army of a floor comes from the tuning too: the budget and the number of traits of the floor, and the cap, the weight, and the first floor of each kind. The defaults are `FLOORS` and `ENEMY_KINDS` in `game/src/content.rs`.
- The enemy of a floor has kinds and no squares (`EnemyPieces::Kinds`). `formation::place` gives the squares at the start of the battle, against the army of the player: the engine refuses a formation with the enemy king in check, with a win of the player on the first move, or with a forced win in two moves (`Flaw`). The check uses the rules of the relics and of the traits, thus a new movement rule needs no code there. `PROTOCOL.md` has the details. An enemy of `debug_set_enemy` keeps its squares (`EnemyPieces::Placed`).
- A run has a seed from 0 to 999999999. Each random result of the run has dice of its own (`Dice::stream`) from the seed, the kind of the result, and two numbers: the enemy of a floor (the floor), the formation of that enemy (the floor), the reward before a floor (the floor), the shop before a floor (the floor and the number of rerolls), the relic of Heirloom at the start of the run, and the move of the AI (the floor and the number of moves that the battle played). Thus the same run seed gives the same armies, rewards, and shop items, also when the battles have different numbers of moves. The dice of the session (`--seed`) make only the seed of each new run.
- Heirloom gives one relic of the relics that are not barred. Antiquary takes its gold from the price of a relic first (the price stays 1 gold or more), then the price gets the factor of Haggler.
- Envoy gives a reward after a draw, but not after a draw on the last floor. The run stays on that floor, and each camp before a floor has the same reward.
- Muster gives a pawn with a unit that the player takes as a reward, if the army has space after the unit. A relic card, a gold card, and a shop item give no pawn.
- Curator puts its card first in a reward: one of the two relics of the reward. The other cards come from the items that remain, by weight. The card comes also when each relic slot of the run has a relic, because the player can discard a relic and then take the card. Curator guarantees no card only if no relic remains for the run.
- A run has relic slots (`Run.slots`). A new run gets them from the tuning, and the default is `RELIC_SLOTS` (4). A relic offer to a run with a relic in each slot is blocked (`relics_full`). Heirloom gives no relic to a run with 0 slots.
- `discard_relic` removes a relic from the run in the camp, at no cost and with no gold. The reward can be open. The game can offer the relic again.
- `RELICS_MAX` (10) is the most slots of a run: the relic fan of the client has space for 10 medals. `debug_set_relic_slots` gives a run 0 to 10 slots and removes no relic. `debug_set_relic` refuses a relic only for a run with 10 relics. Thus a debug session can give a run more relics than slots, and each relic offer is then blocked until the player discards. Saved data keeps the first 10 relics.
- Interest counts the gold of the run, the captures, the gold of the floor, and the bonuses of the relics before it in the run.
- Apprenticeship gives its pawn for a unit only: a pawn of the army that promoted in the battle and is on the board at the end. The pawn of Conscription is not a unit. A full army gets no pawn.
- Gambit counts each piece that the enemy captured, also a unit that Second Wind returns and the pawn of Conscription. It gives its gold after a win and after a draw.
- Saved data: a unit id is at most 19999 and appears one time, the seed of a run is at most 999999999, a relic id appears one time in a list, a count is at most 2^53 - 1, and the board must be valid (no two pieces on one square, and one king on each side). A file that the core cannot use is kept as `<name>.bad-<unix time>`. The comment of `game/src/save.rs` has the list.
- The pawn of Conscription does not go to the square of an enemy piece (only a debug enemy can be on rank 2 or 3).
- The debug commands refuse two pieces on one square and a side with no king or two kings (see `PROTOCOL.md`).
