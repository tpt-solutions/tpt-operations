//! Randomized properties over the toy states, run with a fixed seed.
//!
//! These cover the spec's Phase 3 properties: reversibility, commutativity
//! under shuffled application, and idempotence.

use crate::{
    core::{
        toy::{Counter, CounterOp, Map, MapOp},
        ActorId, Operation, State,
    },
    testkit::{Rng, CASES},
};
use alloc::{format, string::String, vec::Vec};

const ACTOR: ActorId = ActorId(1);

/// Property: `state == apply(inverse(state), apply(op, state))`.
fn assert_reversible<S, O>(state: &S, op: &O)
where
    S: State + PartialEq + ::core::fmt::Debug,
    O: Operation<S>,
{
    let mut working = state.clone();
    let inverse = op.inverse(&working).unwrap();
    working.apply(op).unwrap();
    working.apply(&inverse).unwrap();
    assert_eq!(&working, state);
}

fn random_counter_op(rng: &mut Rng) -> CounterOp {
    if rng.below(2) == 0 {
        CounterOp::Add(rng.range_i64(-500, 500))
    } else {
        CounterOp::Set(rng.range_i64(-1_000, 1_000))
    }
}

fn random_key(rng: &mut Rng) -> String {
    format!("k{}", rng.below(4))
}

fn random_map_op(rng: &mut Rng) -> MapOp {
    if rng.below(2) == 0 {
        MapOp::Insert {
            key: random_key(rng),
            value: rng.range_i64(-100, 100),
        }
    } else {
        MapOp::Remove {
            key: random_key(rng),
        }
    }
}

#[test]
fn counter_ops_are_reversible_over_many_cases() {
    let mut rng = Rng::new(0xC0FF_EE01);
    for _ in 0..CASES {
        let state = Counter {
            value: rng.range_i64(-1_000, 1_000),
        };
        let op = random_counter_op(&mut rng);
        assert_reversible(&state, &op);
    }
}

#[test]
fn map_ops_are_reversible_over_many_cases() {
    let mut rng = Rng::new(0x0BAD_CAFE);
    let mut exercised = 0usize;
    for _ in 0..CASES {
        let mut state = Map::default();
        for i in 0..4 {
            if rng.below(2) == 0 {
                state
                    .entries
                    .insert(format!("k{i}"), rng.range_i64(-100, 100));
            }
        }
        let op = random_map_op(&mut rng);
        // Ops that fail validation have no inverse to test.
        if op.validate(&state, &ACTOR).is_ok() {
            assert_reversible(&state, &op);
            exercised += 1;
        }
    }
    assert!(exercised > CASES / 4, "too few valid cases: {exercised}");
}

#[test]
fn counter_adds_commute_under_shuffled_application() {
    let mut rng = Rng::new(0x5EED_0001);
    for _ in 0..CASES / 10 {
        let start = rng.range_i64(-1_000, 1_000);
        let mut ops: Vec<CounterOp> = (0..8)
            .map(|_| CounterOp::Add(rng.range_i64(-500, 500)))
            .collect();

        let mut ordered = Counter { value: start };
        for op in &ops {
            ordered.apply(op).unwrap();
        }

        rng.shuffle(&mut ops);
        let mut shuffled = Counter { value: start };
        for op in &ops {
            shuffled.apply(op).unwrap();
        }
        assert_eq!(shuffled, ordered);
    }
}

#[test]
fn map_inserts_on_distinct_keys_commute_under_shuffled_application() {
    let mut rng = Rng::new(0x5EED_0002);
    for _ in 0..CASES / 10 {
        let mut ops: Vec<MapOp> = (0..6)
            .map(|i| MapOp::Insert {
                key: format!("k{i}"),
                value: rng.range_i64(-100, 100),
            })
            .collect();

        let mut ordered = Map::default();
        for op in &ops {
            ordered.apply(op).unwrap();
        }

        rng.shuffle(&mut ops);
        let mut shuffled = Map::default();
        for op in &ops {
            shuffled.apply(op).unwrap();
        }
        assert_eq!(shuffled, ordered);
    }
}

#[test]
fn set_and_insert_are_idempotent_over_many_cases() {
    let mut rng = Rng::new(0x1D3_0001);
    for _ in 0..CASES {
        let state = Counter {
            value: rng.range_i64(-1_000, 1_000),
        };
        let op = CounterOp::Set(rng.range_i64(-1_000, 1_000));
        let mut once = state.clone();
        once.apply(&op).unwrap();
        let mut twice = once.clone();
        twice.apply(&op).unwrap();
        assert_eq!(once, twice);

        let mut map = Map::default();
        let insert = MapOp::Insert {
            key: random_key(&mut rng),
            value: rng.range_i64(-100, 100),
        };
        map.apply(&insert).unwrap();
        let after_one = map.clone();
        map.apply(&insert).unwrap();
        assert_eq!(map, after_one);
    }
}
