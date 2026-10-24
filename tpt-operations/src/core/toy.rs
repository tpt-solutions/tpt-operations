//! Toy states and operations used to test the core traits.

use super::{ActorId, OpError, Operation, State};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
};

// ---- Counter -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counter {
    pub value: i64,
}

impl State for Counter {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterOp {
    Add(i64),
    Set(i64),
}

impl Operation<Counter> for CounterOp {
    type Inverse = CounterOp;

    fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
        match self {
            // Compute first, then assign, so a failed add leaves the state untouched.
            CounterOp::Add(n) => {
                state.value = state
                    .value
                    .checked_add(*n)
                    .ok_or_else(|| OpError::Bounds("counter overflow".to_string()))?;
            }
            CounterOp::Set(v) => state.value = *v,
        }
        Ok(())
    }

    fn inverse(&self, state: &Counter) -> Result<CounterOp, OpError> {
        // Snapshot rule: restore the exact pre-state value. Never `Add(-n)`.
        Ok(CounterOp::Set(state.value))
    }

    fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

// ---- Map -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Map {
    pub entries: BTreeMap<String, i64>,
}

impl State for Map {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapOp {
    Insert { key: String, value: i64 },
    Remove { key: String },
}

impl MapOp {
    fn key(&self) -> &str {
        match self {
            MapOp::Insert { key, .. } | MapOp::Remove { key } => key,
        }
    }
}

impl Operation<Map> for MapOp {
    type Inverse = MapOp;

    fn apply(&self, state: &mut Map) -> Result<(), OpError> {
        self.validate(state, &ActorId(0))?;
        match self {
            MapOp::Insert { key, value } => {
                state.entries.insert(key.clone(), *value);
            }
            MapOp::Remove { key } => {
                state.entries.remove(key);
            }
        }
        Ok(())
    }

    fn inverse(&self, state: &Map) -> Result<MapOp, OpError> {
        let key = self.key();
        Ok(match state.entries.get(key) {
            // The key existed: put back the old value.
            Some(&value) => MapOp::Insert {
                key: key.to_string(),
                value,
            },
            // The key was absent: removing it restores that.
            None => MapOp::Remove {
                key: key.to_string(),
            },
        })
    }

    fn validate(&self, state: &Map, _actor: &ActorId) -> Result<(), OpError> {
        match self {
            MapOp::Remove { key } if !state.entries.contains_key(key) => {
                Err(OpError::NotFound(key.clone()))
            }
            _ => Ok(()),
        }
    }
}

// ---- Tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec, vec::Vec};

    /// Computes the inverse from the pre-state, applies `op`, then applies the
    /// inverse. Asserts the state matches the original.
    fn assert_reversible<S, O>(mut state: S, op: O)
    where
        S: State + PartialEq + ::core::fmt::Debug,
        O: Operation<S>,
    {
        let before = state.clone();
        let inverse = op.inverse(&state).unwrap();
        state.apply(&op).unwrap();
        state.apply(&inverse).unwrap();
        assert_eq!(state, before);
    }

    fn map_with(entries: &[(&str, i64)]) -> Map {
        Map {
            entries: entries.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn counter_add_is_reversible() {
        assert_reversible(Counter { value: 7 }, CounterOp::Add(5));
        assert_reversible(Counter { value: 7 }, CounterOp::Add(-100));
    }

    #[test]
    fn counter_set_is_reversible() {
        assert_reversible(Counter { value: 7 }, CounterOp::Set(-3));
    }

    #[test]
    fn counter_inverse_is_a_snapshot_not_arithmetic() {
        let state = Counter { value: 7 };
        // Add(5) would invert to Add(-5). The snapshot inverse is Set(7).
        assert_eq!(
            CounterOp::Add(5).inverse(&state).unwrap(),
            CounterOp::Set(7)
        );
    }

    #[test]
    fn counter_overflow_leaves_state_untouched() {
        let mut state = Counter { value: i64::MAX };
        let err = state.apply(&CounterOp::Add(1)).unwrap_err();
        assert!(matches!(err, OpError::Bounds(_)));
        assert_eq!(state.value, i64::MAX);
    }

    #[test]
    fn map_insert_new_key_is_reversible() {
        assert_reversible(
            map_with(&[("a", 1)]),
            MapOp::Insert {
                key: "b".into(),
                value: 2,
            },
        );
    }

    #[test]
    fn map_insert_existing_key_restores_old_value() {
        let state = map_with(&[("a", 1)]);
        let op = MapOp::Insert {
            key: "a".into(),
            value: 99,
        };
        assert_eq!(
            op.inverse(&state).unwrap(),
            MapOp::Insert {
                key: "a".into(),
                value: 1
            }
        );
        assert_reversible(state, op);
    }

    #[test]
    fn map_remove_is_reversible() {
        assert_reversible(
            map_with(&[("a", 1), ("b", 2)]),
            MapOp::Remove { key: "a".into() },
        );
    }

    #[test]
    fn map_remove_missing_key_is_not_found() {
        let mut state = map_with(&[("a", 1)]);
        let op = MapOp::Remove { key: "z".into() };
        assert_eq!(
            op.validate(&state, &ActorId(1)),
            Err(OpError::NotFound("z".into()))
        );
        assert_eq!(state.apply(&op), Err(OpError::NotFound("z".into())));
        assert_eq!(state, map_with(&[("a", 1)]));
    }

    #[test]
    fn reversing_a_sequence_in_order_restores_the_start() {
        let start = Counter { value: 0 };
        let ops = vec![CounterOp::Add(5), CounterOp::Set(10), CounterOp::Add(-3)];

        let mut state = start.clone();
        let mut inverses = Vec::new();
        for op in &ops {
            inverses.push(op.inverse(&state).unwrap());
            state.apply(op).unwrap();
        }
        assert_eq!(state.value, 7);

        for inv in inverses.iter().rev() {
            state.apply(inv).unwrap();
        }
        assert_eq!(state, start);
    }

    #[test]
    fn error_display_includes_kind_and_message() {
        let err = OpError::Conflict("row locked".into());
        assert_eq!(format!("{err}"), "conflict error: row locked");
    }
}
