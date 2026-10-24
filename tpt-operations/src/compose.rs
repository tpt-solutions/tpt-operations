//! Batching: groups of operations that are applied all-or-nothing.

use crate::core::{ActorId, OpError, Operation, State};
use alloc::{format, vec::Vec};

/// An ordered group of operations that applies atomically.
///
/// If any operation fails validation or application, the operations already
/// applied are undone in reverse order, and the error is returned. The state
/// is then exactly as it was before the batch started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationBatch<O> {
    ops: Vec<O>,
}

impl<O> Default for OperationBatch<O> {
    fn default() -> Self {
        Self { ops: Vec::new() }
    }
}

impl<O> OperationBatch<O> {
    /// Creates an empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a batch from a list of operations, in application order.
    pub fn from_ops(ops: Vec<O>) -> Self {
        Self { ops }
    }

    /// Appends an operation to the end of the batch.
    pub fn push(&mut self, op: O) {
        self.ops.push(op);
    }

    /// Number of operations in the batch.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns `true` if the batch has no operations.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Iterates over the operations in application order.
    pub fn iter(&self) -> ::core::slice::Iter<'_, O> {
        self.ops.iter()
    }

    /// Applies every operation in order, atomically.
    ///
    /// On success, returns the batch that undoes this one. Its operations are
    /// the inverses in reverse order, so applying it restores the pre-batch
    /// state. On failure, the state is rolled back and the error is returned.
    /// If the rollback itself fails, the error is `OpError::Conflict` and the
    /// state is unknown. Snapshot-based inverses should make that impossible.
    pub fn apply<S>(
        &self,
        state: &mut S,
        actor: &ActorId,
    ) -> Result<OperationBatch<O::Inverse>, OpError>
    where
        S: State,
        O: Operation<S>,
    {
        let mut applied = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            match apply_step(op, state, actor) {
                Ok(inverse) => applied.push(inverse),
                Err(err) => return Err(rollback(state, applied, err)),
            }
        }
        applied.reverse();
        Ok(OperationBatch::from_ops(applied))
    }
}

/// Validates, snapshots the inverse, and applies one operation.
fn apply_step<S, O>(op: &O, state: &mut S, actor: &ActorId) -> Result<O::Inverse, OpError>
where
    S: State,
    O: Operation<S>,
{
    op.validate(state, actor)?;
    let inverse = op.inverse(state)?;
    op.apply(state)?;
    Ok(inverse)
}

/// Undoes the operations applied so far, newest first, and returns `err`.
fn rollback<S, I>(state: &mut S, applied: Vec<I>, err: OpError) -> OpError
where
    S: State,
    I: Operation<S>,
{
    for inverse in applied.iter().rev() {
        if let Err(undo_err) = inverse.apply(state) {
            return OpError::Conflict(format!("rollback failed ({undo_err}) after: {err}"));
        }
    }
    err
}

impl<O> FromIterator<O> for OperationBatch<O> {
    fn from_iter<I: IntoIterator<Item = O>>(iter: I) -> Self {
        Self {
            ops: iter.into_iter().collect(),
        }
    }
}

/// Builds an [`OperationBatch`] from a comma-separated list of operations.
///
/// ```ignore
/// let batch = tpt_batch![MapOp::Insert { key: "a".into(), value: 1 }, MapOp::Remove { key: "b".into() }];
/// ```
#[macro_export]
macro_rules! tpt_batch {
    ($($op:expr),* $(,)?) => {
        [$($op),*].into_iter().collect::<$crate::compose::OperationBatch<_>>()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::toy::{Counter, CounterOp, Map, MapOp};
    use alloc::{string::ToString, vec};

    fn actor() -> ActorId {
        ActorId(1)
    }

    fn valid_map_prefix() -> Vec<MapOp> {
        vec![
            MapOp::Insert {
                key: "b".to_string(),
                value: 2,
            },
            MapOp::Insert {
                key: "c".to_string(),
                value: 3,
            },
            MapOp::Remove {
                key: "a".to_string(),
            },
        ]
    }

    #[test]
    fn successful_batch_applies_every_op_and_returns_undo_batch() {
        let mut state = Map::default();
        state.entries.insert("a".to_string(), 1);
        let start = state.clone();

        let batch: OperationBatch<MapOp> = valid_map_prefix().into_iter().collect();
        let undo = batch.apply(&mut state, &actor()).unwrap();

        assert_eq!(state.entries.get("b"), Some(&2));
        assert_eq!(state.entries.get("c"), Some(&3));
        assert!(!state.entries.contains_key("a"));

        // The undo batch is the reversed list of inverses, so it restores the start.
        assert_eq!(undo.len(), batch.len());
        undo.apply(&mut state, &actor()).unwrap();
        assert_eq!(state, start);
    }

    #[test]
    fn validation_failure_at_every_position_rolls_back() {
        let start = {
            let mut s = Map::default();
            s.entries.insert("a".to_string(), 1);
            s
        };
        let valid = valid_map_prefix();

        for position in 0..=valid.len() {
            let mut ops = valid.clone();
            // Inserting a remove of a missing key fails validation at `position`.
            ops.insert(
                position,
                MapOp::Remove {
                    key: "missing".to_string(),
                },
            );
            let batch = OperationBatch::from_ops(ops);

            let mut state = start.clone();
            let err = batch.apply(&mut state, &actor()).unwrap_err();
            assert_eq!(
                err,
                OpError::NotFound("missing".to_string()),
                "position {position}"
            );
            assert_eq!(state, start, "state changed at position {position}");
        }
    }

    #[test]
    fn apply_failure_at_every_position_rolls_back() {
        // Every prefix is a valid add, then Add(20) overflows, since the start is
        // near i64::MAX. The earlier successful adds must all be undone.
        let start = Counter {
            value: i64::MAX - 10,
        };

        for position in 0..=5 {
            let mut ops = vec![CounterOp::Add(1); 5];
            ops.insert(position, CounterOp::Add(20));
            let batch = OperationBatch::from_ops(ops);

            let mut state = start.clone();
            let err = batch.apply(&mut state, &actor()).unwrap_err();
            assert!(matches!(err, OpError::Bounds(_)), "position {position}");
            assert_eq!(state, start, "state changed at position {position}");
        }
    }

    #[test]
    fn batch_inverse_undoes_mixed_counter_ops() {
        let mut state = Counter { value: 4 };
        let start = state.clone();
        let batch = crate::tpt_batch![CounterOp::Add(3), CounterOp::Set(-8), CounterOp::Add(1)];

        let undo = batch.apply(&mut state, &actor()).unwrap();
        assert_eq!(state.value, -7);
        undo.apply(&mut state, &actor()).unwrap();
        assert_eq!(state, start);
    }

    #[test]
    fn macro_and_from_iterator_build_the_same_batch() {
        let via_macro = crate::tpt_batch![CounterOp::Add(1), CounterOp::Set(2)];
        let via_iter: OperationBatch<CounterOp> =
            [CounterOp::Add(1), CounterOp::Set(2)].into_iter().collect();
        assert_eq!(via_macro, via_iter);
        assert_eq!(via_macro.iter().count(), 2);
    }
}
