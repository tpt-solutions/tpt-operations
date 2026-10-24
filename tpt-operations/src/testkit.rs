//! Minimal deterministic RNG for property tests.
//!
//! Stand-in for `tpt-testkit::property`. It has no shrinking and no generator
//! combinators yet. The seed is fixed, so failures reproduce on every platform.

/// Number of random cases per property. Kept high, as the spec asks.
pub const CASES: usize = 100_000;

/// xorshift64* generator.
pub struct Rng(u64);

impl Rng {
    /// Creates a generator. A zero seed is replaced, because xorshift needs a nonzero state.
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// Next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform value in `0..n`. Panics if `n` is zero.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Uniform value in `lo..=hi`.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (i128::from(hi) - i128::from(lo) + 1) as u64;
        lo.wrapping_add((self.next_u64() % span) as i64)
    }

    /// Fisher-Yates shuffle in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}
