//! Test helpers for code built on this crate.
//!
//! Enable the `testkit` feature to use them from your own tests:
//!
//! - [`check_reversible`], [`check_commutative`] and [`check_idempotent`] test
//!   one property of your operations on one input, and return a [`Failure`]
//!   that says what went wrong, with the states involved.
//! - `check_replay_deterministic` (with the `serde` feature) replays a log twice
//!   and compares the results.
//! - [`assert_property`] runs a check on many random inputs from a seeded
//!   [`Rng`], and shrinks the first failing input to a smaller one. The seed is
//!   fixed, so failures reproduce on every platform.
//!   [`find_counterexample`] is the same without the panic.
//!
//! These test a property on sampled inputs. They do not prove it.

// Without the `testkit` feature this module is only compiled for the crate's own tests,
// which use a part of it.
#![cfg_attr(not(feature = "testkit"), allow(dead_code))]

use crate::core::{ActorId, OpError, Operation, State};
use alloc::{format, string::String, vec::Vec};
use core::fmt::{self, Debug, Display};

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
        // Widened to u128 so the full i64 range (a span of 2^64) does not wrap to zero.
        let span = (i128::from(hi) - i128::from(lo) + 1) as u128;
        lo.wrapping_add((u128::from(self.next_u64()) % span) as i64)
    }

    /// Fisher-Yates shuffle in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

/// Why a property check did not pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The operation could not be applied to this input, so the property was not
    /// tested. [`find_counterexample`] skips these.
    NotApplicable(OpError),
    /// The property does not hold. The message says how.
    Violated(String),
}

impl Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NotApplicable(error) => write!(f, "not applicable: {error}"),
            Failure::Violated(message) => f.write_str(message),
        }
    }
}

/// The actor used when a check validates an operation.
const ACTOR: ActorId = ActorId(0);

/// Checks that applying `op` and then its inverse restores `base` exactly.
pub fn check_reversible<S, O>(base: &S, op: &O) -> Result<(), Failure>
where
    S: State + PartialEq + Debug,
    O: Operation<S>,
{
    op.validate(base, &ACTOR).map_err(Failure::NotApplicable)?;
    let inverse = op.inverse(base).map_err(Failure::NotApplicable)?;
    let mut state = base.clone();
    op.apply(&mut state).map_err(Failure::NotApplicable)?;
    let after_op = state.clone();
    inverse.apply(&mut state).map_err(|e| {
        Failure::Violated(format!(
            "the inverse failed to apply: {e}\n  state after the op: {after_op:?}"
        ))
    })?;
    if state == *base {
        Ok(())
    } else {
        Err(Failure::Violated(format!(
            "applying the op and then its inverse changed the state\n  before:        {base:?}\n  after the op:  {after_op:?}\n  after inverse: {state:?}"
        )))
    }
}

/// Checks that `a` then `b` and `b` then `a` give the same state from `base`.
///
/// Unlike [`crate::crdt::check_commutative`], the failure says which order
/// failed, or shows the two states that differ.
pub fn check_commutative<S, O>(base: &S, a: &O, b: &O) -> Result<(), Failure>
where
    S: State + PartialEq + Debug,
    O: Operation<S>,
{
    match (run_pair(base, a, b), run_pair(base, b, a)) {
        (Ok(ab), Ok(ba)) if ab == ba => Ok(()),
        (Ok(ab), Ok(ba)) => Err(Failure::Violated(format!(
            "the order matters\n  a then b: {ab:?}\n  b then a: {ba:?}"
        ))),
        (Err(first), Err(_)) => Err(Failure::NotApplicable(first)),
        (Ok(_), Err(error)) => Err(Failure::Violated(format!(
            "a then b works, but b then a fails: {error}"
        ))),
        (Err(error), Ok(_)) => Err(Failure::Violated(format!(
            "b then a works, but a then b fails: {error}"
        ))),
    }
}

/// Applies `first` then `second` to a copy of `base`.
fn run_pair<S, O>(base: &S, first: &O, second: &O) -> Result<S, OpError>
where
    S: State,
    O: Operation<S>,
{
    let mut state = base.clone();
    first.validate(&state, &ACTOR)?;
    first.apply(&mut state)?;
    second.validate(&state, &ACTOR)?;
    second.apply(&mut state)?;
    Ok(state)
}

/// Checks that applying `op` twice gives the same state as applying it once.
pub fn check_idempotent<S, O>(base: &S, op: &O) -> Result<(), Failure>
where
    S: State + PartialEq + Debug,
    O: Operation<S>,
{
    op.validate(base, &ACTOR).map_err(Failure::NotApplicable)?;
    let mut once = base.clone();
    op.apply(&mut once).map_err(Failure::NotApplicable)?;
    let mut twice = once.clone();
    op.validate(&twice, &ACTOR)
        .and_then(|()| op.apply(&mut twice))
        .map_err(|e| Failure::Violated(format!("the second application fails: {e}")))?;
    if once == twice {
        Ok(())
    } else {
        Err(Failure::Violated(format!(
            "applying twice differs from applying once\n  once:  {once:?}\n  twice: {twice:?}"
        )))
    }
}

