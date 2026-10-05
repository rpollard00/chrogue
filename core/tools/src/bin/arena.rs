//! Self-play matches between two configurations of the AI.
//!
//! Usage: arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N]
//!              [--seed N] [--max-plies N] [--threads N]
//!
//! A CONFIG is a base and then options, with `,` between them:
//! - Base: `levelN` (level `N` of the ladder, where `level1` is the weakest), `reference` (the
//!   reference AI), or `nodes=N` (the search with a node limit and no noise).
//! - Options: `eval=derived|fixed|blind`, `noise=CP`, `overlook=N` and `careless=N` (percent),
//!   `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`, `threats=0|1`. The reference AI reads only
//!   `noise` and `depth`.
//!
//! The starts are the start position of chess and `--positions` armies in the style of the
//! game: the base army of the player plus recruits against the enemy army of a floor with
//! the same total value. Each start is played under each rule set of `--rules`.
//!
//! The armies and the rules are not symmetric. Thus each start is played two times from the
//! same position and rules: A as White against B, then B as White against A.
//!
//! A game that gets to `--max-plies` half moves is a draw. The same arguments give the same
//! output: a level with a node limit is deterministic, and all random numbers come from `--seed`.

use chrogue_engine::rng::{Rng, mix};
use chrogue_engine::rules::FLAG_NAMES;
use chrogue_engine::{Color, Outcome, Placement, Rules, SideRules, fen};
use chrogue_tools::armies::game_start;
use chrogue_tools::cli::{number, option, threads};
use chrogue_tools::{Player, parallel, play};

struct RuleSet {
    name: String,
    white: Vec<&'static str>,
    black: Vec<&'static str>,
}

impl RuleSet {
    fn new(white: &[&'static str], black: &[&'static str]) -> RuleSet {
        let list = |flags: &[&str]| if flags.is_empty() { "-".to_string() } else { flags.join("+") };
        RuleSet { name: format!("w:{} b:{}", list(white), list(black)), white: white.to_vec(), black: black.to_vec() }
    }

    fn rules(&self) -> Rules {
        let side = |flags: &[&'static str]| SideRules::from_flags(flags.iter().copied()).expect("the flags are known");
        Rules::new(side(&self.white), side(&self.black))
    }

    fn is_standard(&self) -> bool {
        self.white.is_empty() && self.black.is_empty()
    }
}

/// Ordinary chess, each flag for White only, for Black only, and for the two sides, and
/// eight mixed combinations.
fn rule_sets() -> Vec<RuleSet> {
    let mut sets = vec![RuleSet::new(&[], &[])];
    for flag in FLAG_NAMES {
        sets.push(RuleSet::new(&[flag], &[]));
    }
    for flag in FLAG_NAMES {
        sets.push(RuleSet::new(&[], &[flag]));
    }
    for flag in FLAG_NAMES {
        sets.push(RuleSet::new(&[flag], &[flag]));
    }
    sets.push(RuleSet::new(&["kingKnight", "longLeap"], &[]));
    sets.push(RuleSet::new(&[], &["forcedMarch", "earlyPromo"]));
    sets.push(RuleSet::new(&["sidestep", "backpedal"], &["longLeap"]));
    sets.push(RuleSet::new(&["forcedMarch", "longLeap"], &["kingKnight", "sidestep"]));
    sets.push(RuleSet::new(&["earlyPromo"], &["backpedal", "kingKnight"]));
    sets.push(RuleSet::new(&FLAG_NAMES, &[]));
    sets.push(RuleSet::new(&[], &FLAG_NAMES));
    sets.push(RuleSet::new(&FLAG_NAMES, &FLAG_NAMES));
    sets
}

struct Start {
    pieces: Vec<Placement>,
}

/// The start position of chess and `count` armies of floors 3 to 8.
fn starts(seed: u64, count: usize) -> Vec<Start> {
    let mut rng = Rng::new(mix(seed, 0x57A7));
    let mut list = vec![Start { pieces: fen::placements(fen::START).expect("the start position is valid") }];
    for index in 0..count {
        list.push(Start { pieces: game_start(&mut rng, 3 + index % 6) });
    }
    list
}

struct Job {
    rule_set: usize,
    start: usize,
    a_is_white: bool,
}

#[derive(Clone, Copy, Default)]
struct Tally {
    wins: u32,
    draws: u32,
    losses: u32,
}

impl Tally {
    fn add(&mut self, points: f64) {
        if points == 1.0 {
            self.wins += 1;
        } else if points == 0.0 {
            self.losses += 1;
        } else {
            self.draws += 1;
        }
    }

    fn games(&self) -> u32 {
        self.wins + self.draws + self.losses
    }

