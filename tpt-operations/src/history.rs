//! Undo/redo history with count and memory caps.
//!
//! Each step is one [`OperationBatch`], so a batch is a single undo step.
//! Undo applies the stored undo batch and keeps the batch that reverses it,
//! for redo. Applying a new batch clears the redo stack.

use crate::{
    compose::OperationBatch,
    core::{ActorId, OpError, Operation, State},
};
use alloc::{collections::VecDeque, vec::Vec};

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub mod replay;
#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub use replay::{ReplayEngine, ReplayError, ReplayPolicy, ReplayTrace};

/// Default cap on the number of undo steps.
pub const DEFAULT_MAX_OPS: usize = 10_000;

/// Default cap on the estimated memory held by undo steps (50 MB).
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024 * 1024;

/// Estimates how much memory an operation holds, for the byte cap.
pub trait SizeEstimate {
    /// Approximate heap plus inline size of this operation, in bytes.
    fn estimated_size(&self) -> usize;
}

#[derive(Debug, Clone)]
struct Step<O> {
    batch: OperationBatch<O>,
    bytes: usize,
}

/// Bounded undo/redo history for operations of type `O`.
///
/// ```
/// use tpt_operations::prelude::*;
///
/// # #[derive(Clone, Default)] struct Counter(i64);
/// # impl State for Counter {}
/// # #[derive(Clone)] struct Set(i64);
/// # impl Operation<Counter> for Set {
/// #     type Inverse = Set;
/// #     fn apply(&self, s: &mut Counter) -> Result<(), OpError> { s.0 = self.0; Ok(()) }
/// #     fn inverse(&self, s: &Counter) -> Result<Set, OpError> { Ok(Set(s.0)) }
/// #     fn validate(&self, _: &Counter, _: &ActorId) -> Result<(), OpError> { Ok(()) }
/// # }
/// # impl SizeEstimate for Set { fn estimated_size(&self) -> usize { 8 } }
/// let me = ActorId(1);
/// let mut state = Counter::default();
/// // Keep at most two steps, and 1 MB of operations.
/// let mut history = HistoryStack::with_limits(2, 1 << 20);
///
/// for n in 1..=3 {
///     history.execute(&mut state, &tpt_batch![Set(n)], &me)?;
/// }
/// // The oldest step was evicted, so only two undos are left.
/// assert_eq!(history.undo_len(), 2);
///
/// assert!(history.next_undo().is_some()); // what undo would apply
/// while history.undo(&mut state, &me)? {}
/// assert_eq!(state.0, 1);
/// # Ok::<(), OpError>(())
/// ```
///
/// Eviction removes the oldest undo steps first. Redo steps are not counted
/// toward the caps. They only come from steps that were undone, so they never
/// outgrow the undo history.
#[derive(Debug, Clone)]
pub struct HistoryStack<O> {
    undo: VecDeque<Step<O>>,
    redo: Vec<Step<O>>,
    undo_bytes: usize,
    max_ops: usize,
    max_bytes: usize,
}

