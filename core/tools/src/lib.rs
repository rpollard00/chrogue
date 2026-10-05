//! The parts that the binaries of the tools share: the reference AI, a player that is a level of
//! the engine or the reference AI, a battle between two players, armies in the style of the game,
//! the battles of the game for `balance`, and the command line.

pub mod armies;
pub mod balance;
pub mod cli;
pub mod reference;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrogue_engine::rng::mix;
use chrogue_engine::{
    Color, EvalVariant, Flaws, Level, Limits, Outcome, Placement, Rules, SearchOptions, SearchResult, State,
    choose_move, outcome,
};

/// A player of the tools.
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

    /// The player of a CONFIG: a base and then options, with `,` between them.
    ///
    /// - Base: `levelN` (level `N` of the ladder, where `level1` is the weakest), `reference` (the
    ///   reference AI), or `nodes=N` (the search with a node limit and no flaw).
    /// - Options: `eval=derived|fixed|blind`, `noise=CP`, `overlook=N` and `careless=N` (percent),
    ///   `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`, `threats=0|1`. The reference AI reads only
    ///   `noise` and `depth`.
    ///
    /// Panics with a message if the text is not a CONFIG.
    pub fn parse(text: &str) -> Player {
        let mut parts = text.split(',');
        let base = parts.next().unwrap_or_default();
        let mut player = if base == "reference" {
            Player::reference()
        } else if let Some(number) = base.strip_prefix("level") {
            let number: usize = number.parse().expect("the level must be a number");
            let levels = Level::LADDER.len();
            assert!((1..=levels).contains(&number), "the level must be from 1 to {levels}");
            Player::search(Level::number(number))
        } else if let Some(nodes) = base.strip_prefix("nodes=") {
            Player::search(Level::nodes("nodes", nodes.parse().expect("the nodes must be a number")))
        } else {
            panic!("\"{base}\" is not a base of a CONFIG");
        };
        let level = &mut player.level;
        for part in parts {
            let (name, value) = part.split_once('=').unwrap_or_else(|| panic!("\"{part}\" must be NAME=VALUE"));
            let number = || value.parse::<u64>().unwrap_or_else(|_| panic!("\"{value}\" must be a number"));
            match name {
                "eval" => {
                    level.eval = match value {
                        "derived" => EvalVariant::Derived,
                        "fixed" => EvalVariant::FixedValues,
                        "blind" => EvalVariant::RuleBlind,
                        _ => panic!("\"{value}\" is not an evaluation"),
                    }
                }
                "noise" => level.flaws.noise_cp = number() as i32,
                "overlook" => level.flaws.overlook = number() as u32,
                "careless" => level.flaws.careless = number() as u32,
                "depth" => level.limits.max_depth = number() as u32,
                "nodes" => level.limits.max_nodes = number(),
                "null" => level.options.null_move = number() != 0,
                "lmr" => level.options.lmr = number() != 0,
                "threats" => level.options.threats = number() != 0,
                _ => panic!("\"{name}\" is not an option of a CONFIG"),
            }
        }
        assert!(
            !player.reference || player.level.limits.max_depth <= 4,
            "the reference AI has no node limit: its depth must be small"
        );
        player
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

/// Plays one battle from the pieces of a start, with White to move. Returns the outcome and the
/// number of half moves. The outcome is None if the battle got to `max_plies` half moves.
pub fn play(
    pieces: &[Placement],
    rules: Rules,
    white: &Player,
    black: &Player,
    seed: u64,
    max_plies: u32,
) -> (Option<Outcome>, u32) {
    let mut state = State::new(pieces, rules).expect("the tools make valid starts");
    for ply in 0..max_plies {
        if let Some(end) = outcome(&mut state) {
            return (Some(end), ply);
        }
        let player = if state.turn() == Color::White { white } else { black };
        let result = player.choose_move(&mut state, mix(seed, ply as u64)).expect("the battle continues");
        state.make(result.mv);
    }
    (outcome(&mut state), max_plies)
}

/// Runs `job` for each index from 0 to `count - 1` on `threads` threads. Returns the results in
/// the order of the indexes, thus the result does not depend on the threads.
pub fn parallel<T: Send>(count: usize, threads: usize, job: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<T>>> = Mutex::new((0..count).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= count {
                        break;
                    }
                    let result = job(index);
                    results.lock().expect("no thread panicked")[index] = Some(result);
                }
            });
        }
    });
    let results = results.into_inner().expect("no thread panicked");
    results.into_iter().map(|result| result.expect("each job has a result")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_is_a_base_with_options() {
        let player = Player::parse("level2,overlook=0,depth=3,noise=7");
        let mut level = Level::number(2);
        level.flaws.overlook = 0;
        level.flaws.noise_cp = 7;
        level.limits.max_depth = 3;
        assert_eq!(player, Player::search(level));
        assert_eq!(Player::parse("reference"), Player::reference());
        assert_eq!(Player::parse("nodes=500").level.limits, Limits::nodes(500));
    }

    #[test]
    #[should_panic(expected = "the level must be from 1 to")]
    fn a_config_with_a_level_that_the_ladder_does_not_have_is_refused() {
        Player::parse("level0");
    }

    #[test]
    fn parallel_gives_the_results_in_the_order_of_the_indexes() {
        for threads in [1, 3, 16] {
            assert_eq!(parallel(50, threads, |index| index * index), (0..50).map(|i| i * i).collect::<Vec<_>>());
        }
        assert_eq!(parallel(0, 4, |index| index), Vec::<usize>::new());
    }
}
