//! Measures the levels of the AI in battles between armies in the style of the game.
//!
//! Usage:
//! - `levels mistakes [--white CONFIG] [--games N] [--judge NODES] [--max-plies N] [--seed N]
//!   [--threads N] CONFIG...`
//! - `levels floors --player CONFIG [--games N] [--max-plies N] [--seed N] [--threads N]
//!   CONFIG CONFIG CONFIG CONFIG CONFIG CONFIG CONFIG CONFIG`
//!
//! A CONFIG is that of `arena` (`Player::parse`). The battles have the rules of ordinary chess.
//! The same arguments give the same output: all random numbers come from `--seed`.
//!
//! `mistakes` counts the mistakes of each CONFIG. The CONFIG plays Black, the enemy, in `--games`
//! battles against `--white`. The battles are on the armies of floors 2 to 6, and each CONFIG
//! gets the same battles. A search with `--judge` nodes and no flaw, the judge, gives a score
//! to the position before and after each move of Black. The columns are:
//!
//! - `moves`: the moves that the judge scored. A move has no score if the judge sees a forced
//!   win or a forced loss before it, or if the move ends the battle.
//! - `>=100`, `>=300`: the move is this number of centipawns, or more, below the best move.
//! - `hung`: the move is 250 centipawns or more below the best move, and the best reply
//!   captures an officer.
//! - `lost`: after the move, the judge sees a forced win of White.
//! - `not taken`: the best move captures an officer, and the move is 250 centipawns or more
//!   below it. The column also gives the number of such best moves.
//! - `wins`, `draws`: the battles that Black won, and the draws.
//!
//! A weak White (for example `--white level4,noise=600`) leaves more officers where Black can
//! capture them, thus `not taken` has more data.
//!
//! `floors` plays a run floor by floor. `--player` plays White in `--games` battles on the
//! armies of each floor, against the CONFIG of that floor. It prints the wins, the draws, and
//! the losses of the player on each floor.

use chrogue_engine::rng::{Rng, mix};
use chrogue_engine::{Color, Kind, Level, MATE_BOUND, Move, Outcome, Rules, State, choose_move, outcome};
use chrogue_tools::armies::{FLOOR_BUDGET, game_start};
use chrogue_tools::cli::{number, option, positionals, threads};
use chrogue_tools::{Player, parallel, play};

/// A move is a large mistake if it is this number of centipawns below the best move.
const LARGE: i32 = 250;

const USAGE: &str = "usage: levels mistakes [--white CONFIG] [--games N] [--judge NODES] [--max-plies N] [--seed N] [--threads N] CONFIG...
       levels floors --player CONFIG [--games N] [--max-plies N] [--seed N] [--threads N] CONFIG x8";

#[derive(Clone, Copy, Default)]
struct Mistakes {
    moves: u32,
    loss_100: u32,
    loss_300: u32,
    hung: u32,
    lost: u32,
    chances: u32,
    not_taken: u32,
    wins: u32,
    draws: u32,
}

impl Mistakes {
    fn add(&mut self, other: &Mistakes) {
        self.moves += other.moves;
        self.loss_100 += other.loss_100;
        self.loss_300 += other.loss_300;
        self.hung += other.hung;
        self.lost += other.lost;
        self.chances += other.chances;
        self.not_taken += other.not_taken;
        self.wins += other.wins;
        self.draws += other.draws;
    }
}

fn captures_officer(state: &State, m: Move) -> bool {
    state.piece_at(m.to).is_some_and(|piece| !matches!(piece.kind, Kind::Pawn | Kind::King))
}

/// The armies of a battle of `mistakes`: the floors go from 2 to 6 with the number of the game.
fn mistakes_start(seed: u64, game: u64) -> State {
    let mut rng = Rng::new(mix(mix(seed, 0xA7), game));
    let pieces = game_start(&mut rng, 2 + (game % 5) as usize);
    State::new(&pieces, Rules::standard()).expect("the tools make valid starts")
}

/// One battle of `mistakes`: the mistakes of Black, and the result.
fn judged_battle(
    black: &Player,
    white: &Player,
    judge: &Level,
    mut state: State,
    seed: u64,
    max_plies: u32,
) -> Mistakes {
    let mut tally = Mistakes::default();
    let mut end = None;
    for ply in 0..max_plies {
        end = outcome(&mut state);
        if end.is_some() {
            break;
        }
        let move_seed = mix(seed, ply as u64);
        if state.turn() == Color::White {
            let result = white.choose_move(&mut state, move_seed).expect("the battle continues");
            state.make(result.mv);
            continue;
        }
        let best = choose_move(&mut state, judge, 1).expect("the battle continues");
        let played = black.choose_move(&mut state, move_seed).expect("the battle continues").mv;
        let chance = captures_officer(&state, best.mv);
        state.make(played);
        if best.score.abs() >= MATE_BOUND || outcome(&mut state).is_some() {
            continue;
        }
        let reply = choose_move(&mut state, judge, 1).expect("the battle continues");
        tally.moves += 1;
        tally.chances += chance as u32;
        if played == best.mv {
            continue;
        }
        if reply.score >= MATE_BOUND {
            tally.lost += 1;
            continue;
        }
        let loss = best.score + reply.score;
        tally.loss_100 += (loss >= 100) as u32;
        tally.loss_300 += (loss >= 300) as u32;
        if loss >= LARGE {
            tally.not_taken += chance as u32;
            tally.hung += (!chance && captures_officer(&state, reply.mv)) as u32;
        }
    }
    match end.or_else(|| outcome(&mut state)).map(Outcome::winner) {
        Some(Some(Color::Black)) => tally.wins += 1,
        Some(Some(Color::White)) => {}
        Some(None) | None => tally.draws += 1,
    }
    tally
}

