//! CRDT compatibility: marker traits, Lamport timestamps, and property checks.
//!
//! The check functions here test one pair of operations on one base state.
//! They do not prove a property. Use them in tests to confirm that a
//! [`CrdtCompatible`] declaration is true on sampled inputs.

use crate::core::{ActorId, Operation, State};

/// Declares which algebraic properties an operation type guarantees.
///
/// The markers are per type, not per variant. A type that mixes commutative
/// and non-commutative variants must declare `false`. A declaration is only
/// valid if it holds for every value of the type.
pub trait CrdtCompatible<S: State>: Operation<S> {
    /// `a` then `b` gives the same state as `b` then `a`, for every base state.
    const IS_COMMUTATIVE: bool;
    /// Grouping does not change the result. Sequential application always
    /// has this property, so the marker mainly matters for batch- and
    /// merge-level grouping.
    const IS_ASSOCIATIVE: bool;
    /// Applying `a` twice gives the same state as applying it once.
    const IS_IDEMPOTENT: bool;
}

/// Lamport timestamp with an actor tie-break.
///
/// Ordering compares the Lamport counter first, then the actor. Any two
/// distinct stamps have a deterministic order on every replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpTimestamp {
    /// Lamport counter. Larger values happened later.
    pub lamport: u64,
    /// Actor that produced the operation. Breaks ties between equal counters.
    pub actor: ActorId,
}

/// Lamport logical clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LamportClock {
    counter: u64,
}

impl LamportClock {
    /// Creates a clock at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stamps a local operation. The stamp is greater than every earlier one.
    pub fn tick(&mut self, actor: ActorId) -> OpTimestamp {
        self.counter = self.counter.saturating_add(1);
        OpTimestamp {
            lamport: self.counter,
            actor,
        }
    }

    /// Merges a remote stamp, so later local stamps sort after it.
    pub fn observe(&mut self, remote: OpTimestamp) {
        self.counter = self.counter.max(remote.lamport);
    }
}

/// Returns `true` if `a` then `b` and `b` then `a` both succeed from `base`
/// and give equal states. A pair where either order fails is not commutative.
pub fn check_commutative<S, O>(base: &S, a: &O, b: &O) -> bool
where
    S: State + PartialEq,
    O: Operation<S>,
{
    match (sequence(base, &[a, b]), sequence(base, &[b, a])) {
        (Some(ab), Some(ba)) => ab == ba,
        _ => false,
    }
}

/// Returns `true` if applying `op` twice from `base` succeeds and gives the
/// same state as applying it once.
pub fn check_idempotent<S, O>(base: &S, op: &O) -> bool
where
    S: State + PartialEq,
    O: Operation<S>,
{
    match (sequence(base, &[op]), sequence(base, &[op, op])) {
        (Some(once), Some(twice)) => once == twice,
        _ => false,
    }
}

/// Applies `ops` in order to a copy of `base`. Returns `None` if any step fails.
fn sequence<S, O>(base: &S, ops: &[&O]) -> Option<S>
where
    S: State,
    O: Operation<S>,
{
    let mut state = base.clone();
    for op in ops {
        state.apply(*op).ok()?;
    }
    Some(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::toy::{Counter, CounterOp, Map, MapOp};
    use alloc::string::ToString;

    impl CrdtCompatible<Counter> for CounterOp {
        // Add commutes, but Set does not. The marker must be the conservative intersection.
        const IS_COMMUTATIVE: bool = false;
        const IS_ASSOCIATIVE: bool = true;
        const IS_IDEMPOTENT: bool = false;
    }

    impl CrdtCompatible<Map> for MapOp {
        // Ops on the same key do not commute, and Remove is not idempotent.
        const IS_COMMUTATIVE: bool = false;
        const IS_ASSOCIATIVE: bool = true;
        const IS_IDEMPOTENT: bool = false;
    }

    fn insert(key: &str, value: i64) -> MapOp {
        MapOp::Insert {
            key: key.to_string(),
            value,
        }
    }

    #[test]
    fn add_pairs_commute() {
        let base = Counter { value: 1 };
        assert!(check_commutative(
            &base,
            &CounterOp::Add(2),
            &CounterOp::Add(3)
        ));
    }

    #[test]
    fn set_pairs_do_not_commute() {
        let base = Counter { value: 1 };
        assert!(!check_commutative(
            &base,
            &CounterOp::Set(1),
            &CounterOp::Set(2)
        ));
    }

    #[test]
    fn set_is_idempotent_and_add_is_not() {
        let base = Counter { value: 1 };
        assert!(check_idempotent(&base, &CounterOp::Set(5)));
        assert!(!check_idempotent(&base, &CounterOp::Add(5)));
    }

    #[test]
    fn map_inserts_on_distinct_keys_commute() {
        let base = Map::default();
        assert!(check_commutative(&base, &insert("a", 1), &insert("b", 2)));
    }

    #[test]
    fn map_inserts_on_the_same_key_do_not_commute() {
        let base = Map::default();
        assert!(!check_commutative(&base, &insert("a", 1), &insert("a", 2)));
    }

    #[test]
    fn failing_pair_is_not_commutative() {
        let base = Map::default();
        let remove = MapOp::Remove {
            key: "missing".to_string(),
        };
        assert!(!check_commutative(&base, &remove, &insert("a", 1)));
    }

    #[test]
    fn declared_markers_hold_on_samples() {
        let base = Counter { value: 0 };
        let samples = [
            CounterOp::Add(1),
            CounterOp::Add(-4),
            CounterOp::Set(3),
            CounterOp::Set(-2),
        ];
        if <CounterOp as CrdtCompatible<Counter>>::IS_COMMUTATIVE {
            for a in &samples {
                for b in &samples {
                    assert!(check_commutative(&base, a, b));
                }
            }
        }
        if <CounterOp as CrdtCompatible<Counter>>::IS_IDEMPOTENT {
            for a in &samples {
                assert!(check_idempotent(&base, a));
            }
        }
    }

    #[test]
    fn timestamps_order_by_counter_then_actor() {
        let early = OpTimestamp {
            lamport: 1,
            actor: ActorId(9),
        };
        let late = OpTimestamp {
            lamport: 2,
            actor: ActorId(1),
        };
        assert!(early < late);

        let tie_a = OpTimestamp {
            lamport: 2,
            actor: ActorId(1),
        };
        let tie_b = OpTimestamp {
            lamport: 2,
            actor: ActorId(2),
        };
        assert!(tie_a < tie_b);
    }

    #[test]
    fn lamport_clock_is_monotonic_and_observes_remote_stamps() {
        let mut clock = LamportClock::new();
        let first = clock.tick(ActorId(1));
        let second = clock.tick(ActorId(1));
        assert!(second > first);

        let remote = OpTimestamp {
            lamport: 100,
            actor: ActorId(2),
        };
        clock.observe(remote);
        let after = clock.tick(ActorId(1));
        assert!(after > remote);
        assert_eq!(after.lamport, 101);
    }
}
