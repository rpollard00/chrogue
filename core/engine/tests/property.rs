//! Random rule sets against a naive move generator.
//!
//! The naive generator in this file reads the rules data directly. It walks the offsets square
//! by square and uses no table of the engine. For each random rule set and position, the test
//! compares the pseudo moves of the officers, the castles, and the attacked squares of the two
//! colors. A second test walks the move tree with pawns and castling, and makes sure that
//! `unmake` gives back the full state. The seeds are fixed, thus each run is the same.

use std::collections::BTreeSet;

use chrogue_engine::rng::Rng;
use chrogue_engine::zobrist::key_from_scratch;
use chrogue_engine::{
    Atom, Bitboard, Color, DoubleStep, Kind, Mode, Move, MoveList, Offset, Piece, Placement, Promotions, Rules,
    SideRules, Special, Square, State, is_attacked, pseudo_moves,
};

/// The number of rule sets of the comparison with the naive generator.
const RULE_SETS: usize = 6000;
/// The number of rule sets of the make and unmake walk.
const WALKS: usize = 1500;

const MODES: [Mode; 3] = [Mode::MoveOrCapture, Mode::MoveOnly, Mode::CaptureOnly];

/// A list of 1 to 5 offsets. The list can have the same offset two times.
fn random_offsets(rng: &mut Rng, max: i8) -> Vec<Offset> {
    let count = 1 + rng.below(5);
    let span = (2 * max + 1) as u64;
    let mut offsets = Vec::new();
    while (offsets.len() as u64) < count {
        let offset = (rng.below(span) as i8 - max, rng.below(span) as i8 - max);
        if offset != (0, 0) {
            offsets.push(offset);
        }
    }
    offsets
}

/// Rules with 0 to 3 random atoms for each officer. A leap has offsets of up to 3 squares, or
/// sometimes up to 7. A slide has steps of up to 2 squares, thus knight-step riders occur.
fn random_side(rng: &mut Rng) -> SideRules {
    let mut side = SideRules::standard().with_castling(rng.below(2) == 0);
    for kind in Kind::OFFICERS {
        let mut atoms = Vec::new();
        for _ in 0..rng.below(4) {
            let mode = MODES[rng.below(3) as usize];
            if rng.below(2) == 0 {
                let max = if rng.below(8) == 0 { 7 } else { 3 };
                atoms.push(Atom::Leap { offsets: random_offsets(rng, max), mode });
            } else {
                atoms.push(Atom::Slide { dirs: random_offsets(rng, 2), mode });
            }
        }
        // Most rule sets replace the movement of the kind. The others add to it.
        if rng.below(4) != 0 {
            side = side.with_kind(kind, atoms).unwrap();
        } else {
            for atom in atoms {
                side = side.with_atom(kind, atom).unwrap();
            }
        }
    }
    side
}

/// Random pawn rules and promotion kinds.
fn random_pawns(rng: &mut Rng, mut side: SideRules) -> SideRules {
    let mut kinds = [Kind::Queen, Kind::Knight, Kind::Rook, Kind::Bishop];
    rng.shuffle(&mut kinds);
    let count = 1 + rng.below(4) as usize;
    side.pawn.promotions = Promotions::new(&kinds[..count]).unwrap();
    side.pawn.promo_distance = rng.below(7) as u8;
    side.pawn.backward_step = rng.below(2) == 0;
    side.pawn.double_step = if rng.below(2) == 0 { DoubleStep::Always } else { DoubleStep::FirstMove };
    side
}

/// 4 to 23 random pieces on different squares. Some kings and rooks stand on their castle
/// squares, and some pawns can capture en passant.
fn random_placements(rng: &mut Rng) -> Vec<Placement> {
    let mut placements = Vec::new();
    let mut used: Bitboard = 0;
    let mut place = |placements: &mut Vec<Placement>, square: Square, kind: Kind, color: Color, moved: bool| {
        if used & (1 << square) == 0 {
            used |= 1 << square;
            let id = placements.len() as u16;
            placements.push(Placement { piece: Piece { id, kind, color, moved }, square });
        }
    };
    for (color, home) in [(Color::White, 4), (Color::Black, 60)] {
        if rng.below(2) == 0 {
            place(&mut placements, home, Kind::King, color, rng.below(4) == 0);
            for rook in [home - 4, home + 3] {
                if rng.below(3) != 0 {
                    place(&mut placements, rook, Kind::Rook, color, rng.below(4) == 0);
                }
            }
        }
    }
    // A pawn on its start rank and an enemy pawn that can capture it en passant after a double step.
    for (color, start, enemy) in [(Color::White, 8, 24), (Color::Black, 48, 32)] {
        if rng.below(2) == 0 {
            let file = 1 + rng.below(6) as Square;
            let side = if rng.below(2) == 0 { file - 1 } else { file + 1 };
            place(&mut placements, start + file, Kind::Pawn, color, false);
            place(&mut placements, enemy + side, Kind::Pawn, color.other(), true);
        }
    }
    for _ in 0..4 + rng.below(20) {
        let kind = Kind::ALL[rng.below(6) as usize];
        let color = Color::ALL[rng.below(2) as usize];
        place(&mut placements, rng.below(64) as Square, kind, color, rng.below(2) == 0);
    }
    placements
}

