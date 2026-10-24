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
use serde::{de::DeserializeOwned, Serialize};

/// The result of committing a local operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed<I> {
    /// The signed entry to broadcast.
    pub entry: SignedEntry,
    /// Undoes the operation. Kept locally for undo. It is never sent.
    pub inverse: I,
}

/// Runs a local operation through the flow and returns the signed entry and
/// its inverse.
///
/// The inverse is built from the pre-application state. Nothing is signed or
/// applied unless validation and the inverse both succeed. If the operation
/// fails to apply, `state` is unchanged and no entry is returned.
pub fn commit<S, O, D, G>(
    state: &mut S,
    op: &O,
    actor: ActorId,
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
    let entry = sign_op(actor, op, digest, signer)?;
    let mut next = state.clone();
    op.apply(&mut next)?;
    *state = next;
    Ok(Committed { entry, inverse })
}

/// Verifies, decodes and applies a remote entry. Returns the author.
///
/// Nothing is decoded until the signature checks. `state` changes only if
/// validation and application both succeed.
pub fn receive<S, O, D, V>(
    state: &mut S,
    entry: &SignedEntry,
    digest: &D,
    verifier: &V,
) -> Result<ActorId, OpError>
where
    S: State,
    O: Operation<S> + DeserializeOwned,
    D: Digest,
    V: Verifier,
{
    let (actor, op): (ActorId, O) = verify_entry(entry, digest, verifier)?;
    op.validate(state, &actor)?;
    let mut next = state.clone();
    op.apply(&mut next)?;
    *state = next;
    Ok(actor)
}
