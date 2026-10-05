//! The parts that the binaries of the tools share: the reference AI, and a player that is a
//! level of the engine or the reference AI.

pub mod reference;

use chrogue_engine::{EvalVariant, Flaws, Level, Limits, SearchOptions, SearchResult, State, choose_move};

/// A player of the self-play tool and of `ai speed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Player {
    /// The level of the search. The reference AI reads only its depth limit and its noise.
    pub level: Level,
    /// True for the reference AI (`reference`), false for the search of the engine.
    pub reference: bool,
}

impl Player {
    /// The search of the engine with a level.
    pub const fn search(level: Level) -> Player {
        Player { level, reference: false }
    }

    /// The reference AI (`reference.rs`) at depth 2 with no noise.
    pub const fn reference() -> Player {
        let level = Level {
            name: "reference",
            limits: Limits::depth(2),
            flaws: Flaws::NONE,
            eval: EvalVariant::Derived,
            options: SearchOptions::NONE,
        };
        Player { level, reference: true }
    }

    /// The move of the player for the side that has the move. The state is the same after the call.
    pub fn choose_move(&self, state: &mut State, seed: u64) -> Option<SearchResult> {
        if self.reference {
            reference::choose_move(state, self.level.limits.max_depth, self.level.flaws.noise_cp, seed)
        } else {
            choose_move(state, &self.level, seed)
        }
    }
}
