//! Random rule sets against a naive move generator.
//!
//! The naive generator in this file reads the rules data directly. It walks the offsets square
//! by square and uses no table of the engine. It follows the rules of `core/README.md`: the
//! groups of atoms, the steps of an atom, the conditions, en passant, the promotion, the clock,
//! the hooks, and the castle rows. For each random rule set and position, the test compares all the pseudo
//! moves (of the pawns too), the castles, and the attacked squares of the two colors, also after
//! each move that makes en passant squares. A second test walks the move tree, compares each
//! `make` with a naive `make`, and makes sure that `unmake` gives back the full state and the
//! key. The seeds are fixed, thus each run is the same.

use std::collections::BTreeSet;

use chrogue_engine::movegen::{evasion_moves, evasion_squares, may_give_check};
use chrogue_engine::rng::Rng;
use chrogue_engine::zobrist::key_from_scratch;
use chrogue_engine::{
    Atom, Bitboard, Castle, Color, Condition, Hook, Kind, Mode, Move, MoveList, Offset, Piece, Placement, Promotion,
    Promotions, Rules, SideRules, Special, Square, State, in_check, is_attacked, legal_moves, pseudo_moves,
};

/// The number of rule sets of the comparison with the naive generator.
const RULE_SETS: usize = 6000;
/// The number of rule sets of the make and unmake walk.
const WALKS: usize = 1500;

const MODES: [Mode; 3] = [Mode::MoveOrCapture, Mode::MoveOnly, Mode::CaptureOnly];

// ---- Random rules ----

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

/// An atom with random offsets, steps, mode, condition, and properties. A leap has offsets of
/// up to 3 squares, or sometimes up to 7. An atom with more steps has steps of up to 2
/// squares, thus knight-step riders occur.
fn random_atom(rng: &mut Rng) -> Atom {
    let mode = MODES[rng.below(3) as usize];
    let mut atom = if rng.below(2) == 0 {
        let max = if rng.below(8) == 0 { 7 } else { 3 };
        Atom::leap(&random_offsets(rng, max), mode)
    } else {
        let steps = if rng.below(2) == 0 { Atom::MAX_STEPS } else { 2 + rng.below(5) as u8 };
        Atom::slide(&random_offsets(rng, 2), mode).max_steps(steps)
    };
    if rng.below(4) == 0 {
        atom = atom.if_unmoved();
    }
    if rng.below(5) == 0 {
        atom = atom.makes_en_passant();
    }
    if rng.below(4) == 0 {
        atom = atom.captures_en_passant();
    }
    atom
}

/// Atoms like those of the pawn: steps forward, captures to the front, and a step back, with
/// random properties.
fn random_pawn_atom(rng: &mut Rng) -> Atom {
    let offsets: &[Offset] = match rng.below(5) {
        0 => &[(0, 1)],
        1 => &[(-1, 1), (1, 1)],
        2 => &[(0, -1)],
        3 => &[(-1, 0), (1, 0)],
        _ => &[(-1, -1), (1, -1)],
    };
    let mut atom = Atom::leap(offsets, MODES[rng.below(3) as usize]).max_steps(1 + rng.below(3) as u8);
    if rng.below(3) == 0 {
        atom = atom.if_unmoved();
    }
    if rng.below(3) == 0 {
        atom = atom.makes_en_passant();
    }
    if rng.below(2) == 0 {
        atom = atom.captures_en_passant();
    }
    atom
}

/// A hook with 1 to 3 random bends, or the hook of `Hook::right_angle` along random directions.
fn random_hook(rng: &mut Rng) -> Hook {
    let mode = MODES[rng.below(3) as usize];
    let min_leg = 1 + rng.below(3) as u8;
    if rng.below(2) == 0 {
        return Hook::right_angle(&random_offsets(rng, 1), min_leg, mode);
    }
    let bends = (0..1 + rng.below(3)).map(|_| (random_offsets(rng, 2)[0], random_offsets(rng, 2)[0])).collect();
    Hook { bends, min_leg, max_leg: min_leg + rng.below(8 - min_leg as u64) as u8, mode }
}

fn random_promotion(rng: &mut Rng) -> Promotion {
    let mut kinds = [Kind::Queen, Kind::Knight, Kind::Rook, Kind::Bishop];
    rng.shuffle(&mut kinds);
    let count = 1 + rng.below(4) as usize;
    Promotion { distance: rng.below(7) as u8, kinds: Promotions::new(&kinds[..count]).unwrap() }
}

