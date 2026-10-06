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
    pub const OPTIONS: SearchOptions = SearchOptions { null_move: false, lmr: true, threats: true, formation: true };

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

    const fn rung(name: &'static str, max_depth: u32, max_nodes: u64, flaws: Flaws) -> Level {
        let mut level = Level::nodes(name, max_nodes);
        level.limits.max_depth = max_depth;
        level.flaws = flaws;
        level
    }

    /// A level that looks one half move ahead. These levels differ only in their overlook and in
    /// their carelessness.
    const fn shallow(name: &'static str, overlook: u32, careless: u32) -> Level {
        Level::rung(name, 1, 300, Flaws { noise_cp: 150, overlook, careless })
    }

    /// The levels of the AI, from the weakest to the strongest. The first three levels are for
    /// a player who knows only how the pieces move: they do not see each capture, and they
    /// leave pieces where the opponent can capture them.
    pub const LADDER: [Level; 11] = [
        Level::shallow("Recruit", 40, 50),
        Level::shallow("Cadet", 25, 25),
        Level::shallow("Private", 12, 10),
        Level::shallow("Corporal", 0, 0),
        Level::rung("Sergeant", 2, 1_000, Flaws::noise(90)),
        Level::rung("Lieutenant", 3, 2_500, Flaws::noise(50)),
        Level::rung("Captain", 48, 6_000, Flaws::noise(25)),
        Level::rung("Major", 48, 15_000, Flaws::noise(12)),
        Level::rung("Colonel", 48, 36_000, Flaws::noise(6)),
        Level::rung("General", 48, 100_000, Flaws::NONE),
        Level::rung("Marshal", 48, 320_000, Flaws::NONE),
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
