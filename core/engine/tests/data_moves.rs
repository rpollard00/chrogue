//! Pawn moves, en passant, castles, ranges, and first-move atoms as data. Each test defines its
//! rules through the public rules data only: the engine has no code for these rules. Each test
//! checks the generated moves, and the attacks and the checks where the rule changes them.

mod common;

use chrogue_engine::fen::square;
use chrogue_engine::rules::{CAMEL, FORWARD, FORWARD_DIAG, KING, ORTHO};
use chrogue_engine::zobrist::key_from_scratch;
use chrogue_engine::{
    Atom, Castle, Color, Kind, Mode, Move, Piece, Placement, Promotion, Promotions, Rules, SideRules, Special, Square,
    State, in_check, is_attacked, perft,
};
use common::{from_fen, legal, squares, targets};

fn white(rules: SideRules) -> Rules {
    Rules::new(rules, SideRules::standard())
}

/// The legal moves from a square.
fn moves(state: &mut State, from: &str) -> Vec<Move> {
    legal(state).into_iter().filter(|m| m.from == square(from)).collect()
}

/// The legal move from `from` to `to` with no promotion.
fn find(state: &mut State, from: &str, to: &str) -> Move {
    moves(state, from).into_iter().find(|m| m.to == square(to) && m.promo.is_none()).expect("the move is legal")
}

/// The same pieces, with the `moved` flag of the piece on `s` set.
fn with_moved(state: &State, s: Square, rules: Rules) -> State {
    let pieces: Vec<Placement> = (0..64)
        .filter_map(|at| {
            state.piece_at(at).map(|piece| Placement { piece: Piece { moved: at == s, ..piece }, square: at })
        })
        .collect();
    State::new(&pieces, rules).unwrap().with_turn(state.turn())
}

/// The pawn of ordinary chess with other atoms.
fn pawn(atoms: Vec<Atom>) -> SideRules {
    SideRules::standard().with_kind(Kind::Pawn, atoms)
}

/// The step and the double step of the pawn of ordinary chess.
fn pawn_steps() -> Vec<Atom> {
    vec![
        Atom::leap(&FORWARD, Mode::MoveOnly),
        Atom::slide(&FORWARD, Mode::MoveOnly).max_steps(2).if_unmoved().makes_en_passant(),
    ]
}