/// 1 to 3 castles on random squares of the first two ranks, with a random partner.
fn random_castles(rng: &mut Rng) -> Vec<Castle> {
    let mut castles: Vec<Castle> = Vec::new();
    for _ in 0..1 + rng.below(3) {
        let squares = [rng.below(16), rng.below(16), rng.below(16)].map(|s| s as Square);
        let castle = Castle {
            king_from: if castles.is_empty() { 4 } else { castles[0].king_from },
            king_to: squares[0],
            partner: Kind::ALL[rng.below(5) as usize],
            partner_from: squares[1],
            partner_to: squares[2],
            empty: rng.next_u64() & 0xFFFF & rng.next_u64(),
            safe: rng.next_u64() & 0xFF & rng.next_u64() & rng.next_u64(),
        };
        let valid = castle.king_from != castle.king_to
            && castle.king_from != castle.partner_from
            && castle.king_to != castle.partner_to
            && !castles.iter().any(|c| c.king_to == castle.king_to);
        if valid {
            castles.push(castle);
        }
    }
    castles
}

/// Random atoms for each kind, the pawn too, random promotions, and random castles. The atoms
/// replace the movement of the kind, or add to it.
fn random_side(rng: &mut Rng) -> SideRules {
    let mut side = SideRules::standard();
    for kind in Kind::ALL {
        if kind == Kind::Pawn && rng.below(3) == 0 {
            // The pawn of chess with some of the rule flags.
            for flag in ["forcedMarch", "backpedal", "earlyPromo"] {
                if rng.below(2) == 0 {
                    side = side.with_flag(flag).unwrap();
                }
            }
            continue;
        }
        let mut atoms = Vec::new();
        for _ in 0..rng.below(4) {
            atoms.push(if kind == Kind::Pawn && rng.below(2) == 0 { random_pawn_atom(rng) } else { random_atom(rng) });
        }
        side = if rng.below(4) != 0 {
            side.with_kind(kind, atoms)
        } else {
            atoms.into_iter().fold(side, |side, atom| side.with_atom(kind, atom))
        };
        let promotion = match kind {
            Kind::Pawn => (rng.below(5) != 0).then(|| random_promotion(rng)),
            _ => (rng.below(12) == 0).then(|| random_promotion(rng)),
        };
        side = side.with_promotion(kind, promotion);
        // Only a kind with plain atoms and no promotion can have a hook (`RulesError::BadHook`).
        let plain = promotion.is_none()
            && side
                .atoms(kind)
                .iter()
                .all(|atom| atom.condition == Condition::Always && !atom.makes_en_passant && !atom.captures_en_passant);
        if plain && rng.below(3) == 0 {
            for _ in 0..1 + rng.below(2) {
                side = side.with_hook(kind, random_hook(rng));
            }
        }
    }
    // A king never makes en passant squares (`RulesError::KingMakesEnPassant`).
    for atom in &mut side.kinds[Kind::King.index()].atoms {
        atom.makes_en_passant = false;
    }
    let castles = match rng.below(4) {
        0 => Vec::new(),
        1 => random_castles(rng),
        _ => Castle::STANDARD.to_vec(),
    };
    let side = side.with_castles(castles);
    side.validate().expect("the random rules are valid");
    side
}