/// Replays `log` twice and checks that the state and the trace are identical.
#[cfg(feature = "serde")]
pub fn check_replay_deterministic<S, O>(
    initial: &S,
    log: &[u8],
    policy: crate::history::ReplayPolicy,
) -> Result<(), Failure>
where
    S: State + PartialEq + Debug,
    O: Operation<S> + serde::de::DeserializeOwned,
{
    let engine = crate::history::ReplayEngine::new(policy);
    let first = engine.replay::<S, O>(initial.clone(), log);
    let second = engine.replay::<S, O>(initial.clone(), log);
    if first == second {
        Ok(())
    } else {
        Err(Failure::Violated(format!(
            "two replays of the same log differ\n  first:  {first:?}\n  second: {second:?}"
        )))
    }
}

/// A failing input found by [`find_counterexample`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterexample<T> {
    /// Index of the random case that first failed.
    pub case: usize,
    /// The input as generated.
    pub original: T,
    /// The smallest failing input that shrinking reached.
    pub shrunk: T,
    /// What the property reported for `shrunk`.
    pub failure: Failure,
}

/// Most shrink steps tried before giving up. Guards against a shrinker that cycles.
const MAX_SHRINK_STEPS: usize = 10_000;

/// Runs `property` on `cases` inputs from `generate`, seeded with `seed`.
///
/// Returns the first input where it reports [`Failure::Violated`], shrunk as far
/// as `shrink` can take it. Inputs that report [`Failure::NotApplicable`] are
/// skipped. `shrink` lists inputs that are smaller than its argument. It is
/// tried in order, and the first one that still fails is kept.
pub fn find_counterexample<T: Clone>(
    seed: u64,
    cases: usize,
    mut generate: impl FnMut(&mut Rng) -> T,
    shrink: impl Fn(&T) -> Vec<T>,
    property: impl Fn(&T) -> Result<(), Failure>,
) -> Option<Counterexample<T>> {
    let mut rng = Rng::new(seed);
    for case in 0..cases {
        let original = generate(&mut rng);
        let Err(Failure::Violated(message)) = property(&original) else {
            continue;
        };

        let mut shrunk = original.clone();
        let mut failure = Failure::Violated(message);
        'shrinking: for _ in 0..MAX_SHRINK_STEPS {
            for candidate in shrink(&shrunk) {
                if let Err(still @ Failure::Violated(_)) = property(&candidate) {
                    shrunk = candidate;
                    failure = still;
                    continue 'shrinking;
                }
            }
            break;
        }
        return Some(Counterexample {
            case,
            original,
            shrunk,
            failure,
        });
    }
    None
}

/// Like [`find_counterexample`], but panics with a report when a property fails.
///
/// The report names the property, the seed and case, and the shrunk input.
pub fn assert_property<T: Clone + Debug>(
    name: &str,
    seed: u64,
    cases: usize,
    generate: impl FnMut(&mut Rng) -> T,
    shrink: impl Fn(&T) -> Vec<T>,
    property: impl Fn(&T) -> Result<(), Failure>,
) {
    if let Some(found) = find_counterexample(seed, cases, generate, shrink, property) {
        panic!(
            "property `{name}` failed at case {} (seed {seed:#x})\n  shrunk input:   {:?}\n  original input: {:?}\n  {}",
            found.case, found.shrunk, found.original, found.failure
        );
    }
}

/// A shrinker that offers nothing. Use it for inputs that cannot get smaller.
pub fn shrink_none<T>(_: &T) -> Vec<T> {
    Vec::new()
}

/// Shrinks an integer towards zero.
pub fn shrink_i64(value: &i64) -> Vec<i64> {
    if *value == 0 {
        return Vec::new();
    }
    let mut out = alloc::vec![0, value / 2, value - value.signum()];
    out.dedup();
    out.retain(|candidate| candidate != value);
    out
}