#[test]
fn pawns_that_capture_straight_ahead_and_not_diagonally() {
    // The forward step can also capture. The pawn has no diagonal capture.
    let mut atoms = pawn_steps();
    atoms[0] = Atom::leap(&FORWARD, Mode::MoveOrCapture);
    let rules = || white(pawn(atoms.clone()));

    // The pawn on e3 captures the pawn on e4, and not the pawn on d4.
    let mut state = from_fen("4k3/8/8/8/3pp3/4P3/8/4K3", Color::White, rules());
    assert_eq!(targets(&mut state, "e3"), squares(&["e4"]));
    assert_eq!(
        moves(&mut state, "e3")[0],
        Move { from: square("e3"), to: square("e4"), promo: None, special: Special::None }
    );
    assert!(is_attacked(&state, square("e4"), Color::White));
    assert!(!is_attacked(&state, square("d4"), Color::White) && !is_attacked(&state, square("f4"), Color::White));
    // A capture into the last rank promotes.
    let mut state = from_fen("4r2k/4P3/8/8/8/8/8/4K3", Color::White, rules());
    assert_eq!(moves(&mut state, "e7").len(), 4);
    assert!(moves(&mut state, "e7").iter().all(|m| m.to == square("e8") && m.promo.is_some()));

    // The pawn gives check straight ahead, and not diagonally.
    assert!(in_check(&from_fen("4k3/4P3/8/8/8/8/8/4K3", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("5k2/4P3/8/8/8/8/8/4K3", Color::Black, rules()), Color::Black));
    assert!(in_check(&from_fen("5k2/4P3/8/8/8/8/8/4K3", Color::Black, Rules::standard()), Color::Black));
}

#[test]
fn a_forward_capture_and_a_quiet_diagonal_step_with_the_pawn_of_ordinary_chess() {
    // Shield Wall and Echelon: the pawn of ordinary chess with two more atoms.
    let side = || {
        SideRules::standard()
            .with_atom(Kind::Pawn, Atom::leap(&FORWARD, Mode::CaptureOnly))
            .with_atom(Kind::Pawn, Atom::leap(&FORWARD_DIAG, Mode::MoveOnly))
    };
    let rules = || white(side());

    // The pawn on e3 captures on e4 and on d4, and steps to the empty square f4.
    let mut state = from_fen("4k3/8/8/8/3pp3/4P3/8/4K3", Color::White, rules());
    assert_eq!(targets(&mut state, "e3"), squares(&["d4", "e4", "f4"]));
    // The pawn attacks the three squares before it, thus it gives check straight ahead.
    assert!(in_check(&from_fen("4k3/4P3/8/8/8/8/8/4K3", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("4k3/4P3/8/8/8/8/8/4K3", Color::Black, Rules::standard()), Color::Black));
    // An unmoved pawn with a piece before it has no double step: the capture is one square.
    let mut state = from_fen("4k3/8/8/8/8/4n3/4P3/4K3", Color::White, rules());
    assert_eq!(targets(&mut state, "e2"), squares(&["d3", "e3", "f3"]));
    // The capture and the diagonal step promote on the last rank.
    let mut state = from_fen("4r2k/4P3/8/8/8/8/8/K7", Color::White, rules());
    let promotions = moves(&mut state, "e7");
    assert_eq!(promotions.len(), 12);
    assert!(promotions.iter().all(|m| m.promo.is_some()));
    for to in ["d8", "e8", "f8"] {
        assert_eq!(promotions.iter().filter(|m| m.to == square(to)).count(), 4, "{to}");
    }
    // The diagonal step is not an en passant capture, and the usual en passant capture stays.
    let mut state = from_fen("4k3/3p4/8/4P3/8/8/8/4K3", Color::Black, Rules::new(side(), SideRules::standard()));
    let double = find(&mut state, "d7", "d5");
    state.make(double);
    assert_eq!(find(&mut state, "e5", "d6").special, Special::EnPassant);
    assert_eq!(find(&mut state, "e5", "f6").special, Special::None);
}

#[test]
fn a_sideways_pawn_step_that_only_moves() {
    let sidestep = |atom: Atom| white(SideRules::standard().with_atom(Kind::Pawn, atom));
    let rules = || sidestep(Atom::leap(&[(-1, 0), (1, 0)], Mode::MoveOnly));

    // The pawn on d4 steps to e4, and does not capture the knight on c4.
    let mut state = from_fen("4k3/8/8/8/2nP4/8/8/4K3", Color::White, rules());
    assert_eq!(targets(&mut state, "d4"), squares(&["d5", "e4"]));
    // The step attacks no square: the king on e4 is not in check, and c4 is not attacked.
    let check = from_fen("8/8/8/7k/3P4/8/8/4K3", Color::Black, rules());
    assert!(!is_attacked(&check, square("e4"), Color::White) && !is_attacked(&check, square("c4"), Color::White));
    assert!(!in_check(&from_fen("8/8/8/8/3Pk3/8/8/4K3", Color::Black, rules()), Color::Black));
}

#[test]
fn a_backward_pawn_capture() {
    let back = || SideRules::standard().with_atom(Kind::Pawn, Atom::leap(&[(-1, -1), (1, -1)], Mode::CaptureOnly));
    let rules = || Rules::new(back(), back());

    // The pawn on e4 captures backward on d3, and forward on f5 as usual.
    let mut state = from_fen("4k3/8/8/5n2/4P3/3n4/8/K7", Color::White, rules());
    assert_eq!(targets(&mut state, "e4"), squares(&["d3", "e5", "f5"]));
    let capture = find(&mut state, "e4", "d3");
    assert_eq!(capture.special, Special::None);
    // The capture gives check backward. Black is mirrored: its backward is toward rank 8.
    assert!(in_check(&from_fen("8/8/8/8/4P3/5k2/8/4K3", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("8/8/8/8/4P3/5k2/8/4K3", Color::Black, Rules::standard()), Color::Black));
    assert!(in_check(&from_fen("4k3/8/8/8/8/3K4/4p3/8", Color::White, rules()), Color::White));
    assert!(!in_check(&from_fen("4k3/8/8/8/8/3K4/4p3/8", Color::White, Rules::standard()), Color::White));
}

#[test]
fn a_triple_first_step_makes_two_en_passant_squares() {
    // The first move can go up to three squares. The squares that it passes become en passant
    // squares, and the pawn that moved is the victim of an en passant capture on each of them.
    let mut atoms = pawn_steps();
    atoms[1] = atoms[1].clone().max_steps(3);
    atoms.push(Atom::leap(&FORWARD_DIAG, Mode::CaptureOnly).captures_en_passant());
    let triple = || pawn(atoms.clone());
    let rules = || Rules::new(triple(), triple());

    let start = from_fen("4k3/8/8/5p2/3p4/8/4P3/4K3", Color::White, rules());
    let mut state = start.clone();
    assert_eq!(targets(&mut state, "e2"), squares(&["e3", "e4", "e5"]));
    let triple_step = find(&mut state, "e2", "e5");
    assert_eq!(triple_step.special, Special::DoubleStep);
    assert_eq!(state.ep_squares_of(triple_step), squares_set(&["e3", "e4"]));

    state.make(triple_step);
    assert_eq!(state.ep_squares(), squares_set(&["e3", "e4"]));
    assert_eq!(state.ep_victim(), square("e5"));
    assert_eq!(state.key(), key_from_scratch(&state));
    // d4 captures on e3 and f5 captures on e4. Each capture removes the pawn on e5.
    for (from, to) in [("d4", "e3"), ("f5", "e4")] {
        let capture = find(&mut state, from, to);
        assert_eq!(capture.special, Special::EnPassant);
        let undo = state.make(capture);
        assert_eq!(undo.captured_square, square("e5"));
        assert!(state.piece_at(square("e5")).is_none() && state.ep_squares() == 0);
        assert_eq!(state.key(), key_from_scratch(&state));
        state.unmake(capture, undo);
    }

    // A move of two squares passes one square: only e3 is an en passant square, and f5 takes
    // the pawn on e4 by an ordinary capture.
    let mut state = start.clone();
    let double = find(&mut state, "e2", "e4");
    state.make(double);
    assert_eq!((state.ep_squares(), state.ep_victim()), (squares_set(&["e3"]), square("e4")));
    assert_eq!(find(&mut state, "f5", "e4").special, Special::None);
    assert_eq!(find(&mut state, "d4", "e3").special, Special::EnPassant);
    // A piece on e4 blocks the step: e3 only. A pawn that moved has no first step.
    let mut blocked = from_fen("4k3/8/8/8/4n3/8/4P3/4K3", Color::White, rules());
    assert_eq!(targets(&mut blocked, "e2"), squares(&["e3"]));
    let mut moved = with_moved(&start, square("e2"), rules());
    assert_eq!(targets(&mut moved, "e2"), squares(&["e3"]));
}

fn squares_set(names: &[&str]) -> u64 {
    names.iter().fold(0, |set, name| set | 1 << square(name))
}

#[test]
fn a_king_that_slides_two_squares() {
    let rules = || {
        white(SideRules::standard().with_kind(
            Kind::King,
            vec![Atom::leap(&KING, Mode::MoveOrCapture), Atom::slide(&ORTHO, Mode::MoveOrCapture).max_steps(2)],
        ))
    };

    // The king on d4 goes two squares on files and ranks. The pawn on d5 blocks the line up.
    let mut state = from_fen("4k3/8/8/3P4/3K4/8/8/8", Color::White, rules());
    assert_eq!(targets(&mut state, "d4"), squares(&["b4", "c3", "c4", "c5", "d2", "d3", "e3", "e4", "e5", "f4"]));
    // The king gives check from two squares away, and not from three squares or past a piece.
    assert!(in_check(&from_fen("4k3/8/4K3/8/8/8/8/8", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("4k3/8/4K3/8/8/8/8/8", Color::Black, Rules::standard()), Color::Black));
    assert!(!in_check(&from_fen("4k3/8/8/4K3/8/8/8/8", Color::Black, rules()), Color::Black));
    assert!(!in_check(&from_fen("4k3/4p3/4K3/8/8/8/8/8", Color::Black, rules()), Color::Black));
    // The black king cannot go to c4, two squares from the white king on its rank.
    let mut state = from_fen("8/8/8/2k5/4K3/8/8/8", Color::Black, rules());
    assert_eq!(targets(&mut state, "c5"), squares(&["b4", "b5", "b6", "c6", "d6"]));
    let mut standard = from_fen("8/8/8/2k5/4K3/8/8/8", Color::Black, Rules::standard());
    assert!(targets(&mut standard, "c5").contains(&square("c4")));
    // From e1, the slide goes to the castle squares: the castle takes the place of the king move.
    let mut state = from_fen("4k3/8/8/8/8/8/8/R3K2R", Color::White, rules());
    let to_g1: Vec<Special> =
        moves(&mut state, "e1").iter().filter(|m| m.to == square("g1")).map(|m| m.special).collect();
    assert_eq!(to_g1, vec![Special::Castle]);
}

#[test]
fn a_castle_with_the_queen_as_partner() {
    // The queen side castle has a queen on a1 in place of the rook.
    let queen_castle = Castle { partner: Kind::Queen, ..Castle::QUEEN_SIDE };
    let castles = || SideRules::standard().with_castles(vec![queen_castle, Castle::KING_SIDE]);
    let rules = || Rules::new(castles(), castles());

    let mut state = from_fen("3k4/8/8/8/8/8/8/Q3K2R", Color::White, rules());
    let castle = find(&mut state, "e1", "c1");
    assert_eq!(castle.special, Special::Castle);
    assert_eq!(state.rules().castle_partner(Color::White, castle), Some((square("a1"), square("d1"))));
    assert_eq!(find(&mut state, "e1", "g1").special, Special::Castle);
    let before = state.clone();
    let undo = state.make(castle);
    assert_eq!(state.piece_at(square("d1")).map(|p| (p.kind, p.moved)), Some((Kind::Queen, true)));
    assert_eq!(state.piece_at(square("c1")).map(|p| (p.kind, p.moved)), Some((Kind::King, true)));
    assert_eq!(state.key(), key_from_scratch(&state));
    // The queen on d1 gives check to the king on d8.
    assert!(in_check(&state, Color::Black));
    state.unmake(castle, undo);
    assert!(state == before);

    // A rook on a1 is not the partner of this castle. Black castles on its own side.
    let mut rook = from_fen("3k4/8/8/8/8/8/8/R3K2R", Color::White, rules());
    assert!(moves(&mut rook, "e1").iter().all(|m| m.to != square("c1")));
    let mut black = from_fen("q3k2r/8/8/8/8/8/8/4K3", Color::Black, rules());
    let castle = find(&mut black, "e8", "c8");
    assert_eq!(black.rules().castle_partner(Color::Black, castle), Some((square("a8"), square("d8"))));
    // The enemy attacks d1: no castle. The enemy attacks c1: the castle is not legal.
    let mut attacked = from_fen("3rk3/8/8/8/8/8/8/Q3K3", Color::White, rules());
    assert!(moves(&mut attacked, "e1").iter().all(|m| m.special != Special::Castle));
    let mut into_check = from_fen("2r1k3/8/8/8/8/8/8/Q3K3", Color::White, rules());
    assert!(moves(&mut into_check, "e1").iter().all(|m| m.special != Special::Castle));
}

#[test]
fn castling_switched_off_for_one_side() {
    let rules = || Rules::new(SideRules::standard(), SideRules::standard().with_castling(false));
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R";
    let castles = |state: &mut State| legal(state).iter().filter(|m| m.special == Special::Castle).count();
    assert_eq!(castles(&mut from_fen(fen, Color::White, rules())), 2);
    assert_eq!(castles(&mut from_fen(fen, Color::Black, rules())), 0);
    assert_eq!(castles(&mut from_fen(fen, Color::Black, Rules::standard())), 2);
    // Kiwipete has 48 moves for White. Two of them are castles.
    let kiwipete = chrogue_engine::fen::KIWIPETE;
    let off = Rules::new(SideRules::standard().with_castling(false), SideRules::standard());
    assert_eq!(perft(&mut from_fen(kiwipete, Color::White, off), 1), 46);
}

#[test]
fn a_knight_leap_for_the_first_move_only() {
    let rules =
        || white(SideRules::standard().with_atom(Kind::Knight, Atom::leap(&CAMEL, Mode::MoveOrCapture).if_unmoved()));

    // The knight on b1 has not moved: it also leaps to a4, c4, and e2.
    let mut state = from_fen("4k3/8/8/8/8/8/8/1N2K3", Color::White, rules());
    assert_eq!(targets(&mut state, "b1"), squares(&["a3", "a4", "c3", "c4", "d2", "e2"]));
    let mut moved = with_moved(&state, square("b1"), rules());
    assert_eq!(targets(&mut moved, "b1"), squares(&["a3", "c3", "d2"]));
    // After its first move, the knight has only its usual leaps.
    let leap = find(&mut state, "b1", "c4");
    state.make(leap);
    let mut back = state.clone().with_turn(Color::White);
    assert!(!targets(&mut back, "c4").contains(&square("d7")) && targets(&mut back, "c4").contains(&square("d6")));

    // An unmoved knight on d5 gives check to the king on e8 by the leap. A moved one does not.
    let check = from_fen("4k3/8/8/3N4/8/8/8/4K3", Color::Black, rules());
    assert!(in_check(&check, Color::Black));
    assert!(!in_check(&with_moved(&check, square("d5"), rules()), Color::Black));
    // The moved flag of the knight is now in the key. With ordinary rules, it is not.
    assert_ne!(check.key(), with_moved(&check, square("d5"), rules()).key());
    let standard = from_fen("4k3/8/8/3N4/8/8/8/4K3", Color::Black, Rules::standard());
    assert_eq!(standard.key(), with_moved(&standard, square("d5"), Rules::standard()).key());
}

#[test]
fn promotion_one_rank_earlier_for_one_side_only() {
    let early = Promotion { distance: 1, kinds: Promotions::STANDARD };
    let rules = || white(SideRules::standard().with_promotion(Kind::Pawn, Some(early)));

    // The white pawn promotes on rank 7. The black pawn promotes on rank 1 only.
    let mut state = from_fen("7k/8/4P3/8/8/3p4/8/K7", Color::White, rules());
    let promotions = moves(&mut state, "e6");
    assert_eq!(promotions.len(), 4);
    assert!(promotions.iter().all(|m| m.to == square("e7") && m.promo.is_some()));
    let mut black = from_fen("7k/8/4P3/8/8/3p4/8/K7", Color::Black, rules());
    assert_eq!(moves(&mut black, "d3"), vec![Move::new(square("d3"), square("d2"))]);
    let mut black = from_fen("7k/8/4P3/8/8/8/3p4/K7", Color::Black, rules());
    assert_eq!(moves(&mut black, "d2").len(), 4);

    // The new knight on e7 gives check to the king on g8.
    let mut state = from_fen("6k1/8/4P3/8/8/8/8/K7", Color::White, rules());
    let knight = moves(&mut state, "e6").into_iter().find(|m| m.promo == Some(Kind::Knight)).unwrap();
    state.make(knight);
    assert!(in_check(&state, Color::Black));
}