/// 4 to 23 random pieces on different squares. Some kings and partners stand on the squares of
/// the castles of their side, and some pawns stand on their start rank next to enemy pawns.
fn random_placements(rng: &mut Rng, rules: &Rules) -> Vec<Placement> {
    let mut placements = Vec::new();
    let mut used: Bitboard = 0;
    let mut place = |placements: &mut Vec<Placement>, square: Square, kind: Kind, color: Color, moved: bool| {
        if used & (1 << square) == 0 {
            used |= 1 << square;
            let id = placements.len() as u16;
            placements.push(Placement { piece: Piece { id, kind, color, moved }, square });
        }
    };
    for color in Color::ALL {
        if rng.below(3) != 0 {
            for castle in &rules.side(color).castles {
                let castle = castle.for_color(color);
                place(&mut placements, castle.king_from, Kind::King, color, rng.below(5) == 0);
                if rng.below(4) != 0 {
                    place(&mut placements, castle.partner_from, castle.partner, color, rng.below(5) == 0);
                }
            }
        }
    }
    for (color, start, enemy) in [(Color::White, 8, 24), (Color::Black, 48, 32)] {
        for _ in 0..rng.below(3) {
            let file = 1 + rng.below(6) as Square;
            let side = if rng.below(2) == 0 { file - 1 } else { file + 1 };
            place(&mut placements, start + file, Kind::Pawn, color, false);
            place(&mut placements, enemy + side + 8 * rng.below(2) as Square, Kind::Pawn, color.other(), true);
        }
    }
    for _ in 0..4 + rng.below(20) {
        let kind = Kind::ALL[rng.below(6) as usize];
        let color = Color::ALL[rng.below(2) as usize];
        place(&mut placements, rng.below(64) as Square, kind, color, rng.below(2) == 0);
    }
    placements
}

// ---- The naive generator ----

fn step(from: Square, (df, dr): Offset) -> Option<Square> {
    let file = (from % 8) as i8 + df;
    let rank = (from / 8) as i8 + dr;
    ((0..8).contains(&file) && (0..8).contains(&rank)).then(|| (rank * 8 + file) as Square)
}

/// A square that an atom reaches from `from`: the square, the number of squares that the step
/// passes, and the piece on the square. An atom stops on the first piece.
struct Reached {
    to: Square,
    passed: u32,
    piece: Option<Piece>,
}

fn walk(state: &State, from: Square, atom: &Atom, forward: i8) -> Vec<Reached> {
    let mut squares = Vec::new();
    for &(df, dr) in &atom.offsets {
        let mut at = from;
        for n in 0..atom.max_steps as u32 {
            let Some(to) = step(at, (df, dr * forward)) else { break };
            let piece = state.piece_at(to);
            squares.push(Reached { to, passed: n, piece });
            if piece.is_some() {
                break;
            }
            at = to;
        }
    }
    squares
}

/// The squares where the last step of a hook ends: each square of the leg is empty, and the leg
/// has `min_leg` squares or more.
fn hook_targets(state: &State, from: Square, hook: &Hook, forward: i8) -> Vec<Square> {
    let mut targets = Vec::new();
    for &((df, dr), (lf, lr)) in &hook.bends {
        let mut at = from;
        for squares in 1..=hook.max_leg {
            let Some(corner) = step(at, (df, dr * forward)).filter(|&s| state.piece_at(s).is_none()) else { break };
            at = corner;
            if squares >= hook.min_leg
                && let Some(to) = step(corner, (lf, lr * forward))
            {
                targets.push(to);
            }
        }
    }
    targets
}

fn usable(atom: &Atom, piece: Piece) -> bool {
    atom.condition == Condition::Always || !piece.moved
}

/// The rank of a square from the view of a color.
fn view_rank(s: Square, color: Color) -> u8 {
    if color == Color::White { s / 8 } else { 7 - s / 8 }
}

/// The squares that the pieces of a color attack.
fn naive_attacks(state: &State, color: Color) -> Bitboard {
    let rules = state.rules().side(color);
    let mut attacked = 0;
    for from in 0..64 {
        let Some(piece) = state.piece_at(from).filter(|piece| piece.color == color) else { continue };
        for atom in rules.atoms(piece.kind).iter().filter(|atom| atom.mode.can_capture() && usable(atom, piece)) {
            for reached in walk(state, from, atom, color.forward()) {
                attacked |= 1 << reached.to;
            }
        }
        for hook in rules.kind(piece.kind).hooks.iter().filter(|hook| hook.mode.can_capture()) {
            for to in hook_targets(state, from, hook, color.forward()) {
                attacked |= 1 << to;
            }
        }
    }
    attacked
}

type Key = (Square, Square, Option<Kind>, Special);

fn group_key(atom: &Atom) -> (Condition, bool, bool) {
    (atom.condition, atom.makes_en_passant, atom.captures_en_passant)
}