fn mistakes(args: &[String]) {
    let configs = positionals(args);
    if configs.is_empty() {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let players: Vec<Player> = configs.iter().map(|text| Player::parse(text)).collect();
    let white_text = option(args, "--white").unwrap_or("nodes=20000,noise=40");
    let white = Player::parse(white_text);
    let games = number(args, "--games", 100).max(1);
    let judge_nodes = number(args, "--judge", 30_000);
    let judge = Level::nodes("judge", judge_nodes);
    let max_plies = number(args, "--max-plies", 140) as u32;
    let seed = number(args, "--seed", 1);

    let battles = parallel(players.len() * games as usize, threads(args), |index| {
        let game = index as u64 % games;
        let player = &players[index / games as usize];
        let battle_seed = mix(mix(seed, 0x5EED), game);
        judged_battle(player, &white, &judge, mistakes_start(seed, game), battle_seed, max_plies)
    });

    println!("White: {white_text}; {games} battles on floors 2 to 6; judge: {judge_nodes} nodes; seed {seed}");
    println!();
    println!(
        "{:<28} {:>6} {:>7} {:>7} {:>7} {:>7} {:>16} {:>5} {:>5}",
        "Black", "moves", ">=100", ">=300", "hung", "lost", "not taken", "wins", "draws"
    );
    for (text, battles) in configs.iter().zip(battles.chunks(games as usize)) {
        let mut tally = Mistakes::default();
        battles.iter().for_each(|battle| tally.add(battle));
        let share = |count: u32, of: u32| 100.0 * count as f64 / of.max(1) as f64;
        let of_moves = |count: u32| format!("{:.1}%", share(count, tally.moves));
        let not_taken =
            format!("{} of {} ({:.0}%)", tally.not_taken, tally.chances, share(tally.not_taken, tally.chances));
        println!(
            "{:<28} {:>6} {:>7} {:>7} {:>7} {:>7} {:>16} {:>5} {:>5}",
            text,
            tally.moves,
            of_moves(tally.loss_100),
            of_moves(tally.loss_300),
            of_moves(tally.hung),
            of_moves(tally.lost),
            not_taken,
            tally.wins,
            tally.draws
        );
    }
}

fn floors(args: &[String]) {
    let configs = positionals(args);
    let Some(player_text) = option(args, "--player").filter(|_| configs.len() == FLOOR_BUDGET.len()) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let player = Player::parse(player_text);
    let enemies: Vec<Player> = configs.iter().map(|text| Player::parse(text)).collect();
    let games = number(args, "--games", 100).max(1);
    let max_plies = number(args, "--max-plies", 240) as u32;
    let seed = number(args, "--seed", 1);

    let winners = parallel(enemies.len() * games as usize, threads(args), |index| {
        let (floor, game) = (index / games as usize + 1, index as u64 % games);
        let battle_seed = mix(mix(seed, floor as u64), game);
        let pieces = game_start(&mut Rng::new(mix(battle_seed, 0xA7)), floor);
        let (end, _) = play(&pieces, Rules::standard(), &player, &enemies[floor - 1], battle_seed, max_plies);
        end.and_then(Outcome::winner)
    });

    println!("player: {player_text}; {games} battles on each floor; seed {seed}");
    println!();
    println!("{:<6} {:>6}  {:<28} {:>5} {:>5} {:>5} {:>7}", "floor", "budget", "enemy", "win", "draw", "loss", "win %");
    for (index, winners) in winners.chunks(games as usize).enumerate() {
        let count = |winner: Option<Color>| winners.iter().filter(|&&w| w == winner).count();
        let wins = count(Some(Color::White));
        println!(
            "{:<6} {:>6}  {:<28} {:>5} {:>5} {:>5} {:>6.0}%",
            index + 1,
            FLOOR_BUDGET[index],
            configs[index],
            wins,
            count(None),
            count(Some(Color::Black)),
            100.0 * wins as f64 / games.max(1) as f64
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mistakes") => mistakes(&args[1..]),
        Some("floors") => floors(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}