impl<O> HistoryStack<O> {
    /// Creates a history with the default caps (10,000 steps, 50 MB).
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_OPS, DEFAULT_MAX_BYTES)
    }

    /// Creates a history with custom caps.
    pub fn with_limits(max_ops: usize, max_bytes: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
            max_ops,
            max_bytes,
        }
    }

    /// Number of undo steps available.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of redo steps available.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Estimated bytes held by the undo steps.
    pub fn undo_bytes(&self) -> usize {
        self.undo_bytes
    }

    /// The batch that [`HistoryStack::undo`] would apply next, if any.
    ///
    /// It is the inverse of the step being undone, so a UI can show what undo
    /// will do.
    pub fn next_undo(&self) -> Option<&OperationBatch<O>> {
        self.undo.back().map(|step| &step.batch)
    }

    /// The batch that [`HistoryStack::redo`] would apply next, if any.
    pub fn next_redo(&self) -> Option<&OperationBatch<O>> {
        self.redo.last().map(|step| &step.batch)
    }

    /// Forgets all undo and redo steps. The state is not touched.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.undo_bytes = 0;
    }

    /// Returns `true` if [`HistoryStack::undo`] would do something.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns `true` if [`HistoryStack::redo`] would do something.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl<O> Default for HistoryStack<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: SizeEstimate> HistoryStack<O> {
    /// Applies `batch` to `state` and records it as one undo step.
    ///
    /// An empty batch changes nothing and records nothing. If the batch fails,
    /// the state and the history are both left unchanged.
    pub fn execute<S>(
        &mut self,
        state: &mut S,
        batch: &OperationBatch<O>,
        actor: &ActorId,
    ) -> Result<(), OpError>
    where
        S: State,
        O: Operation<S, Inverse = O>,
    {
        let undo_batch = batch.apply(state, actor)?;
        if undo_batch.is_empty() {
            return Ok(());
        }
        self.redo.clear();
        let bytes = batch_bytes(&undo_batch);
        self.undo.push_back(Step {
            batch: undo_batch,
            bytes,
        });
        self.undo_bytes += bytes;
        self.evict();
        Ok(())
    }

    /// Undoes the most recent step. Returns `Ok(false)` if there is nothing to undo.
    pub fn undo<S>(&mut self, state: &mut S, actor: &ActorId) -> Result<bool, OpError>
    where
        S: State,
        O: Operation<S, Inverse = O>,
    {
        let Some(step) = self.undo.pop_back() else {
            return Ok(false);
        };
        match step.batch.apply(state, actor) {
            Ok(redo_batch) => {
                self.undo_bytes -= step.bytes;
                // The reversing batch can be a different size, so measure it.
                let bytes = batch_bytes(&redo_batch);
                self.redo.push(Step {
                    batch: redo_batch,
                    bytes,
                });
                Ok(true)
            }
            Err(err) => {
                self.undo.push_back(step);
                Err(err)
            }
        }
    }

    /// Re-applies the most recently undone step. Returns `Ok(false)` if there is nothing to redo.
    pub fn redo<S>(&mut self, state: &mut S, actor: &ActorId) -> Result<bool, OpError>
    where
        S: State,
        O: Operation<S, Inverse = O>,
    {
        let Some(step) = self.redo.pop() else {
            return Ok(false);
        };
        match step.batch.apply(state, actor) {
            Ok(undo_batch) => {
                let bytes = batch_bytes(&undo_batch);
                self.undo_bytes += bytes;
                self.undo.push_back(Step {
                    batch: undo_batch,
                    bytes,
                });
                self.evict();
                Ok(true)
            }
            Err(err) => {
                self.redo.push(step);
                Err(err)
            }
        }
    }

    /// Drops the oldest undo steps until both caps hold.
    ///
    /// A single step larger than `max_bytes` is dropped too. Its change stays
    /// applied to the state, but it can no longer be undone.
    fn evict(&mut self) {
        while self.undo.len() > self.max_ops || self.undo_bytes > self.max_bytes {
            match self.undo.pop_front() {
                Some(step) => self.undo_bytes -= step.bytes,
                None => break,
            }
        }
    }
}