/// The moves of one piece.
fn naive_piece_moves(state: &State, from: Square, piece: Piece, captures_only: bool, moves: &mut Vec<Key>) {
    let color = piece.color;
    let forward = color.forward();
    let rules = state.rules().side(color).kind(piece.kind);
    let foe = |p: &Option<Piece>| p.is_some_and(|p| p.color != color);
    let promotes = |to: Square| {
        rules.promotion.is_some_and(|promotion| {
            view_rank(to, color) >= 7 - promotion.distance && view_rank(to, color) >= view_rank(from, color)
        })
    };
    let push = |moves: &mut Vec<Key>, to: Square, special: Special| {
        if promotes(to) {
            for &kind in rules.promotion.unwrap().kinds.as_slice() {
                moves.push((from, to, Some(kind), special));
            }
        } else {
            moves.push((from, to, None, special));
        }
    };

    // The groups, in the order of their first atom.
    let mut groups: Vec<(_, Vec<&Atom>)> = Vec::new();
    for atom in &rules.atoms {
        match groups.iter_mut().find(|(key, _)| *key == group_key(atom)) {
            Some((_, atoms)) => atoms.push(atom),
            None => groups.push((group_key(atom), vec![atom])),
        }
    }
    groups.retain(|(_, atoms)| usable(atoms[0], piece));

    // The en passant squares where a group can capture.
    let ep = state.ep_squares();
    let victim_is_foe = ep != 0 && foe(&state.piece_at(state.ep_victim()));
    let ep_targets = |atoms: &[&Atom]| -> Vec<Square> {
        let mut targets = Vec::new();
        for atom in atoms.iter().filter(|atom| atom.captures_en_passant && atom.mode.can_capture()) {
            for reached in walk(state, from, atom, forward) {
                if reached.piece.is_none() && ep & (1 << reached.to) != 0 && !targets.contains(&reached.to) {
                    targets.push(reached.to);
                }
            }
        }
        targets
    };
    let all_ep: BTreeSet<Square> =
        if victim_is_foe { groups.iter().flat_map(|(_, atoms)| ep_targets(atoms)).collect() } else { BTreeSet::new() };

    let mut given = BTreeSet::new();
    let mut ep_given = BTreeSet::new();
    for ((_, makes_ep, _), atoms) in &groups {
        // to -> (capture, passes squares)
        let mut targets: Vec<(Square, bool, bool)> = Vec::new();
        for atom in atoms {
            for reached in walk(state, from, atom, forward) {
                let quiet = reached.piece.is_none() && atom.mode.can_move();
                let capture = foe(&reached.piece) && atom.mode.can_capture();
                if !quiet && !capture {
                    continue;
                }
                let trail = *makes_ep && reached.passed >= 1;
                match targets.iter_mut().find(|t| t.0 == reached.to) {
                    Some(t) => t.2 |= trail,
                    None => targets.push((reached.to, capture, trail)),
                }
            }
        }
        for &(to, capture, trail) in &targets {
            if !given.insert(to) || all_ep.contains(&to) {
                continue;
            }
            if captures_only && !capture && !promotes(to) {
                continue;
            }
            // A promotion makes no en passant squares.
            let special = if trail && !promotes(to) { Special::DoubleStep } else { Special::None };
            push(moves, to, special);
        }
        if victim_is_foe {
            for to in ep_targets(atoms) {
                if ep_given.insert(to) {
                    push(moves, to, Special::EnPassant);
                }
            }
        }
    }
    // A hook gives a target that no atom gave.
    for hook in &rules.hooks {
        for to in hook_targets(state, from, hook, forward) {
            let piece = state.piece_at(to);
            let capture = foe(&piece) && hook.mode.can_capture();
            let quiet = piece.is_none() && hook.mode.can_move() && !captures_only;
            if (quiet || capture) && given.insert(to) {
                push(moves, to, Special::None);
            }
        }
    }
}

/// True if the king of `color` on the lowest square is attacked on this board. A side with no
/// king is never in check.
fn naive_in_check(state: &State, board: &[Option<Piece>; 64], color: Color) -> bool {
    let placements: Vec<Placement> =
        (0..64).filter_map(|s| board[s as usize].map(|piece| Placement { piece, square: s })).collect();
    let after = State::with_tables(&placements, state.shared_tables()).unwrap();
    let king =
        (0..64).find(|&s| board[s as usize].is_some_and(|piece| piece.kind == Kind::King && piece.color == color));
    king.is_some_and(|king| naive_attacks(&after, color.other()) & 1 << king != 0)
}