/// Shrinks a list: each half, then the list with one element removed.
pub fn shrink_vec<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if items.len() > 1 {
        let (front, back) = items.split_at(items.len() / 2);
        out.push(front.to_vec());
        out.push(back.to_vec());
    }
    for skip in 0..items.len() {
        let mut shorter = items.to_vec();
        shorter.remove(skip);
        out.push(shorter);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_i64_handles_the_full_range_without_dividing_by_zero() {
        let mut rng = Rng::new(7);
        for _ in 0..1_000 {
            rng.range_i64(i64::MIN, i64::MAX);
        }
    }

    #[test]
    fn range_i64_stays_within_bounds() {
        let mut rng = Rng::new(7);
        for _ in 0..1_000 {
            assert!((-3..=3).contains(&rng.range_i64(-3, 3)));
        }
        assert_eq!(rng.range_i64(5, 5), 5);
    }

    use crate::core::toy::{Counter, CounterOp};

    /// An operation whose inverse is wrong on purpose.
    #[derive(Debug, Clone)]
    struct BadInverse;

    impl Operation<Counter> for BadInverse {
        type Inverse = CounterOp;

        fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
            state.value = 99;
            Ok(())
        }

        fn inverse(&self, _state: &Counter) -> Result<CounterOp, OpError> {
            Ok(CounterOp::Set(0)) // not the pre-state
        }

        fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
            Ok(())
        }
    }

    #[test]
    fn check_reversible_passes_a_snapshot_inverse_and_explains_a_bad_one() {
        let base = Counter { value: 5 };
        assert_eq!(check_reversible(&base, &CounterOp::Add(3)), Ok(()));

        let Err(Failure::Violated(message)) = check_reversible(&base, &BadInverse) else {
            panic!("expected a violation");
        };
        assert!(
            message.contains("before:") && message.contains("after inverse:"),
            "{message}"
        );
    }

    #[test]
    fn check_reversible_reports_an_inapplicable_op_as_such() {
        let base = Counter { value: i64::MAX };
        assert!(matches!(
            check_reversible(&base, &CounterOp::Add(1)),
            Err(Failure::NotApplicable(OpError::Bounds(_)))
        ));
    }

    #[test]
    fn check_commutative_distinguishes_add_from_set() {
        let base = Counter { value: 0 };
        assert_eq!(
            check_commutative(&base, &CounterOp::Add(1), &CounterOp::Add(2)),
            Ok(())
        );
        let Err(Failure::Violated(message)) =
            check_commutative(&base, &CounterOp::Set(1), &CounterOp::Set(2))
        else {
            panic!("set does not commute");
        };
        assert!(
            message.contains("a then b") && message.contains("b then a"),
            "{message}"
        );
    }

    #[test]
    fn check_commutative_names_the_order_that_fails() {
        let base = Counter {
            value: i64::MAX - 1,
        };
        // Add(1) then Add(1) overflows in both orders: not applicable.
        assert!(matches!(
            check_commutative(&base, &CounterOp::Add(1), &CounterOp::Add(1)),
            Err(Failure::NotApplicable(_))
        ));
        // Set(0) then Add(5) works. Add(5) then Set(0) overflows on the first step.
        let Err(Failure::Violated(message)) =
            check_commutative(&base, &CounterOp::Set(0), &CounterOp::Add(5))
        else {
            panic!("only one order is valid");
        };
        assert!(message.contains("fails"), "{message}");
    }

    #[test]
    fn check_idempotent_accepts_set_and_refuses_add() {
        let base = Counter { value: 1 };
        assert_eq!(check_idempotent(&base, &CounterOp::Set(7)), Ok(()));
        assert!(matches!(
            check_idempotent(&base, &CounterOp::Add(7)),
            Err(Failure::Violated(_))
        ));
    }

    #[test]
    fn counterexamples_are_shrunk_towards_the_smallest_failing_input() {
        // Fails for any value of 100 or more. The smallest failing one is 100.
        let found = find_counterexample(
            1,
            1_000,
            |rng| rng.range_i64(0, 1_000_000),
            shrink_i64,
            |value| {
                if *value >= 100 {
                    Err(Failure::Violated(format!("{value} is too big")))
                } else {
                    Ok(())
                }
            },
        )
        .expect("a failing case is found");
        assert_eq!(found.shrunk, 100);
        assert!(found.original >= 100);
        assert_eq!(found.failure, Failure::Violated("100 is too big".into()));
    }

    #[test]
    fn shrink_vec_finds_the_one_bad_element() {
        let found = find_counterexample(
            2,
            100,
            |rng| (0..20).map(|_| rng.range_i64(0, 50)).collect::<Vec<_>>(),
            |items| shrink_vec(items),
            |items| {
                if items.contains(&13) {
                    Err(Failure::Violated("has 13".into()))
                } else {
                    Ok(())
                }
            },
        )
        .expect("some list contains 13");
        assert_eq!(found.shrunk, alloc::vec![13]);
    }

    #[test]
    fn passing_properties_find_nothing_and_inapplicable_cases_are_skipped() {
        assert!(
            find_counterexample(3, 500, |rng| rng.below(10), shrink_none, |_| Ok(())).is_none()
        );
        assert!(find_counterexample(
            3,
            500,
            |rng| rng.below(10),
            shrink_none,
            |_| Err(Failure::NotApplicable(OpError::Validation("never".into())))
        )
        .is_none());
    }

    #[test]
    #[should_panic(expected = "property `always fails` failed at case 0 (seed 0x7)")]
    fn assert_property_panics_with_a_report() {
        assert_property(
            "always fails",
            7,
            500,
            |rng| rng.below(10),
            shrink_none,
            |_| Err(Failure::Violated("no".into())),
        );
    }

    #[test]
    fn shrinkers_make_progress_and_stop() {
        assert!(shrink_i64(&0).is_empty());
        assert_eq!(shrink_i64(&1), alloc::vec![0]);
        assert!(shrink_i64(&i64::MIN)
            .iter()
            .all(|candidate| *candidate > i64::MIN));
        assert!(shrink_vec::<u8>(&[]).is_empty());
        assert_eq!(shrink_vec(&[1]), alloc::vec![Vec::<i32>::new()]);
    }
}
