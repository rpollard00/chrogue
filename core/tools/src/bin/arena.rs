//! Self-play matches between two configurations of the AI.
//!
//! Usage: arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N]
//!              [--seed N] [--max-plies N] [--threads N]
//!
//! A CONFIG is a base and then options, with `,` between them:
//! - Base: `floor1` to `floor8` (a level of the ladder), `reference` (the old TypeScript AI),
//!   or `nodes=N` (the search with a node limit and no noise).
//! - Options: `eval=derived|fixed|blind`, `noise=CP`, `depth=N`, `nodes=N`, `null=0|1`, `lmr=0|1`.
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

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrogue_engine::rng::{Rng, mix};
use chrogue_engine::rules::FLAG_NAMES;
use chrogue_engine::{
    Brain, Color, EvalVariant, Kind, Level, Outcome, Piece, Placement, Rules, SideRules, Square, State, choose_move,
    fen, outcome,
};

struct Config {
    text: String,
    level: Level,
}

fn parse_config(text: &str) -> Config {
    let mut parts = text.split(',');
    let base = parts.next().unwrap_or_default();
    let mut level = if base == "reference" {
        Level::reference()
    } else if let Some(floor) = base.strip_prefix("floor") {
        let floor: usize = floor.parse().expect("the floor must be a number");
        assert!((1..=8).contains(&floor), "the floor must be from 1 to 8");
        Level::floor(floor)
    } else if let Some(nodes) = base.strip_prefix("nodes=") {
        Level::nodes("nodes", nodes.parse().expect("the nodes must be a number"))
    } else {
        panic!("\"{base}\" is not a base of a CONFIG");
    };
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
            "noise" => level.noise_cp = number() as i32,
            "depth" => level.limits.max_depth = number() as u32,
            "nodes" => level.limits.max_nodes = number(),
            "null" => level.options.null_move = number() != 0,
            "lmr" => level.options.lmr = number() != 0,
            "threats" => level.options.threats = number() != 0,
            _ => panic!("\"{name}\" is not an option of a CONFIG"),
        }
    }
    assert!(
        level.brain == Brain::Search || level.limits.max_depth <= 4,
        "the reference AI has no node limit: its depth must be small"
    );
    Config { text: text.to_string(), level }
}

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

// The data of the game: `VALUE` in src/engine/types.ts, `FLOORS` and `generateEnemy` in
// src/game/floors.ts, `baseArmy` and `freeHome` in src/game/army.ts.
const RECRUITS: [Kind; 5] = [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen];
const RECRUIT_VALUE: [u32; 5] = [1, 3, 3, 5, 9];
const RECRUIT_WEIGHT: [f64; 5] = [4.0, 2.0, 2.0, 1.5, 1.0];
const FLOOR_BUDGET: [u32; 8] = [5, 9, 13, 18, 23, 28, 33, 39];
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
fn game_start(rng: &mut Rng, floor: usize) -> Vec<Placement> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Outcome(Outcome),
    /// The game got to the limit of half moves. It is a draw.
    Cap,
}

/// Plays one game. Returns the end and the number of half moves.
fn play(start: &Start, rules: Rules, white: &Level, black: &Level, seed: u64, max_plies: u32) -> (End, u32) {
    let mut state = State::new(&start.pieces, rules).expect("the arena makes valid starts");
    for ply in 0..max_plies {
        if let Some(end) = outcome(&mut state) {
            return (End::Outcome(end), ply);
        }
        let level = if state.turn() == Color::White { white } else { black };
        let result = choose_move(&mut state, level, mix(seed, ply as u64)).expect("the battle continues");
        state.make(result.mv);
    }
    (outcome(&mut state).map_or(End::Cap, End::Outcome), max_plies)
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

fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .map(|i| args.get(i + 1).unwrap_or_else(|| panic!("{name} needs a value")).as_str())
}

fn number(args: &[String], name: &str, default: u64) -> u64 {
    option(args, name).map_or(default, |text| text.parse().unwrap_or_else(|_| panic!("{name} must be a number")))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(a), Some(b)) = (option(&args, "--a"), option(&args, "--b")) else {
        eprintln!(
            "usage: arena --a CONFIG --b CONFIG [--rules all|standard|modified] [--positions N] [--seed N] [--max-plies N] [--threads N]"
        );
        std::process::exit(2);
    };
    let (a, b) = (parse_config(a), parse_config(b));
    let which = option(&args, "--rules").unwrap_or("all");
    assert!(["all", "standard", "modified"].contains(&which), "--rules must be all, standard, or modified");
    let positions = number(&args, "--positions", 20) as usize;
    let seed = number(&args, "--seed", 1);
    let max_plies = number(&args, "--max-plies", 300) as u32;
    let default_threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as u64;
    let threads = number(&args, "--threads", default_threads).max(1) as usize;

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
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<(f64, End, u32)>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else { break };
                    let (white, black) = if job.a_is_white { (&a.level, &b.level) } else { (&b.level, &a.level) };
                    let rules = rule_sets[job.rule_set].rules();
                    let (end, plies) =
                        play(&starts[job.start], rules, white, black, mix(seed, index as u64), max_plies);
                    let winner = match end {
                        End::Outcome(outcome) => outcome.winner(),
                        End::Cap => None,
                    };
                    let points = match winner {
                        None => 0.5,
                        Some(color) if (color == Color::White) == job.a_is_white => 1.0,
                        Some(_) => 0.0,
                    };
                    results.lock().expect("no thread panicked")[index] = Some((points, end, plies));
                }
            });
        }
    });
    let results = results.into_inner().expect("no thread panicked");

    println!("A: {}", a.text);
    println!("B: {}", b.text);
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
            let (points, end, length) = result.expect("each game has a result");
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
                End::Outcome(Outcome::Checkmate { .. }) => 0,
                End::Outcome(Outcome::Stalemate { .. }) => 1,
                End::Outcome(Outcome::Rout { .. }) => 2,
                End::Outcome(Outcome::Bare) => 3,
                End::Outcome(Outcome::Clock) => 4,
                End::Cap => 5,
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
