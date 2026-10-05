//! Armies in the style of the game, for the starts of the tools.
//!
//! The data is that of the crate `chrogue-game`: `gold_value`, `enemy_weight`, and `FLOORS` in
//! content.rs, and `generate_enemy`, `base_army`, and `free_home` in run.rs. The enemy here has
//! each kind on its home square of chess. The game selects the squares at the start of a battle
//! (`formation.rs`).

use chrogue_engine::rng::Rng;
use chrogue_engine::{Color, Kind, Piece, Placement, Square};

const RECRUITS: [Kind; 5] = [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen];
const RECRUIT_VALUE: [u32; 5] = [1, 3, 3, 5, 9];
const RECRUIT_WEIGHT: [f64; 5] = [4.0, 2.0, 2.0, 1.5, 1.0];
/// The total piece value of the enemy army of each floor.
pub const FLOOR_BUDGET: [u32; 8] = [5, 9, 13, 18, 23, 28, 33, 39];
const BACK_HOMES: [Square; 8] = [3, 2, 5, 1, 6, 0, 7, 4];
const FRONT_HOMES: [Square; 8] = [12, 11, 13, 10, 14, 9, 15, 8];
const BASE_ARMY: [(Kind, Square); 7] = [
    (Kind::King, 4),
    (Kind::Rook, 0),
    (Kind::Knight, 6),
    (Kind::Pawn, 10),
    (Kind::Pawn, 11),
    (Kind::Pawn, 12),
    (Kind::Pawn, 13),
];

/// Selects kinds until the budget has no kind that it can pay for. `counts` has the kinds
/// that the army has before the call.
fn recruit(rng: &mut Rng, floor: usize, mut budget: u32, counts: &mut [u32; 5], mut room: usize) -> Vec<Kind> {
    let caps = [8, 2, 2, 2, if floor >= 5 { 1 } else { 0 }];
    let mut kinds = Vec::new();
    while room > 0 {
        let pool: Vec<usize> = (0..5).filter(|&i| counts[i] < caps[i] && RECRUIT_VALUE[i] <= budget).collect();
        if pool.is_empty() {
            break;
        }
        let mut roll = rng.unit() * pool.iter().map(|&i| RECRUIT_WEIGHT[i]).sum::<f64>();
        let mut pick = pool[pool.len() - 1];
        for &i in &pool {
            roll -= RECRUIT_WEIGHT[i];
            if roll < 0.0 {
                pick = i;
                break;
            }
        }
        counts[pick] += 1;
        budget -= RECRUIT_VALUE[pick];
        kinds.push(RECRUITS[pick]);
        room -= 1;
    }
    kinds
}

/// The base army of the player plus recruits, against the enemy army of a floor. The value
/// of the army of the player is the budget of the floor, or 12 (the base army) if the budget is less.
pub fn game_start(rng: &mut Rng, floor: usize) -> Vec<Placement> {
    let budget = FLOOR_BUDGET[floor - 1];
    let mut army: Vec<(Kind, Square)> = BASE_ARMY.to_vec();
    let mut counts = [4, 1, 0, 1, 0];
    for kind in recruit(rng, floor, budget.saturating_sub(12), &mut counts, 16 - BASE_ARMY.len()) {
        let homes = if kind == Kind::Pawn { [FRONT_HOMES, BACK_HOMES] } else { [BACK_HOMES, FRONT_HOMES] };
        let home = homes.as_flattened().iter().find(|&&s| army.iter().all(|unit| unit.1 != s));
        army.push((kind, *home.expect("an army of less than 16 units has a free home")));
    }

    let mut enemy: Vec<(Kind, Square)> = vec![(Kind::King, 60)];
    let mut counts = [0; 5];
    let kinds = recruit(rng, floor, budget, &mut counts, 15);
    let (mut rooks, mut knights, mut bishops) = ([56, 63], [57, 62], [58, 61]);
    rng.shuffle(&mut rooks);
    rng.shuffle(&mut knights);
    rng.shuffle(&mut bishops);
    let pawns = [52, 51, 53, 50, 54, 49, 55, 48];
    for kind in RECRUITS {
        let squares: &[Square] = match kind {
            Kind::Pawn => &pawns,
            Kind::Knight => &knights,
            Kind::Bishop => &bishops,
            Kind::Rook => &rooks,
            _ => &[59],
        };
        let count = kinds.iter().filter(|&&k| k == kind).count();
        enemy.extend(squares[..count].iter().map(|&s| (kind, s)));
    }

    let mut pieces = Vec::new();
    for (color, units) in [(Color::White, army), (Color::Black, enemy)] {
        for (kind, square) in units {
            let piece = Piece { id: pieces.len() as u16, kind, color, moved: false };
            pieces.push(Placement { piece, square });
        }
    }
    pieces
}