/// The castles of a color that are possible now. A legal castle replaces each move of the king
/// with the same squares.
fn naive_castles(state: &State, color: Color, moves: &mut Vec<Key>) {
    let attacked = naive_attacks(state, color.other());
    for castle in &state.rules().side(color).castles {
        let castle = castle.for_color(color);
        let ready = |s: Square, kind: Kind| {
            state.piece_at(s).is_some_and(|piece| piece.kind == kind && piece.color == color && !piece.moved)
        };
        let mut empty = castle.empty | 1 << castle.king_to | 1 << castle.partner_to;
        empty &= !(1 << castle.king_from | 1 << castle.partner_from);
        let empty = (0..64).filter(|&s| empty & (1 << s) != 0).all(|s| state.piece_at(s).is_none());
        if ready(castle.king_from, Kind::King)
            && ready(castle.partner_from, castle.partner)
            && empty
            && attacked & castle.safe == 0
        {
            let castle_move =
                Move { from: castle.king_from, to: castle.king_to, promo: None, special: Special::Castle };
            if !naive_in_check(state, &naive_make(state, castle_move).0, color) {
                moves.retain(|&(from, to, ..)| (from, to) != (castle.king_from, castle.king_to));
            }
            moves.push((castle.king_from, castle.king_to, None, Special::Castle));
        }
    }
}

/// The pseudo moves of a color.
fn naive_moves(state: &State, color: Color, captures_only: bool) -> Vec<Key> {
    let mut moves = Vec::new();
    for from in 0..64 {
        if let Some(piece) = state.piece_at(from).filter(|piece| piece.color == color) {
            naive_piece_moves(state, from, piece, captures_only, &mut moves);
        }
    }
    if !captures_only {
        naive_castles(state, color, &mut moves);
    }
    moves
}

/// The en passant squares that a move makes: the squares that it passes on each usable atom of
/// its kind that makes en passant squares.
fn naive_trail(state: &State, m: Move) -> Bitboard {
    if m.special != Special::DoubleStep || m.promo.is_some() {
        return 0;
    }
    let piece = state.piece_at(m.from).unwrap();
    let forward = piece.color.forward();
    let mut squares = 0;
    for atom in state.rules().side(piece.color).atoms(piece.kind) {
        if !atom.makes_en_passant || !usable(atom, piece) {
            continue;
        }
        for &(df, dr) in &atom.offsets {
            let mut passed = 0;
            let mut at = m.from;
            for _ in 0..atom.max_steps {
                let Some(to) = step(at, (df, dr * forward)) else { break };
                if to == m.to {
                    squares |= passed;
                    break;
                }
                if state.piece_at(to).is_some() {
                    break;
                }
                passed |= 1 << to;
                at = to;
            }
        }
    }
    squares
}

/// The board and the clock after a move, by the rules of `core/README.md`.
fn naive_make(state: &State, m: Move) -> ([Option<Piece>; 64], u32, Bitboard) {
    let mut board = *state.board();
    let mut piece = board[m.from as usize].take().unwrap();
    let mut captured = false;
    if m.special == Special::Castle {
        let (partner_from, partner_to) = state.rules().castle_partner(piece.color, m).unwrap();
        let mut partner = board[partner_from as usize].take().unwrap();
        partner.moved = true;
        piece.moved = true;
        board[m.to as usize] = Some(piece);
        board[partner_to as usize] = Some(partner);
        return (board, state.clock + 1, 0);
    }
    if m.special == Special::EnPassant {
        captured = board[state.ep_victim() as usize].take().is_some();
    }
    captured |= board[m.to as usize].is_some();
    let kind = piece.kind;
    piece.kind = m.promo.unwrap_or(kind);
    piece.moved = true;
    board[m.to as usize] = Some(piece);
    let clock = if captured { 0 } else { state.clock + 1 };
    (board, clock, naive_trail(state, m))
}

// ---- The tests ----

fn sorted(moves: impl IntoIterator<Item = Key>) -> Vec<(Square, Square, Option<u8>, u8)> {
    let mut list: Vec<_> = moves
        .into_iter()
        .map(|(from, to, promo, special)| (from, to, promo.map(|kind| kind as u8), special as u8))
        .collect();
    list.sort_unstable();
    list
}