    fn score(&self) -> f64 {
        (self.wins as f64 + 0.5 * self.draws as f64) / self.games().max(1) as f64
    }

    /// The half width of the 95% interval of the score, from the variance of the game results.
    fn interval(&self) -> f64 {
        let n = self.games() as f64;
        if n < 2.0 {
            return 1.0;
        }
        let mean = self.score();
        let squares = self.wins as f64 * (1.0 - mean).powi(2)
            + self.draws as f64 * (0.5 - mean).powi(2)
            + self.losses as f64 * mean.powi(2);
        1.96 * (squares / (n - 1.0)).sqrt() / n.sqrt()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(a), Some(b)) = (option(&args, "--a"), option(&args, "--b")) else {
        eprintln!(
            "usage: arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N] [--seed N] [--max-plies N] [--threads N]"
        );
        std::process::exit(2);
    };
    let (a_text, b_text) = (a, b);
    let (a, b) = (Player::parse(a_text), Player::parse(b_text));
    let which = option(&args, "--rules").unwrap_or("all");
    assert!(["all", "standard", "modified"].contains(&which), "--rules must be all, standard, or modified");
    let positions = number(&args, "--positions", 20) as usize;
    let seed = number(&args, "--seed", 1);
    let max_plies = number(&args, "--max-plies", 300) as u32;
    let threads = threads(&args);

    let rule_sets: Vec<RuleSet> = rule_sets()
        .into_iter()
        .filter(|set| match which {
            "standard" => set.is_standard(),
            "modified" => !set.is_standard(),
            _ => true,
        })
        .collect();
    let starts = starts(seed, positions);
    let mut jobs = Vec::new();
    for rule_set in 0..rule_sets.len() {
        for start in 0..starts.len() {
            jobs.push(Job { rule_set, start, a_is_white: true });
            jobs.push(Job { rule_set, start, a_is_white: false });
        }
    }

    // Each game has its own seed, thus the result does not depend on the threads.
    let results = parallel(jobs.len(), threads, |index| {
        let job = &jobs[index];
        let (white, black) = if job.a_is_white { (&a, &b) } else { (&b, &a) };
        let rules = rule_sets[job.rule_set].rules();
        let (end, plies) = play(&starts[job.start].pieces, rules, white, black, mix(seed, index as u64), max_plies);
        let points = match end.and_then(Outcome::winner) {
            None => 0.5,
            Some(color) if (color == Color::White) == job.a_is_white => 1.0,
            Some(_) => 0.0,
        };
        (points, end, plies)
    });

    println!("A: {a_text}");
    println!("B: {b_text}");
    println!(
        "starts: chess and {positions} armies; rule sets: {} ({which}); seed {seed}; a draw at {max_plies} half moves",
        rule_sets.len()
    );
    println!();
    println!("{:<58} {:>5} {:>5} {:>5} {:>5} {:>7}", "rule set (score of A)", "games", "win", "draw", "loss", "score");
    let mut total = Tally::default();
    let mut as_white = Tally::default();
    let mut ends = [0u32; 6];
    let mut plies = 0u64;
    for (index, set) in rule_sets.iter().enumerate() {
        let mut tally = Tally::default();
        for (job, result) in jobs.iter().zip(&results) {
            let &(points, end, length) = result;
            if job.rule_set != index {
                continue;
            }
            tally.add(points);
            total.add(points);
            if job.a_is_white {
                as_white.add(points);
            }
            plies += length as u64;
            ends[match end {
                Some(Outcome::Checkmate { .. }) => 0,
                Some(Outcome::Stalemate { .. }) => 1,
                Some(Outcome::Rout { .. }) => 2,
                Some(Outcome::Bare) => 3,
                Some(Outcome::Clock) => 4,
                None => 5,
            }] += 1;
        }
        println!(
            "{:<58} {:>5} {:>5} {:>5} {:>5} {:>6.1}%",
            set.name,
            tally.games(),
            tally.wins,
            tally.draws,
            tally.losses,
            100.0 * tally.score()
        );
    }
    println!();
    println!("total: {} games, A wins {}, draws {}, A losses {}", total.games(), total.wins, total.draws, total.losses);
    println!(
        "score of A: {:.1}% (95% interval {:.1}% to {:.1}%)",
        100.0 * total.score(),
        100.0 * (total.score() - total.interval()),
        100.0 * (total.score() + total.interval())
    );
    println!("score of A as White: {:.1}% in {} games", 100.0 * as_white.score(), as_white.games());
    println!(
        "ends: checkmate {}, stalemate {}, rout {}, bare {}, clock {}, cap {}; mean length {:.0} half moves",
        ends[0],
        ends[1],
        ends[2],
        ends[3],
        ends[4],
        ends[5],
        plies as f64 / total.games().max(1) as f64
    );
}
