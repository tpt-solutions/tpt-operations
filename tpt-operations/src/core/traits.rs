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

/// A state that can join with another replica's copy directly.
///
/// This is opt-in and separate from [`State`]. Most states never merge, and
/// replication handles them through operations. It exists for CRDT-style
/// states such as `tpt-lattice` cells, where joining two copies is the only
/// sync step.
///
/// # Contract
///
/// `merge` must be a join: commutative, associative and idempotent. Callers
/// rely on those three properties to converge without coordination.
pub trait MergeableState: State {
    /// Joins `other` into `self`.
    fn merge(&mut self, other: &Self);
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

#[cfg(test)]
mod tests {
    use super::{MergeableState, State};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MaxRegister(u64);

    impl State for MaxRegister {}

    impl MergeableState for MaxRegister {
        fn merge(&mut self, other: &Self) {
            self.0 = self.0.max(other.0);
        }
    }

    #[test]
    fn merge_is_commutative_associative_and_idempotent() {
        let (a, b, c) = (MaxRegister(3), MaxRegister(9), MaxRegister(5));

        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);
        assert_eq!(ab, ba, "commutative");

        let mut ab_c = ab.clone();
        ab_c.merge(&c);
        let mut bc = b.clone();
        bc.merge(&c);
        let mut a_bc = a.clone();
        a_bc.merge(&bc);
        assert_eq!(ab_c, a_bc, "associative");

        let mut twice = ab.clone();
        twice.merge(&ab);
        assert_eq!(twice, ab, "idempotent");
    }
}