/// The numbers of moves of each special property, of promotions, and of moves of atoms with
/// more than one step and less than a full slide.
#[derive(Default, Debug)]
struct Seen {
    moves: usize,
    special: [usize; 4],
    promotions: usize,
    /// States where a side is in check, and the legal moves of pieces that are not a king there.
    checks: usize,
    evasions: usize,
}

/// Compares the engine with the naive generator in one state.
fn compare(state: &State, label: &str, seen: &mut Seen) {
    for color in Color::ALL {
        for captures_only in [false, true] {
            let mut list = MoveList::new();
            pseudo_moves(state, color, captures_only, &mut list);
            let engine: Vec<Key> = list.iter().map(|m| (m.from, m.to, m.promo, m.special)).collect();
            // A castle that is not legal and the king move to its square have the same squares.
            // The legality filter removes the castle.
            let distinct: BTreeSet<_> = engine
                .iter()
                .map(|&(from, to, promo, special)| (from, to, promo.map(|kind| kind as u8), special == Special::Castle))
                .collect();
            assert_eq!(distinct.len(), engine.len(), "{label}: two moves with the same squares");
            let naive = naive_moves(state, color, captures_only);
            assert_eq!(
                sorted(engine.iter().copied()),
                sorted(naive),
                "{label}: the moves of {color:?} (captures only: {captures_only}) differ. Rules: {:?}, ep {:x} victim {}",
                state.rules().side(color),
                state.ep_squares(),
                state.ep_victim()
            );
            if !captures_only {
                seen.moves += engine.len();
                for &(_, _, promo, special) in &engine {
                    seen.special[special as usize] += 1;
                    seen.promotions += promo.is_some() as usize;
                }
            }
        }
        // The legal moves of the side never have two moves with the same squares.
        let mut legal = if color == state.turn() { state.clone() } else { state.clone().with_turn(color) };
        let mut list = MoveList::new();
        legal_moves(&mut legal, &mut list);
        let distinct: BTreeSet<_> = list.iter().map(|m| (m.from, m.to, m.promo.map(|kind| kind as u8))).collect();
        assert_eq!(distinct.len(), list.len(), "{label}: two legal moves with the same squares");
        // In check, the legal moves of `evasion_moves` are all the legal moves, in the same order.
        if in_check(&legal, color) {
            seen.checks += 1;
            let mut evasions = MoveList::new();
            evasion_moves(&legal, color, evasion_squares(&legal, color), &mut evasions);
            let evasions: Vec<Move> =
                evasions.iter().copied().filter(|&m| chrogue_engine::is_legal(&mut legal, m, color)).collect();
            assert_eq!(evasions, list.as_slice(), "{label}: the evasions differ from the legal moves");
            seen.evasions += evasions.iter().filter(|m| legal.piece_at(m.from).unwrap().kind != Kind::King).count();
        }
        let engine = (0..64).filter(|&s| is_attacked(state, s, color)).fold(0, |set: Bitboard, s| set | 1 << s);
        assert_eq!(
            engine,
            naive_attacks(state, color),
            "{label}: the attacked squares of {color:?} differ. Rules: {:?}",
            state.rules().side(color)
        );
    }
}

#[test]
fn random_rules_agree_with_a_naive_generator() {
    let mut rng = Rng::new(0x5EED_0001);
    let mut seen = Seen::default();
    let mut ep_states = 0;
    for round in 0..RULE_SETS {
        let rules = Rules::new(random_side(&mut rng), random_side(&mut rng));
        let placements = random_placements(&mut rng, &rules);
        let mut state = State::new(&placements, rules).unwrap().with_turn(Color::ALL[rng.below(2) as usize]);
        compare(&state, &format!("round {round}"), &mut seen);
        // The states after the moves that make en passant squares.
        let mut list = MoveList::new();
        pseudo_moves(&state, state.turn(), false, &mut list);
        for &m in list.iter().filter(|m| m.special == Special::DoubleStep).take(3) {
            let undo = state.make(m);
            ep_states += (state.ep_squares() != 0) as usize;
            compare(&state, &format!("round {round} after {m:?}"), &mut seen);
            state.unmake(m, undo);
        }
    }
    println!("{seen:?}, {ep_states} states with en passant squares");
    // The random positions must give many moves and each kind of special move, else the test
    // shows little.
    let [_, double, en_passant, castle] = seen.special;
    assert!(seen.moves > 30 * RULE_SETS, "{seen:?}");
    assert!(double > 5000 && en_passant > 500 && castle > RULE_SETS / 10, "{seen:?}");
    assert!(seen.promotions > 10_000 && ep_states > 2000, "{seen:?}, {ep_states} states with en passant");
    assert!(seen.checks > 1000 && seen.evasions > 1000, "{seen:?}");
}

