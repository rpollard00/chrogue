//! Measures relics and armies in battles of the game.
//!
//! Usage: balance [--floor LIST] [--player LIST] [--enemy LIST] [--army LIST] [--traits LIST]
//!                [--relics LIST] [--games N] [--gold N] [--seed N] [--max-plies N] [--threads N]
//!                [--out FILE]
//!
//! The tool plays `--games` battles for each combination of the six lists, and writes the data of
//! each battle to `--out` (`balance.json`). `core/README.md` describes the lists, the data, and
//! the report (`tools/report`). The same arguments give the same data.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use chrogue_tools::balance::{Game, Sweep};
use chrogue_tools::cli::{option, threads};
use chrogue_tools::parallel;

const USAGE: &str = "usage: balance [--floor LIST] [--player LIST] [--enemy LIST] [--army LIST] [--traits LIST]
               [--relics LIST] [--games N] [--gold N] [--seed N] [--max-plies N] [--threads N] [--out FILE]
  --floor   floors, 1 to 8: 1-8 (default), 3, 2-4,8
  --player  AI levels of the player, 1 to 11: 3 (default), 1-11, 2,4,6
  --enemy   AI levels of the enemy: floor (default, the level of the floor), 1-11, floor,5
  --army    armies of the player: auto (default), auto+N, auto-N, or letters such as KRNPPPPBB
  --traits  traits of the enemy: floor (default), none, or relic keys with + between them
  --relics  relic sets of the player: none,each (default), pairs, KEY+KEY, KEY+each";

/// The part of the battles that have this result.
fn share(games: &[Game], result: char) -> f64 {
    games.iter().filter(|game| game.result == result).count() as f64 / games.len().max(1) as f64
}

/// The mean of the numbers and the half of its 95% interval, if the numbers are independent.
fn mean_interval(numbers: &[f64]) -> (f64, f64) {
    let n = numbers.len().max(1) as f64;
    let mean = numbers.iter().sum::<f64>() / n;
    let variance = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, 1.96 * (variance / n).sqrt())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sweep = Sweep::parse(&args, &["--threads", "--out"]).unwrap_or_else(|error| {
        eprintln!("{error}\n{USAGE}");
        std::process::exit(2);
    });
    let out = option(&args, "--out").unwrap_or("balance.json");
    let cells = sweep.cells();
    let total = cells.len() * sweep.games;
    eprintln!("{} cells, {} battles in each cell: {total} battles", cells.len(), sweep.games);

    let (done, start) = (AtomicUsize::new(0), Instant::now());
    let step = (total / 20).max(1);
    let games = parallel(total, threads(&args), |index| {
        let cell = cells[index / sweep.games];
        let game = sweep.play(cell, index % sweep.games).unwrap_or_else(|error| {
            eprintln!("The battle of {cell:?} cannot start: {error}");
            std::process::exit(1);
        });
        let count = done.fetch_add(1, Ordering::Relaxed) + 1;
        if count % step == 0 && count < total {
            let seconds = start.elapsed().as_secs_f64();
            eprintln!("{count} of {total} battles, {:.0} s more", seconds * (total - count) as f64 / count as f64);
        }
        game
    });
    let games: Vec<Vec<Game>> = games.chunks(sweep.games).map(<[Game]>::to_vec).collect();

    let command = format!("balance {}", args.join(" "));
    let data = sweep.json(command.trim_end(), &games);
    std::fs::write(out, data.to_string()).unwrap_or_else(|error| {
        eprintln!("Cannot write {out}: {error}");
        std::process::exit(1);
    });

    // One row for each relic set: its battles in all the cells. The difference is to the battles
    // of the first relic set with the same number in the cell that differs only by the relics.
    // The cells with the same floor and army have the same run seeds, thus the interval comes
    // from the mean difference of each run seed.
    let same_seeds = sweep.traits.len() * sweep.players.len() * sweep.enemies.len();
    let of_set = |set: usize| -> Vec<Game> {
        cells.iter().zip(&games).filter(|(cell, _)| cell.relics == set).flat_map(|(_, games)| games.clone()).collect()
    };
    let name = |set: usize| {
        let keys: Vec<&str> = sweep.relics[set].iter().map(|id| id.key()).collect();
        if keys.is_empty() { "none".to_string() } else { keys.join("+") }
    };
    let won = |game: &Game| (game.result == 'w') as u8 as f64;
    let base = of_set(0);
    let mut rows: Vec<(String, Vec<Game>, f64, f64)> = (0..sweep.relics.len())
        .map(|set| {
            let games = of_set(set);
            let differences: Vec<f64> = games.iter().zip(&base).map(|(game, base)| won(game) - won(base)).collect();
            let seeds: Vec<f64> = differences
                .chunks(same_seeds * sweep.games)
                .flat_map(|cells| {
                    let of_seed = |game| cells.iter().skip(game).step_by(sweep.games).sum::<f64>() / same_seeds as f64;
                    (0..sweep.games).map(of_seed).collect::<Vec<_>>()
                })
                .collect();
            let (mean, interval) = mean_interval(&seeds);
            (name(set), games, mean, interval)
        })
        .collect();
    rows[1..].sort_by(|a, b| b.2.total_cmp(&a.2));

    println!("{command}");
    println!("{total} battles in {:.0} s; the data is in {out}", start.elapsed().as_secs_f64());
    println!();
    println!(
        "{:<34} {:>7} {:>6} {:>6} {:>6} {:>16} {:>6} {:>6}",
        "relics",
        "battles",
        "win",
        "draw",
        "loss",
        format!("win - {}", rows[0].0),
        "gold",
        "lost"
    );
    for (index, (name, games, difference, interval)) in rows.iter().enumerate() {
        let percent = |result| format!("{:.1}%", 100.0 * share(games, result));
        let mean = |number: fn(&Game) -> f64| games.iter().map(number).sum::<f64>() / games.len().max(1) as f64;
        let difference =
            if index == 0 { String::new() } else { format!("{:+.1} +-{:.1}", 100.0 * difference, 100.0 * interval) };
        println!(
            "{:<34} {:>7} {:>6} {:>6} {:>6} {:>16} {:>6.1} {:>6.1}",
            name,
            games.len(),
            percent('w'),
            percent('d'),
            percent('l'),
            difference,
            mean(|game| game.gold as f64),
            mean(|game| game.lost as f64)
        );
    }
}
