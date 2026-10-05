//! The strength of the AI. A `Level` tells how much the AI searches and which flaws its
//! search has. The same search plays all the levels.

use crate::eval::EvalVariant;
use crate::outcome::outcome;
use crate::search::{Flaws, Limits, SearchOptions, SearchResult, search};
use crate::state::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Level {
    pub name: &'static str,
    pub limits: Limits,
    pub flaws: Flaws,
    pub eval: EvalVariant,
    pub options: SearchOptions,
}

impl Level {
    /// The options of the search that the levels of the game use.
    pub const OPTIONS: SearchOptions = SearchOptions { null_move: false, lmr: true, threats: true };

    /// A level of the search with a node limit and no flaw.
    pub const fn nodes(name: &'static str, max_nodes: u64) -> Level {
        Level {
            name,
            limits: Limits::nodes(max_nodes),
            flaws: Flaws::NONE,
            eval: EvalVariant::Derived,
            options: Level::OPTIONS,
        }
    }

    const fn rung(name: &'static str, max_depth: u32, max_nodes: u64, noise_cp: i32) -> Level {
        let mut level = Level::nodes(name, max_nodes);
        level.limits.max_depth = max_depth;
        level.flaws = Flaws::noise(noise_cp);
        level
    }

    /// The levels of the AI, from the weakest to the strongest.
    pub const LADDER: [Level; 8] = [
        Level::rung("Corporal", 1, 300, 150),
        Level::rung("Sergeant", 2, 1_000, 90),
        Level::rung("Lieutenant", 3, 2_500, 50),
        Level::rung("Captain", 48, 6_000, 25),
        Level::rung("Major", 48, 15_000, 12),
        Level::rung("Colonel", 48, 36_000, 6),
        Level::rung("General", 48, 100_000, 0),
        Level::rung("Marshal", 48, 320_000, 0),
    ];

    /// The level of a number from 1 to `LADDER.len()`. Panics for other numbers.
    pub fn number(number: usize) -> Level {
        Level::LADDER[number - 1]
    }

    /// The strongest level.
    pub fn strongest() -> Level {
        Level::LADDER[Level::LADDER.len() - 1]
    }
}

/// Selects a move for the side that has the move. Returns None if the battle has ended
/// (`outcome` is not None): bare kings, a rout, no legal move, or the limit of the clock. The
/// state is the same after the call.
///
/// The same state, level, and seed give the same result, if the level has no time limit.
pub fn choose_move(state: &mut State, level: &Level, seed: u64) -> Option<SearchResult> {
    if outcome(state).is_some() {
        return None;
    }
    search(state, &level.limits, level.eval, level.options, level.flaws, seed)
}