/// Makes and takes back each pseudo move to `depth`, and compares each `make` with `naive_make`.
fn walk_tree(state: &mut State, depth: u32, counts: &mut [usize; 6]) {
    if depth == 0 {
        return;
    }
    let before = state.clone();
    let before_key = state.key();
    let mover = state.turn();
    // The search skips `in_check` after a move that cannot give check. This is valid when the
    // other side is not in check before the move.
    let other_safe = !in_check(state, mover.other());
    let mut list = MoveList::new();
    pseudo_moves(state, mover, false, &mut list);
    for &m in &list {
        let piece = state.piece_at(m.from).expect("a move starts on a piece");
        assert_eq!(piece.color, mover);
        // A castle can swap the king and its partner.
        let friend = state.piece_at(m.to).is_some_and(|target| target.color == mover);
        assert!(!friend || m.special == Special::Castle, "{m:?} captures a friend");
        counts[m.special as usize] += 1;
        let (board, clock, ep) = naive_make(state, m);
        counts[4] += (ep.count_ones() > 1) as usize;
        let undo = state.make(m);
        assert_eq!(state.board(), &board, "the board after {m:?}. Rules: {:?}", state.rules().side(mover));
        assert_eq!(state.clock, clock, "the clock after {m:?}");
        assert_eq!(state.ep_squares(), ep, "the en passant squares after {m:?}");
        if ep != 0 {
            assert_eq!(state.ep_victim(), m.to);
        }
        assert!(state.is_consistent(), "the bitboards do not agree with the mailbox after {m:?}");
        assert_eq!(state.key(), key_from_scratch(state), "the key is wrong after {m:?}");
        if !in_check(state, mover) {
            assert_no_royal_capture(state, mover);
            if other_safe && in_check(state, mover.other()) {
                counts[5] += 1;
                assert!(may_give_check(state, m, &undo), "{m:?} gives check, but `may_give_check` is false");
            }
        }
        walk_tree(state, depth - 1, counts);
        state.unmake(m, undo);
        assert!(*state == before, "the unmake of {m:?} did not give back the state");
        assert_eq!(state.key(), before_key, "the unmake of {m:?} did not give back the key");
    }
}

/// After a legal move of `mover`, no pseudo move of the other side removes the royal king of
/// `mover` (the king on the lowest square): not by a move to its square, and not by an en
/// passant capture. The search relies on this: only its root can capture a king.
fn assert_no_royal_capture(state: &State, mover: Color) {
    let Some(king) = state.king_square(mover) else { return };
    let mut list = MoveList::new();
    pseudo_moves(state, state.turn(), false, &mut list);
    for &m in &list {
        let victim = if m.special == Special::EnPassant { state.ep_victim() } else { m.to };
        assert!(victim != king, "{m:?} captures the king of a side that is not in check");
    }
}

#[test]
fn make_and_unmake_give_back_the_state_for_random_rules() {
    let mut rng = Rng::new(0x5EED_0002);
    let mut counts = [0; 6];
    for _ in 0..WALKS {
        let rules = Rules::new(random_side(&mut rng), random_side(&mut rng));
        let placements = random_placements(&mut rng, &rules);
        let state = State::new(&placements, rules).unwrap();
        let mut state = state.with_turn(Color::ALL[rng.below(2) as usize]);
        walk_tree(&mut state, 2, &mut counts);
    }
    println!("moves by special property, moves with two en passant squares or more, and checks: {counts:?}");
    // Each special move occurs: double steps, en passant captures, castles, and
    // moves that make more than one en passant square.
    // The last count is the moves that give check.
    let [plain, double, en_passant, castle, wide, checks] = counts;
    assert!(plain > 500_000 && double > 5000 && en_passant > 300 && castle > 1000 && wide > 1000, "{counts:?}");
    assert!(checks > 10_000, "{counts:?}");
}
