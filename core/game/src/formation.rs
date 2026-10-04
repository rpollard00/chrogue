//! The squares of an enemy army at the start of a battle: its formation.
//!
//! The army of a floor has kinds and no squares. The player sets the homes of the units in the
//! camp. If each kind had a set square, the player could learn an arrangement that wins on the
//! first move, such as a rook on an open file against a king behind its pawns. Thus the battle
//! selects the formation against the army that it meets: it makes formations from the dice of
//! the floor and takes the first one with no flaw.
//!
//! The engine finds the flaws with the rules of the battle. Thus a relic or a trait that changes
//! a movement needs no code here.

use crate::battle::enemy_placements;
use crate::chess::{self, Color, Kind, Placement, Square, Tables};
use crate::content;
use crate::random::{Dice, Stream};
use crate::run::{EnemyPiece, Run};

/// The number of formations that a battle makes. If each one has a flaw, the battle takes the
/// one with the smallest flaw.
const TRIES: usize = 24;
/// The first formations are shielded. The others are loose.
const SHIELDED_TRIES: usize = 8;

const KING_HOME: Square = 60;
/// The other squares of rank 8, from the king to the corners. The two orders start on
/// different sides of the king.
const OFFICER_HOMES: [[Square; 7]; 2] = [[59, 61, 58, 62, 57, 63, 56], [61, 59, 62, 58, 63, 57, 56]];

/// The reason that a formation is not sound, from the smallest flaw to the largest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Flaw {
    /// The player has a forced win in two moves.
    WinInTwo,
    /// The player wins with the first move.
    WinInOne,
    /// The enemy king starts in check.
    Check,
}

/// The flaw of a start position with White to move, or None if the position is sound.
pub fn flaw(pieces: &[Placement], tables: &Tables) -> Result<Option<Flaw>, String> {
    let mut state = chess::new_state(pieces, tables)?;
    Ok(if chess::in_check(&state, Color::Black) {
        Some(Flaw::Check)
    } else if chess::wins_in(&mut state, 1) {
        Some(Flaw::WinInOne)
    } else if chess::wins_in(&mut state, 2) {
        Some(Flaw::WinInTwo)
    } else {
        None
    })
}

/// The pieces of the kinds on the squares, in the order of the kinds. None if the army has more
/// officers or more pawns than squares.
fn on_squares(kinds: &[Kind], king: Square, officers: [Square; 7], pawns: [Square; 8]) -> Option<Vec<EnemyPiece>> {
    let (mut officers, mut pawns) = (officers.into_iter(), pawns.into_iter());
    let piece = |&kind: &Kind| {
        let square = match kind {
            Kind::King => Some(king),
            Kind::Pawn => pawns.next(),
            _ => officers.next(),
        };
        Some(EnemyPiece { kind, square: square? })
    };
    kinds.iter().map(piece).collect()
}

/// The king is on e8. The officers are next to the king, in a random order. The pawns are in
/// front of the king first, then in front of the officers, then from the officers to the corners.
fn shielded(kinds: &[Kind], dice: &mut Dice) -> Option<Vec<EnemyPiece>> {
    let mut homes = OFFICER_HOMES[dice.below(2)];
    let pawns = std::array::from_fn(|i| if i == 0 { KING_HOME - 8 } else { homes[i - 1] - 8 });
    let officers = kinds.iter().filter(|&&kind| !matches!(kind, Kind::King | Kind::Pawn)).count().min(homes.len());
    let nearest = dice.shuffle(homes[..officers].to_vec());
    homes[..officers].copy_from_slice(&nearest);
    on_squares(kinds, KING_HOME, homes, pawns)
}

/// The king and the officers are on random squares of rank 8, and the pawns are on random
/// squares of rank 7. Thus a king with no pawn can leave a file that the player has.
fn loose(kinds: &[Kind], dice: &mut Dice) -> Option<Vec<EnemyPiece>> {
    let back = dice.shuffle((56..64).collect());
    let pawns = dice.shuffle((48..56).collect());
    on_squares(kinds, back[0], std::array::from_fn(|i| back[i + 1]), std::array::from_fn(|i| pawns[i]))
}

/// The formation of the enemy army of the run against `army`, the pieces of the player at the
/// start of the battle. The same run and the same army give the same formation.
///
/// An error if the army does not have one king, if the kinds do not fit on the last two ranks, or
/// if the rules of the run are not valid.
pub fn place(run: &Run, kinds: &[Kind], army: &[Placement]) -> Result<Vec<EnemyPiece>, String> {
    if kinds.iter().filter(|&&kind| kind == Kind::King).count() != 1 {
        return Err("The enemy must have one king".into());
    }
    let tables = chess::tables(content::rules_for(&run.relics), content::rules_for(&run.enemy.traits))?;
    let dice = &mut Dice::stream(run.seed, Stream::Formation, run.floor as u64, 0);
    let mut best: Option<(Flaw, Vec<EnemyPiece>)> = None;
    for n in 0..TRIES {
        let pieces = if n < SHIELDED_TRIES { shielded(kinds, dice) } else { loose(kinds, dice) };
        let pieces = pieces.ok_or("The enemy has at most 7 officers and at most 8 pawns")?;
        let board: Vec<Placement> = army.iter().copied().chain(enemy_placements(&pieces)).collect();
        let Some(flaw) = flaw(&board, &tables)? else { return Ok(pieces) };
        if best.as_ref().is_none_or(|(least, _)| flaw < *least) {
            best = Some((flaw, pieces));
        }
    }
    best.map(|(_, pieces)| pieces).ok_or_else(|| "The battle made no formation".into())
}
