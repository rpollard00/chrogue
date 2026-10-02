//! The random numbers of a session. All randomness of the game comes from one `Dice`, thus the
//! same seed and the same commands give the same game.

use crate::chess::Rng;

#[derive(Clone, Debug)]
pub struct Dice(Rng);

impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(Rng::new(seed))
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

    /// The algorithm of `shuffle` in `src/game/random.ts`.
    pub fn shuffle<T>(&mut self, mut items: Vec<T>) -> Vec<T> {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
        items
    }

    /// Up to `n` different items: the algorithm of `pickWeighted` in `src/game/random.ts`.
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
