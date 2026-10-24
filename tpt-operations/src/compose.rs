//! Batching: groups of operations that are applied all-or-nothing.

use crate::core::{ActorId, OpError, Operation, State};
use alloc::vec::Vec;

/// An ordered group of operations that applies atomically.
///
/// If any operation fails validation or application, the error is returned and
/// the state is exactly as it was before the batch started, even when the
/// failing operation had already changed part of a state.
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

    /// The operations, in application order.
    pub fn ops(&self) -> &[O] {
        &self.ops
    }

    /// Consumes the batch and returns its operations, in application order.
    pub fn into_ops(self) -> Vec<O> {
        self.ops
    }

    /// Applies every operation in order, atomically.
    ///
    /// On success, returns the batch that undoes this one. Its operations are
    /// the inverses in reverse order, so applying it restores the pre-batch
    /// state. On failure, `state` is untouched and the error is returned.
    ///
    /// The operations run on a copy of `state`, which replaces `state` only if
    /// every operation succeeds. [`Operation::apply`] may leave a state partly
    /// changed on error, and the copy keeps that from reaching the caller. It
    /// also means no rollback is needed, so there is no rollback to fail. The
    /// cost is one clone of the state per batch.
    pub fn apply<S>(
        &self,
        state: &mut S,
        actor: &ActorId,
    ) -> Result<OperationBatch<O::Inverse>, OpError>
    where
        S: State,
        O: Operation<S>,
    {
        let mut next = state.clone();
        let mut inverses = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            inverses.push(apply_step(op, &mut next, actor)?);
        }
        *state = next;
        inverses.reverse();
        Ok(OperationBatch::from_ops(inverses))
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

impl<O> From<Vec<O>> for OperationBatch<O> {
    fn from(ops: Vec<O>) -> Self {
        Self { ops }
    }
}

impl<O> Extend<O> for OperationBatch<O> {
    fn extend<I: IntoIterator<Item = O>>(&mut self, iter: I) {
        self.ops.extend(iter);
    }
}

impl<O> IntoIterator for OperationBatch<O> {
    type Item = O;
    type IntoIter = ::alloc::vec::IntoIter<O>;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.into_iter()
    }
}

impl<'a, O> IntoIterator for &'a OperationBatch<O> {
    type Item = &'a O;
    type IntoIter = ::core::slice::Iter<'a, O>;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.iter()
    }
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
/// ```
/// use tpt_operations::{compose::OperationBatch, core::{ActorId, OpError, Operation, State}, tpt_batch};
///
/// #[derive(Clone, Default)]
/// struct Counter(i64);
/// impl State for Counter {}
///
/// #[derive(Clone)]
/// struct Set(i64);
/// impl Operation<Counter> for Set {
///     type Inverse = Set;
///     fn apply(&self, s: &mut Counter) -> Result<(), OpError> { s.0 = self.0; Ok(()) }
///     fn inverse(&self, s: &Counter) -> Result<Set, OpError> { Ok(Set(s.0)) }
///     fn validate(&self, _s: &Counter, _a: &ActorId) -> Result<(), OpError> { Ok(()) }
/// }
///
/// let batch: OperationBatch<Set> = tpt_batch![Set(1), Set(2)];
/// let mut state = Counter::default();
/// batch.apply(&mut state, &ActorId(1)).unwrap();
/// assert_eq!(state.0, 2);
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

    /// An op that changes the state and then fails, which `State::apply` permits.
    #[derive(Debug, Clone)]
    struct HalfApply;

    impl Operation<Counter> for HalfApply {
        type Inverse = CounterOp;

        fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
            state.value += 100;
            Err(OpError::Conflict("failed after mutating".to_string()))
        }

        fn inverse(&self, state: &Counter) -> Result<CounterOp, OpError> {
            Ok(CounterOp::Set(state.value))
        }

        fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
            Ok(())
        }
    }

    #[test]
    fn partial_mutation_by_a_failing_op_is_not_left_in_the_state() {
        let start = Counter { value: 4 };
        let mut state = start.clone();
        let batch = OperationBatch::from_ops(vec![HalfApply]);

        let err = batch.apply(&mut state, &actor()).unwrap_err();
        assert!(matches!(err, OpError::Conflict(_)));
        assert_eq!(state, start);
    }

    #[test]
    fn partial_mutation_after_earlier_successes_leaves_the_start_state() {
        // The first op succeeds and the second half-applies, so neither may survive.
        let start = Counter { value: 4 };
        let mut state = start.clone();
        let batch: OperationBatch<CounterOpOrHalf> = OperationBatch::from_ops(vec![
            CounterOpOrHalf::Plain(CounterOp::Add(1)),
            CounterOpOrHalf::Half(HalfApply),
        ]);

        batch.apply(&mut state, &actor()).unwrap_err();
        assert_eq!(state, start);
    }

    #[derive(Debug, Clone)]
    enum CounterOpOrHalf {
        Plain(CounterOp),
        Half(HalfApply),
    }

    impl Operation<Counter> for CounterOpOrHalf {
        type Inverse = CounterOp;

        fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
            match self {
                Self::Plain(op) => op.apply(state),
                Self::Half(op) => op.apply(state),
            }
        }

        fn inverse(&self, state: &Counter) -> Result<CounterOp, OpError> {
            Ok(CounterOp::Set(state.value))
        }

        fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
            Ok(())
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
    fn batch_converts_extends_and_iterates() {
        let mut batch = OperationBatch::from(vec![CounterOp::Add(1)]);
        batch.extend([CounterOp::Add(2), CounterOp::Set(3)]);
        assert_eq!(batch.ops().len(), 3);

        let by_ref: Vec<&CounterOp> = (&batch).into_iter().collect();
        assert_eq!(by_ref.len(), 3);

        let owned: Vec<CounterOp> = batch.clone().into_iter().collect();
        assert_eq!(owned, batch.into_ops());
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