fn step(from: Square, (df, dr): Offset) -> Option<Square> {
    let file = (from % 8) as i8 + df;
    let rank = (from / 8) as i8 + dr;
    ((0..8).contains(&file) && (0..8).contains(&rank)).then(|| (rank * 8 + file) as Square)
}

/// The squares that an atom of a piece on `from` reaches: (square, the square has a piece).
/// A slide stops on the first piece.
fn reach(state: &State, from: Square, atom: &Atom, forward: i8) -> Vec<(Square, bool)> {
    let mut squares = Vec::new();
    match atom {
        Atom::Leap { offsets, .. } => {
            for &(df, dr) in offsets {
                if let Some(to) = step(from, (df, dr * forward)) {
                    squares.push((to, state.piece_at(to).is_some()));
                }
            }
        }
        Atom::Slide { dirs, .. } => {
            for &(df, dr) in dirs {
                let mut at = from;
                while let Some(to) = step(at, (df, dr * forward)) {
                    let occupied = state.piece_at(to).is_some();
                    squares.push((to, occupied));
                    if occupied {
                        break;
                    }
                    at = to;
                }
            }
        }
    }
    squares
}

fn mode(atom: &Atom) -> Mode {
    match atom {
        Atom::Leap { mode, .. } | Atom::Slide { mode, .. } => *mode,
    }
}

/// The squares that the pieces of a color attack.
fn naive_attacks(state: &State, color: Color) -> Bitboard {
    let rules = state.rules().side(color);
    let mut attacked = 0;
    for from in 0..64 {
        let Some(piece) = state.piece_at(from).filter(|piece| piece.color == color) else { continue };
        if piece.kind == Kind::Pawn {
            for df in [-1, 1] {
                if let Some(to) = step(from, (df, color.forward())) {
                    attacked |= 1 << to;
                }
            }
            continue;
        }
        for atom in rules.atoms(piece.kind).iter().filter(|atom| mode(atom).can_capture()) {
            for (to, _) in reach(state, from, atom, color.forward()) {
                attacked |= 1 << to;
            }
        }
    }
    attacked
}

type Key = (Square, Square, Option<Kind>, Special);

fn key(m: Move) -> (Square, Square, Option<u8>, u8) {
    (m.from, m.to, m.promo.map(|kind| kind as u8), m.special as u8)
}

/// The pseudo moves of the officers of a color, with the castles.
fn naive_moves(state: &State, color: Color, captures_only: bool) -> Vec<Key> {
    let rules = state.rules().side(color);
    let mut moves: Vec<Key> = Vec::new();
    for from in 0..64 {
        let Some(piece) = state.piece_at(from).filter(|piece| piece.color == color) else { continue };
        if piece.kind == Kind::Pawn {
            continue;
        }
        for atom in rules.atoms(piece.kind) {
            for (to, occupied) in reach(state, from, atom, color.forward()) {
                let foe = state.piece_at(to).is_some_and(|other| other.color != color);
                let legal =
                    if occupied { foe && mode(atom).can_capture() } else { mode(atom).can_move() && !captures_only };
                let m = (from, to, None, Special::None);
                if legal && !moves.contains(&m) {
                    moves.push(m);
                }
            }
        }
    }
    if rules.castling && !captures_only {
        let attacked = naive_attacks(state, color.other());
        let home: Square = if color == Color::White { 4 } else { 60 };
        let ready = |s: Square, kind: Kind| {
            state.piece_at(s).is_some_and(|piece| piece.kind == kind && piece.color == color && !piece.moved)
        };
        let empty = |squares: &[Square]| squares.iter().all(|&s| state.piece_at(s).is_none());
        if ready(home, Kind::King) && attacked & (1 << home) == 0 {
            let wings = [
                (home + 3, home + 1, home + 2, vec![home + 1, home + 2]),
                (home - 4, home - 1, home - 2, vec![home - 1, home - 2, home - 3]),
            ];
            for (rook, crossed, to, between) in wings {
                if ready(rook, Kind::Rook) && empty(&between) && attacked & (1 << crossed) == 0 {
                    // The castle replaces a king move with the same squares.
                    moves.retain(|&m| m != (home, to, None, Special::None));
                    moves.push((home, to, None, Special::Castle));
                }
            }
        }
    }
    moves
}

