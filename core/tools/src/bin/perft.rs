//! Measures the speed of perft.
//!
//! Usage: perft [DEPTH] [FEN_PIECE_FIELD] [w|b]

use std::time::Instant;

use chrogue_engine::{Color, Rules, fen, perft};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let depth: u32 = args.first().map_or(6, |text| text.parse().expect("DEPTH must be a number"));
    let pieces = args.get(1).map_or(fen::START, String::as_str);
    let turn = if args.get(2).is_some_and(|text| text == "b") { Color::Black } else { Color::White };
    let mut state = fen::from_fen(pieces, turn, Rules::standard()).unwrap_or_else(|error| panic!("{error}"));
    let start = Instant::now();
    let nodes = perft(&mut state, depth);
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "depth {depth}: {nodes} nodes in {seconds:.3} s, {:.1} million nodes per second",
        nodes as f64 / seconds / 1e6
    );
}
