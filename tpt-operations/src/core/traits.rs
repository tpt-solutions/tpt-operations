use super::{ActorId, OpError};

/// Data that operations mutate.
///
/// A `State` is a plain data container. It has no merge method. Merging
/// belongs to `tpt-replicate`, which uses the CRDT markers.
pub trait State: Clone + Sized {
    /// Applies `op` to this state.
    ///
    /// Implementations should validate before they mutate. If `apply` returns
    /// an error, the state may be partly changed. All-or-nothing behaviour
    /// comes from `OperationBatch` (Phase 2).
    fn apply<O: Operation<Self>>(&mut self, op: &O) -> Result<(), OpError> {
        op.apply(self)
    }
}

/// A reversible mutation of a state of type `S`.
///
/// # Inverse Drift rule
///
/// [`Operation::inverse`] is called with the **pre-application** state, and it
/// returns an operation that restores exactly that state. It must capture the
/// old values from the snapshot. It must not compute them back from the
/// operation's inputs.
///
/// Example: the inverse of `Add(n)` is `Set(old_value)`, where `old_value` is
/// read from the state before `Add` runs. It is not `Add(-n)`. Integer and
/// float arithmetic is not always exact, so `Add(-n)` can leave a different
/// value behind.
///
/// The sequence for undo is: `let inv = op.inverse(&state)?; state.apply(&op)?;`.
/// Undo is `state.apply(&inv)`.
pub trait Operation<S: State> {
    /// The operation that undoes this one. Usually `Self`.
    type Inverse: Operation<S>;

    /// Applies this mutation to `state`.
    fn apply(&self, state: &mut S) -> Result<(), OpError>;

    /// Builds the operation that restores `state` to its current contents.
    /// Call this **before** [`Operation::apply`], with the pre-application state.
    fn inverse(&self, state: &S) -> Result<Self::Inverse, OpError>;

    /// Checks that `actor` may perform this mutation on `state`, without changing
    /// anything.
    fn validate(&self, state: &S, actor: &ActorId) -> Result<(), OpError>;
}
