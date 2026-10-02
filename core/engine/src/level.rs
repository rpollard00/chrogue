//! The strength of the AI. A `Level` tells how much the AI searches and how much noise its
//! root scores get. The same search plays all the levels.

use crate::eval::EvalVariant;
use crate::reference;
use crate::search::{Limits, SearchOptions, SearchResult, search};
use crate::state::State;

/// The algorithm of a level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brain {
    /// The search of `search`, with the evaluation of `eval`.
    Search,
    /// The algorithm of the old TypeScript AI. See `reference`.
    Reference,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Level {
    pub name: &'static str,
    pub limits: Limits,
    /// Each root move gets a random bonus from 0 to this number of centipawns.
    pub noise_cp: i32,
    pub eval: EvalVariant,
    pub options: SearchOptions,
    pub brain: Brain,
}

impl Level {
    /// The options of the search that the levels of the game use.
    pub const OPTIONS: SearchOptions = SearchOptions { null_move: false, lmr: true, threats: true };

    /// A level of the search with a node limit and no noise.
    pub const fn nodes(name: &'static str, max_nodes: u64) -> Level {
        Level {
            name,
            limits: Limits::nodes(max_nodes),
            noise_cp: 0,
            eval: EvalVariant::Derived,
            options: Level::OPTIONS,
            brain: Brain::Search,
        }
    }

    const fn floor_level(name: &'static str, max_depth: u32, max_nodes: u64, noise_cp: i32) -> Level {
        let mut level = Level::nodes(name, max_nodes);
        level.limits.max_depth = max_depth;
        level.noise_cp = noise_cp;
        level
    }

    /// The levels of the eight floors of a run, from the weakest to the strongest. Floor 4
    /// and floor 8 are bosses.
    pub const LADDER: [Level; 8] = [
        Level::floor_level("Border Patrol", 1, 300, 150),
        Level::floor_level("Scouts", 2, 1_000, 90),
        Level::floor_level("Garrison", 3, 2_500, 50),
        Level::floor_level("The Warden", 48, 6_000, 25),
        Level::floor_level("Cavalry", 48, 15_000, 12),
        Level::floor_level("Royal Guard", 48, 36_000, 6),
        Level::floor_level("Vanguard", 48, 100_000, 0),
        Level::floor_level("The Black King", 48, 320_000, 0),
    ];

    /// The level of a floor from 1 to 8. Panics for other numbers.
    pub fn floor(floor: usize) -> Level {
        Level::LADDER[floor - 1]
    }

    /// The strongest level.
    pub fn strongest() -> Level {
        Level::LADDER[Level::LADDER.len() - 1]
    }

    /// The old TypeScript AI at its strongest setting of the game before floor 7: depth 2, no noise.
    pub const fn reference() -> Level {
        Level {
            name: "reference",
            limits: Limits::depth(2),
            noise_cp: 0,
            eval: EvalVariant::Derived,
            options: SearchOptions::NONE,
            brain: Brain::Reference,
        }
    }
}

/// Selects a move for the side that has the move. Returns None if the side has no legal move.
/// The state is the same after the call.
///
/// The same state, level, and seed give the same result, if the level has no time limit.
pub fn choose_move(state: &mut State, level: &Level, seed: u64) -> Option<SearchResult> {
    match level.brain {
        Brain::Search => search(state, &level.limits, level.eval, level.options, level.noise_cp, seed),
        Brain::Reference => reference::choose_move(state, level.limits.max_depth, level.noise_cp, seed),
    }
}
