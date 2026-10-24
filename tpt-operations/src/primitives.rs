//! Maps [`ActorId`] to and from `tpt-primitives`' [`Principal`].
//!
//! `tpt-primitives` treats a principal as an opaque label and leaves the
//! mapping to actors to the ecosystem at its boundary. That boundary is this
//! module. The label is `actor:<n>`, and only that exact form is accepted, so
//! every principal has one canonical actor.

use crate::core::{ActorId, OpError};
use alloc::{format, string::ToString};
use tpt_primitives::capability::Principal;

const PREFIX: &str = "actor:";

/// The principal for `actor`.
pub fn principal_of(actor: ActorId) -> Principal {
    // The label is never empty, so construction cannot fail.
    Principal::new(format!("{PREFIX}{}", actor.0)).expect("actor label is never empty")
}

/// The actor for `principal`. Fails unless the label is exactly `actor:<n>`.
pub fn actor_of(principal: &Principal) -> Result<ActorId, OpError> {
    let digits = principal
        .label()
        .strip_prefix(PREFIX)
        .ok_or_else(|| OpError::Validation("principal is not an actor label".to_string()))?;
    let n: u64 = digits
        .parse()
        .map_err(|_| OpError::Validation(format!("bad actor number: {digits}")))?;
    // Rejects leading zeros and a leading `+`, so each actor has one label.
    if n.to_string() != digits {
        return Err(OpError::Validation(format!(
            "non-canonical actor label: {digits}"
        )));
    }
    Ok(ActorId(n))
}
