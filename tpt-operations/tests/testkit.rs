//! The public `testkit`, used the way a downstream crate would use it.

#![cfg(feature = "testkit")]

#[allow(dead_code)]
mod common;

use common::{Counter, CounterOp};
use tpt_operations::{
    core::{ActorId, OpError, Operation, State},
    testkit::{
        assert_property, check_commutative, check_idempotent, check_reversible,
        find_counterexample, shrink_i64, shrink_none, Failure, Rng,
    },
};

fn random_op(rng: &mut Rng) -> CounterOp {
    if rng.below(2) == 0 {
        CounterOp::Add(rng.range_i64(-500, 500))
    } else {
        CounterOp::Set(rng.range_i64(-1_000, 1_000))
    }
}

#[test]
fn counter_ops_are_reversible_on_random_inputs() {
    assert_property(
        "counter ops are reversible",
        0xFEED,
        5_000,
        |rng| (rng.range_i64(-1_000, 1_000), random_op(rng)),
        shrink_none,
        |(value, op)| check_reversible(&Counter { value: *value }, op),
    );
}

/// Add with an arithmetic inverse: the mistake the crate warns about.
#[derive(Debug, Clone)]
struct DriftingAdd(i64);

impl Operation<Counter> for DriftingAdd {
    type Inverse = DriftingAdd;

    fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
        // Saturates, so the arithmetic inverse is wrong near the limits.
        state.value = state.value.saturating_add(self.0);
        Ok(())
    }

    fn inverse(&self, _state: &Counter) -> Result<DriftingAdd, OpError> {
        Ok(DriftingAdd(-self.0))
    }

    fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

#[test]
fn the_testkit_catches_an_arithmetic_inverse_and_shrinks_the_input() {
    let found = find_counterexample(
        0xBAD,
        2_000,
        |rng| {
            (
                rng.range_i64(i64::MAX - 1_000, i64::MAX),
                rng.range_i64(1, 5_000),
            )
        },
        |(value, delta)| {
            let mut out: Vec<(i64, i64)> =
                shrink_i64(delta).into_iter().map(|d| (*value, d)).collect();
            out.extend(shrink_i64(value).into_iter().map(|v| (v, *delta)));
            out
        },
        |(value, delta)| check_reversible(&Counter { value: *value }, &DriftingAdd(*delta)),
    )
    .expect("saturation near i64::MAX breaks the arithmetic inverse");

    let Failure::Violated(message) = &found.failure else {
        panic!("expected a violation");
    };
    assert!(message.contains("after inverse"), "{message}");
    // The shrunk delta is no bigger than the original one.
    assert!(found.shrunk.1 <= found.original.1);
}

#[test]
fn commutative_and_idempotent_checks_work_on_a_real_state() {
    let base = Counter { value: 10 };
    assert_eq!(
        check_commutative(&base, &CounterOp::Add(1), &CounterOp::Add(2)),
        Ok(())
    );
    assert!(check_commutative(&base, &CounterOp::Set(1), &CounterOp::Add(2)).is_err());
    assert_eq!(check_idempotent(&base, &CounterOp::Set(3)), Ok(()));
    assert!(check_idempotent(&base, &CounterOp::Add(3)).is_err());
    assert!(base.clone().apply(&CounterOp::Add(1)).is_ok());
}

#[cfg(feature = "serde")]
#[test]
fn replay_is_deterministic_for_a_real_log() {
    use tpt_operations::{
        history::{replay::encode_entry, ReplayPolicy},
        testkit::check_replay_deterministic,
    };

    let mut log = Vec::new();
    for (i, op) in [
        CounterOp::Add(1),
        CounterOp::Set(5),
        CounterOp::Add(i64::MAX),
    ]
    .iter()
    .enumerate()
    {
        encode_entry(ActorId(i as u64), op, &mut log).unwrap();
    }
    for policy in [ReplayPolicy::FailFast, ReplayPolicy::SkipInvalid] {
        assert_eq!(
            check_replay_deterministic::<Counter, CounterOp>(&Counter::default(), &log, policy),
            Ok(())
        );
    }
}