fn sorted(moves: impl IntoIterator<Item = Key>) -> Vec<(Square, Square, Option<u8>, u8)> {
    let mut list: Vec<_> =
        moves.into_iter().map(|(from, to, promo, special)| key(Move { from, to, promo, special })).collect();
    list.sort_unstable();
    list
}

#[test]
fn random_rules_agree_with_a_naive_generator() {
    let mut rng = Rng::new(0x5EED_0001);
    let mut moves = 0;
    let mut castles = 0;
    for round in 0..RULE_SETS {
        let rules = Rules::new(random_side(&mut rng), random_side(&mut rng));
        let state = State::new(&random_placements(&mut rng), rules).unwrap();
        for color in Color::ALL {
            for captures_only in [false, true] {
                let mut list = MoveList::new();
                pseudo_moves(&state, color, captures_only, &mut list);
                let engine: Vec<Key> = list
                    .iter()
                    .filter(|m| state.piece_at(m.from).is_some_and(|piece| piece.kind != Kind::Pawn))
                    .map(|m| (m.from, m.to, m.promo, m.special))
                    .collect();
                let distinct: BTreeSet<_> = engine.iter().map(|&(from, to, ..)| (from, to)).collect();
                assert_eq!(distinct.len(), engine.len(), "round {round}: two moves with the same squares");
                let naive = naive_moves(&state, color, captures_only);
                assert_eq!(
                    sorted(engine.iter().copied()),
                    sorted(naive),
                    "round {round}: the moves of {color:?} (captures only: {captures_only}) differ. Rules: {:?}",
                    state.rules().side(color)
                );
                moves += engine.len();
                castles += engine.iter().filter(|m| m.3 == Special::Castle).count();
            }
            let engine = (0..64).filter(|&s| is_attacked(&state, s, color)).fold(0, |set: Bitboard, s| set | 1 << s);
            assert_eq!(
                engine,
                naive_attacks(&state, color),
                "round {round}: the attacked squares of {color:?} differ. Rules: {:?}",
                state.rules().side(color)
            );
        }
    }
    // The random positions must give many moves and some castles, else the test shows little.
    assert!(moves > 30 * RULE_SETS, "only {moves} moves");
    assert!(castles > RULE_SETS / 20, "only {castles} castles");
}

/// Makes and takes back each pseudo move to `depth`. Returns the number of moves made.
fn walk(state: &mut State, depth: u32, counts: &mut [usize; 5]) {
    if depth == 0 {
        return;
    }
    let before = state.clone();
    let before_key = state.key();
    let mover = state.turn();
    let mut list = MoveList::new();
    pseudo_moves(state, mover, false, &mut list);
    for &m in &list {
        let piece = state.piece_at(m.from).expect("a move starts on a piece");
        assert_eq!(piece.color, mover);
        assert!(state.piece_at(m.to).is_none_or(|target| target.color != mover), "{m:?} captures a friend");
        counts[m.special as usize] += 1;
        let undo = state.make(m);
        assert!(state.is_consistent(), "the bitboards do not agree with the mailbox after {m:?}");
        assert_eq!(state.key(), key_from_scratch(state), "the key is wrong after {m:?}");
        walk(state, depth - 1, counts);
        state.unmake(m, undo);
        assert!(*state == before, "the unmake of {m:?} did not give back the state");
        assert_eq!(state.key(), before_key, "the unmake of {m:?} did not give back the key");
    }
}

#[test]
fn make_and_unmake_give_back_the_state_for_random_rules() {
    let mut rng = Rng::new(0x5EED_0002);
    let mut counts = [0; 5];
    for _ in 0..WALKS {
        let white = random_side(&mut rng);
        let white = random_pawns(&mut rng, white);
        let black = random_side(&mut rng);
        let black = random_pawns(&mut rng, black);
        let state = State::new(&random_placements(&mut rng), Rules::new(white, black)).unwrap();
        let mut state = state.with_turn(Color::ALL[rng.below(2) as usize]);
        walk(&mut state, 2, &mut counts);
    }
    // Each special move occurs: double steps, en passant captures, backward steps, and castles.
    let [plain, double, en_passant, backward, castle] = counts;
    assert!(plain > 500_000 && double > 5000 && en_passant > 300 && backward > 10_000 && castle > 1000, "{counts:?}");
}
