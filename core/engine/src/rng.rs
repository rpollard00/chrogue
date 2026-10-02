//! A small seeded random number generator (SplitMix64). The engine has no dependencies,
//! thus it has its own generator. The same seed gives the same numbers.

/// One step of SplitMix64: moves the state and returns the next number.
pub const fn split_mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Mixes two numbers into one. The search uses it to give each root move its own random number.
pub const fn mix(a: u64, b: u64) -> u64 {
    let mut state = a ^ b.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    split_mix(&mut state)
}

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        split_mix(&mut self.0)
    }

    /// A number from 0 to `bound - 1`. Panics if `bound` is 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        ((self.next_u64() as u128 * bound as u128) >> 64) as u64
    }

    /// A number from 0.0 to 1.0, 1.0 not included.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i as u64 + 1) as usize);
        }
    }
}
