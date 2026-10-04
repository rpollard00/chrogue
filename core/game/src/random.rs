//! The random numbers of the game. Each random result of a run comes from a stream of its own:
//! a `Dice` from the seed of the run, the kind of the result, and two numbers. Thus a result
//! does not depend on the results before it, and the same run seed gives the same run.

use crate::chess::{Rng, mix};

/// The kind of a random result of a run. Each kind has its own streams.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stream {
    /// The enemy of a floor: `(floor, 0)`.
    Enemy,
    /// The reward before a floor: `(floor, 0)`.
    Draft,
    /// The shop before a floor: `(floor, the number of rerolls)`.
    Shop,
    /// The start of the run: `(0, 0)`.
    Start,
    /// The move of the AI: `(floor, the number of moves that the battle played)`.
    Ai,
    /// The squares of the enemy of a floor: `(floor, 0)`.
    Formation,
}

impl Stream {
    const fn salt(self) -> u64 {
        match self {
            Stream::Enemy => 0x454E_454D_5900_0001,
            Stream::Draft => 0x4452_4146_5400_0002,
            Stream::Shop => 0x5348_4F50_0000_0003,
            Stream::Start => 0x5354_4152_5400_0004,
            Stream::Ai => 0x4149_0000_0000_0005,
            Stream::Formation => 0x464F_524D_0000_0006,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Dice(Rng);

impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(Rng::new(seed))
    }

    /// The dice of one random result of a run. The same four numbers give the same dice.
    pub fn stream(seed: u64, stream: Stream, a: u64, b: u64) -> Dice {
        Dice::new(mix(mix(mix(seed, stream.salt()), a), b))
    }

    /// A number from 0.0 to 1.0, 1.0 not included.
    pub fn unit(&mut self) -> f64 {
        (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A seed for the AI.
    pub fn seed(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// A number from 0 to `n - 1`. Returns 0 if `n` is 0.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.unit() * n as f64) as usize).min(n.saturating_sub(1))
    }

    /// The items in a random order (a Fisher-Yates shuffle from the last item).
    pub fn shuffle<T>(&mut self, mut items: Vec<T>) -> Vec<T> {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
        items
    }

    /// Up to `n` different items. The chance of each item is its part of the weights that remain.
    pub fn pick_weighted<T>(&mut self, pool: Vec<(T, f64)>, n: usize) -> Vec<T> {
        let mut rest = pool;
        let mut picked = Vec::new();
        while picked.len() < n && !rest.is_empty() {
            let mut roll = self.unit() * rest.iter().map(|(_, weight)| weight).sum::<f64>();
            let i = rest
                .iter()
                .position(|(_, weight)| {
                    roll -= weight;
                    roll < 0.0
                })
                .unwrap_or(rest.len() - 1);
            picked.push(rest.remove(i).0);
        }
        picked
    }
}
