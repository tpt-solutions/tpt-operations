//! The action flow, from a local intent to a signed entry, and back on the
//! receiving side.
//!
//! Local: Validate, Inverse, Serialize, Sign, Apply. Remote: Verify, Decode,
//! Validate, Apply. Broadcast is the transport's job. Hand the returned
//! [`SignedEntry`] to `tpt-replicate`, for example through `Replica::local_op`
//! or `Replica::import_ops`.
//!
//! Both paths apply the operation to a copy of the state and commit only on
//! success. [`Operation::apply`] may leave a state partly changed on error, so
//! a failed apply must not reach the caller's state.

use crate::{
    core::{ActorId, OpError, Operation, State},
    crypto::{sign_op, verify_entry, Digest, SignedEntry, Signer, Verifier},
};
use alloc::{collections::BTreeMap, format};
use serde::{de::DeserializeOwned, Serialize};

/// The result of committing a local operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed<I> {
    /// The signed entry to broadcast.
    pub entry: SignedEntry,
    /// Undoes the operation. Kept locally for undo. It is never sent.
    pub inverse: I,
}

/// Remembers the highest sequence number accepted from each author, so a
/// receiver can refuse replayed and reordered entries.
///
/// Sequence numbers must strictly increase per author. Gaps are fine. Entries
/// from one author must reach [`receive`] in order. A transport that can
/// reorder them should buffer by author first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayGuard {
    last: BTreeMap<u64, u64>,
}

impl ReplayGuard {
    /// Creates a guard that has accepted nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// The highest sequence number accepted from `author`, if any.
    pub fn last_seq(&self, author: ActorId) -> Option<u64> {
        self.last.get(&author.0).copied()
    }

    fn check(&self, author: ActorId, seq: u64) -> Result<(), OpError> {
        match self.last_seq(author) {
            Some(last) if seq <= last => Err(OpError::Conflict(format!(
                "entry seq {seq} from actor {} is not above the last accepted seq {last}",
                author.0
            ))),
            _ => Ok(()),
        }
    }

    fn record(&mut self, author: ActorId, seq: u64) {
        self.last.insert(author.0, seq);
    }
}

/// Runs a local operation through the flow and returns the signed entry and
/// its inverse.
///
/// `seq` must be higher than the `seq` of every entry `actor` signed before.
/// The inverse is built from the pre-application state. Nothing is signed or
/// applied unless validation and the inverse both succeed. If the operation
/// fails to apply, `state` is unchanged and no entry is returned.
pub fn commit<S, O, D, G>(
    state: &mut S,
    op: &O,
    actor: ActorId,
    seq: u64,
    digest: &D,
    signer: &G,
) -> Result<Committed<O::Inverse>, OpError>
where
    S: State,
    O: Operation<S> + Serialize,
    D: Digest,
    G: Signer,
{
    op.validate(state, &actor)?;
    let inverse = op.inverse(state)?;
    let entry = sign_op(actor, seq, op, digest, signer)?;
    let mut next = state.clone();
    op.apply(&mut next)?;
    *state = next;
    Ok(Committed { entry, inverse })
}

/// Verifies, decodes and applies a remote entry.
///
/// `author` is the actor that the `verifier`'s key belongs to. The signature
/// only proves that the key signed the entry, so an entry that claims another
/// actor is refused. Without this check, any key holder could act as anyone.
/// An entry whose `seq` is not above the last one accepted from `author` is
/// refused as a replay.
///
/// Nothing is decoded until the signature checks. `state` and `guard` change
/// only if validation and application both succeed.
pub fn receive<S, O, D, V>(
    state: &mut S,
    entry: &SignedEntry,
    author: ActorId,
    guard: &mut ReplayGuard,
    digest: &D,
    verifier: &V,
) -> Result<(), OpError>
where
    S: State,
    O: Operation<S> + DeserializeOwned,
    D: Digest,
    V: Verifier,
{
    let (actor, op): (ActorId, O) = verify_entry(entry, digest, verifier)?;
    if actor != author {
        return Err(OpError::Capability(format!(
            "entry claims actor {} but is signed by the key of actor {}",
            actor.0, author.0
        )));
    }
    guard.check(author, entry.seq)?;
    op.validate(state, &actor)?;
    let mut next = state.clone();
    op.apply(&mut next)?;
    *state = next;
    guard.record(author, entry.seq);
    Ok(())
}
