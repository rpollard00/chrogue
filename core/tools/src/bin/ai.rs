//! Shows data of the AI.
//!
//! Usage:
//! - `ai values`: the derived material values for ordinary chess and for each rule flag.
//! - `ai speed [FEN_PIECE_FIELD] [w|b]`: the nodes per second and the time for one move of each level.
//! - `ai move FEN_PIECE_FIELD w|b WHITE_FLAGS BLACK_FLAGS [NODES]`: the move of the search.
//!   A flag list is flag names with `+` between them, or `-` for no flag.

use std::time::Instant;

use chrogue_engine::eval::{officer_profile, pawn_profile};
use chrogue_engine::rules::FLAG_NAMES;
use chrogue_engine::{Color, EvalTables, Kind, Level, Rules, SideRules, choose_move, fen};

/// A middlegame position of ordinary chess with all the kinds on the board.
const MIDDLEGAME: &str = "r2q1rk1/pp2bppp/2n1bn2/2pp4/3P4/2N1PN2/PP2BPPP/R1BQ1RK1";

fn side(flags: &str) -> SideRules {
    if flags == "-" {
        return SideRules::standard();
    }
    SideRules::from_flags(flags.split('+')).unwrap_or_else(|error| panic!("{error}"))
}

fn values() {
    println!(
        "{:<12} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "rules", "pawn", "knight", "bishop", "rook", "queen", "king"
    );
    let mut rows = vec![("standard".to_string(), SideRules::standard())];
    for name in FLAG_NAMES {
        rows.push((name.to_string(), side(name)));
    }
    rows.push(("all six".to_string(), side(&FLAG_NAMES.join("+"))));
    for (name, rules) in &rows {
        let eval = EvalTables::new(&Rules::new(rules.clone(), SideRules::standard()));
        let v = |kind| eval.value(Color::White, kind);
        println!(
            "{name:<12} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            v(Kind::Pawn),
            v(Kind::Knight),
            v(Kind::Bishop),
            v(Kind::Rook),
            v(Kind::Queen),
            v(Kind::King)
        );
    }
    println!();
    println!("{:<12} {:<8} {:>8} {:>9}", "rules", "kind", "reach", "coverage");
    for (name, rules) in &rows {
        let pawn = pawn_profile(&rules.pawn);
        println!("{name:<12} {:<8} {:>8.3} {:>9.3}", "p", pawn.reach, pawn.coverage);
        for kind in Kind::OFFICERS {
            let profile = officer_profile(rules.atoms(kind));
            println!("{name:<12} {:<8} {:>8.3} {:>9.3}", kind.letter(), profile.reach, profile.coverage);
        }
    }
}

fn speed(args: &[String]) {
    let pieces = args.first().map_or(MIDDLEGAME, String::as_str);
    let turn = if args.get(1).is_some_and(|text| text == "b") { Color::Black } else { Color::White };
    let mut state = fen::from_fen(pieces, turn, Rules::standard()).unwrap_or_else(|error| panic!("{error}"));
    println!(
        "{:<16} {:>9} {:>6} {:>9} {:>6} {:>9} {:>12}",
        "level", "max nodes", "noise", "nodes", "depth", "ms", "nodes/s"
    );
    for level in Level::LADDER.iter().chain([&Level::reference()]) {
        // The best of five runs, because the first run can be slow.
        let mut best = f64::MAX;
        let mut result = None;
        for _ in 0..5 {
            let start = Instant::now();
            result = choose_move(&mut state, level, 1);
            best = best.min(start.elapsed().as_secs_f64());
        }
        let result = result.expect("the position has a legal move");
        let budget =
            if level.limits.max_nodes == u64::MAX { "-".to_string() } else { level.limits.max_nodes.to_string() };
        println!(
            "{:<16} {:>9} {:>6} {:>9} {:>6} {:>9.3} {:>12.0}",
            level.name,
            budget,
            level.noise_cp,
            result.nodes,
            result.depth,
            best * 1e3,
            result.nodes as f64 / best
        );
    }
}

fn square_name(s: u8) -> String {
    format!("{}{}", (b'a' + (s & 7)) as char, (b'1' + (s >> 3)) as char)
}

fn best_move(args: &[String]) {
    let [pieces, turn, white, black, rest @ ..] = args else {
        panic!("usage: ai move FEN_PIECE_FIELD w|b WHITE_FLAGS BLACK_FLAGS [NODES]");
    };
    let turn = if turn == "b" { Color::Black } else { Color::White };
    let nodes = rest.first().map_or(250_000, |text| text.parse().expect("NODES must be a number"));
    let mut state =
        fen::from_fen(pieces, turn, Rules::new(side(white), side(black))).unwrap_or_else(|error| panic!("{error}"));
    let level = Level::nodes("move", nodes);
    match choose_move(&mut state, &level, 1) {
        Some(result) => {
            let pv: Vec<String> = result
                .pv
                .iter()
                .map(|m| {
                    let promo = m.promo.map_or(String::new(), |kind| kind.letter().to_string());
                    format!("{}{}{promo}", square_name(m.from), square_name(m.to))
                })
                .collect();
            println!("score {} depth {} nodes {} pv {}", result.score, result.depth, result.nodes, pv.join(" "));
        }
        None => println!("no legal move"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("values") => values(),
        Some("speed") => speed(&args[1..]),
        Some("move") => best_move(&args[1..]),
        _ => eprintln!("usage: ai values | ai speed [FEN w|b] | ai move FEN w|b WHITE_FLAGS BLACK_FLAGS [NODES]"),
    }
}