fn batch_bytes<O: SizeEstimate>(batch: &OperationBatch<O>) -> usize {
    batch.iter().map(SizeEstimate::estimated_size).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::toy::{Counter, CounterOp, Map, MapOp};
    use alloc::string::ToString;

    impl SizeEstimate for CounterOp {
        fn estimated_size(&self) -> usize {
            ::core::mem::size_of::<Self>()
        }
    }

    impl SizeEstimate for MapOp {
        fn estimated_size(&self) -> usize {
            let key = match self {
                MapOp::Insert { key, .. } | MapOp::Remove { key } => key,
            };
            ::core::mem::size_of::<Self>() + key.len()
        }
    }

    fn actor() -> ActorId {
        ActorId(1)
    }

    fn add(n: i64) -> OperationBatch<CounterOp> {
        OperationBatch::from_ops(alloc::vec![CounterOp::Add(n)])
    }

    #[test]
    fn execute_then_undo_then_redo_round_trips() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };

        history.execute(&mut state, &add(5), &actor()).unwrap();
        assert_eq!(state.value, 5);

        assert!(history.undo(&mut state, &actor()).unwrap());
        assert_eq!(state.value, 0);

        assert!(history.redo(&mut state, &actor()).unwrap());
        assert_eq!(state.value, 5);
    }

    #[test]
    fn next_undo_and_next_redo_show_the_pending_batches_without_applying_them() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };
        assert!(history.next_undo().is_none() && history.next_redo().is_none());

        history.execute(&mut state, &add(5), &actor()).unwrap();
        // The pending undo is the inverse: a snapshot of the old value.
        let pending = history.next_undo().unwrap();
        assert_eq!(pending.ops(), [CounterOp::Set(0)]);
        assert_eq!(state.value, 5);

        history.undo(&mut state, &actor()).unwrap();
        assert!(history.next_undo().is_none());
        assert_eq!(history.next_redo().unwrap().ops(), [CounterOp::Set(5)]);
    }

    #[test]
    fn clear_forgets_both_stacks_and_keeps_the_state() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };
        history.execute(&mut state, &add(1), &actor()).unwrap();
        history.execute(&mut state, &add(2), &actor()).unwrap();
        history.undo(&mut state, &actor()).unwrap();

        history.clear();
        assert!(!history.can_undo() && !history.can_redo());
        assert_eq!(history.undo_bytes(), 0);
        assert_eq!(state.value, 1);
    }

    #[test]
    fn undo_and_redo_on_empty_history_report_false() {
        let mut history: HistoryStack<CounterOp> = HistoryStack::new();
        let mut state = Counter { value: 0 };
        assert!(!history.undo(&mut state, &actor()).unwrap());
        assert!(!history.redo(&mut state, &actor()).unwrap());
    }

    #[test]
    fn new_operation_clears_redo() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };
        history.execute(&mut state, &add(1), &actor()).unwrap();
        history.undo(&mut state, &actor()).unwrap();
        assert!(history.can_redo());

        history.execute(&mut state, &add(2), &actor()).unwrap();
        assert!(!history.can_redo());
        assert_eq!(state.value, 2);
    }

    #[test]
    fn batch_is_a_single_undo_step() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };
        let batch = crate::tpt_batch![CounterOp::Add(1), CounterOp::Set(10), CounterOp::Add(4)];

        history.execute(&mut state, &batch, &actor()).unwrap();
        assert_eq!(state.value, 14);
        assert_eq!(history.undo_len(), 1);

        history.undo(&mut state, &actor()).unwrap();
        assert_eq!(state.value, 0);
        assert!(!history.can_undo());
    }

    #[test]
    fn failed_batch_leaves_state_and_history_alone() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: i64::MAX };
        // Set(5) succeeds, then Add(i64::MAX) overflows, so the Set must be rolled back.
        let batch = crate::tpt_batch![CounterOp::Set(5), CounterOp::Add(i64::MAX)];

        history.execute(&mut state, &batch, &actor()).unwrap_err();
        assert_eq!(state.value, i64::MAX);
        assert_eq!(history.undo_len(), 0);
    }

    #[test]
    fn empty_batch_records_nothing() {
        let mut history: HistoryStack<CounterOp> = HistoryStack::new();
        let mut state = Counter { value: 3 };
        history
            .execute(&mut state, &OperationBatch::new(), &actor())
            .unwrap();
        assert_eq!(history.undo_len(), 0);
        assert_eq!(state.value, 3);
    }

    #[test]
    fn count_cap_evicts_oldest_first() {
        let mut history = HistoryStack::with_limits(3, usize::MAX);
        let mut state = Counter { value: 0 };
        for _ in 0..5 {
            history.execute(&mut state, &add(1), &actor()).unwrap();
        }
        assert_eq!(history.undo_len(), 3);
        assert_eq!(state.value, 5);

        // Only the three newest steps can be undone. The state ends at 2.
        while history.undo(&mut state, &actor()).unwrap() {}
        assert_eq!(state.value, 2);
    }

    #[test]
    fn count_cap_boundary_keeps_exactly_max_steps() {
        let mut history = HistoryStack::with_limits(3, usize::MAX);
        let mut state = Counter { value: 0 };
        for _ in 0..3 {
            history.execute(&mut state, &add(1), &actor()).unwrap();
        }
        assert_eq!(history.undo_len(), 3);
    }

    #[test]
    fn byte_cap_evicts_by_estimated_size() {
        let per_op = CounterOp::Add(1).estimated_size();
        let mut history = HistoryStack::with_limits(usize::MAX, per_op * 2);
        let mut state = Counter { value: 0 };
        for _ in 0..4 {
            history.execute(&mut state, &add(1), &actor()).unwrap();
        }
        assert_eq!(history.undo_len(), 2);
        assert_eq!(history.undo_bytes(), per_op * 2);
    }

    #[test]
    fn step_larger_than_byte_cap_is_applied_but_not_undoable() {
        let mut history = HistoryStack::with_limits(usize::MAX, 1);
        let mut state = Counter { value: 0 };
        history.execute(&mut state, &add(7), &actor()).unwrap();
        assert_eq!(state.value, 7);
        assert!(!history.can_undo());
    }

    #[test]
    fn undo_bytes_follow_the_size_of_each_stored_batch() {
        // Removing a long key stores a small inverse, and inserting it back stores a big one.
        let mut history = HistoryStack::new();
        let mut state = Map::default();
        let key = "k".repeat(100);
        state.entries.insert(key.clone(), 1);
        let remove = OperationBatch::from_ops(alloc::vec![MapOp::Remove { key: key.clone() }]);

        history.execute(&mut state, &remove, &actor()).unwrap();
        let after_execute = history.undo_bytes();
        history.undo(&mut state, &actor()).unwrap();
        assert_eq!(history.undo_bytes(), 0);
        history.redo(&mut state, &actor()).unwrap();
        // The step is the same size as when it was first recorded.
        assert_eq!(history.undo_bytes(), after_execute);
        history.undo(&mut state, &actor()).unwrap();
        history.redo(&mut state, &actor()).unwrap();
        assert_eq!(history.undo_bytes(), after_execute);
    }

    #[test]
    fn eviction_while_redo_is_pending_keeps_redo_and_the_caps() {
        let mut history = HistoryStack::with_limits(2, usize::MAX);
        let mut state = Counter { value: 0 };
        for _ in 0..2 {
            history.execute(&mut state, &add(1), &actor()).unwrap();
        }
        history.undo(&mut state, &actor()).unwrap();
        assert_eq!((history.undo_len(), history.redo_len()), (1, 1));

        // Redo pushes onto a full-minus-one stack, which stays within the cap.
        history.redo(&mut state, &actor()).unwrap();
        assert_eq!((history.undo_len(), history.redo_len()), (2, 0));

        // Undo twice, redo twice: the cap of two is never exceeded.
        history.undo(&mut state, &actor()).unwrap();
        history.undo(&mut state, &actor()).unwrap();
        history.redo(&mut state, &actor()).unwrap();
        history.redo(&mut state, &actor()).unwrap();
        assert!(history.undo_len() <= 2);
        assert_eq!(state.value, 2);
    }

    #[test]
    fn zero_limits_keep_no_history_but_still_apply() {
        let mut state = Counter { value: 0 };
        let mut by_count = HistoryStack::with_limits(0, usize::MAX);
        by_count.execute(&mut state, &add(2), &actor()).unwrap();
        assert!(!by_count.can_undo());

        let mut by_bytes = HistoryStack::with_limits(usize::MAX, 0);
        by_bytes.execute(&mut state, &add(3), &actor()).unwrap();
        assert!(!by_bytes.can_undo());
        assert_eq!(state.value, 5);
    }

    #[test]
    fn failed_execute_keeps_the_redo_stack() {
        let mut history = HistoryStack::new();
        let mut state = Counter { value: 0 };
        history.execute(&mut state, &add(1), &actor()).unwrap();
        history.undo(&mut state, &actor()).unwrap();
        assert_eq!(history.redo_len(), 1);

        state.value = i64::MAX;
        history.execute(&mut state, &add(1), &actor()).unwrap_err();
        assert_eq!(history.redo_len(), 1);
        assert_eq!(state.value, i64::MAX);
    }

    #[test]
    fn failed_undo_keeps_the_step_and_the_state() {
        // Snapshot inverses cannot fail on their own state, so plant an
        // overflowing step to reach the failure path.
        let mut failing = HistoryStack::new();
        failing.undo.push_back(Step {
            batch: crate::tpt_batch![CounterOp::Add(i64::MAX)],
            bytes: 1,
        });
        failing.undo_bytes = 1;
        let mut near_max = Counter { value: 5 };
        failing.undo(&mut near_max, &actor()).unwrap_err();
        assert_eq!(near_max.value, 5);
        assert_eq!(failing.undo_len(), 1);
        assert_eq!(failing.undo_bytes(), 1);
    }

    #[test]
    fn failed_redo_keeps_the_step_and_the_state() {
        let mut history: HistoryStack<CounterOp> = HistoryStack::new();
        history.redo.push(Step {
            batch: crate::tpt_batch![CounterOp::Add(i64::MAX)],
            bytes: 1,
        });
        let mut state = Counter { value: 5 };
        history.redo(&mut state, &actor()).unwrap_err();
        assert_eq!(state.value, 5);
        assert_eq!(history.redo_len(), 1);
    }

    #[test]
    fn map_history_undoes_inserts_and_removes() {
        let mut history = HistoryStack::new();
        let mut state = Map::default();
        let insert = OperationBatch::from_ops(alloc::vec![MapOp::Insert {
            key: "a".to_string(),
            value: 1,
        }]);
        let remove = OperationBatch::from_ops(alloc::vec![MapOp::Remove {
            key: "a".to_string(),
        }]);

        history.execute(&mut state, &insert, &actor()).unwrap();
        history.execute(&mut state, &remove, &actor()).unwrap();
        assert!(state.entries.is_empty());

        history.undo(&mut state, &actor()).unwrap();
        assert_eq!(state.entries.get("a"), Some(&1));
        history.undo(&mut state, &actor()).unwrap();
        assert!(state.entries.is_empty());
    }
}
