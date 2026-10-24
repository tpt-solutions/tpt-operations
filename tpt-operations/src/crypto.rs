//! Signing boundary for operation logs (Phase 4).
//!
//! This module defines the traits only. The real hash and signature
//! algorithms come from `tpt-crypto` through an adapter, which keeps this
//! crate pure Rust with no C-FFI.
//!
//! The digest is taken over an entry's postcard encoding. The spec says the
//! digest should cover the canonical `tpt-uir` form instead. Swap in that
//! input once the `uir` adapter exists. The trait boundary does not change.

use crate::{
    core::{ActorId, OpError},
    history::replay::{encode_entry, FORMAT_VERSION},
};
use alloc::{format, vec::Vec};
use serde::{de::DeserializeOwned, Serialize};

/// Hashes the bytes of an encoded entry.
pub trait Digest {
    /// Digest of `bytes`. Equal inputs must give equal outputs on every platform.
    fn digest(&self, bytes: &[u8]) -> Vec<u8>;
}

/// Produces a signature over a digest.
pub trait Signer {
    /// Signs `digest`.
    fn sign(&self, digest: &[u8]) -> Result<Vec<u8>, OpError>;
}

/// Checks a signature over a digest.
pub trait Verifier {
    /// Returns `Ok(())` only if `signature` is valid for `digest`.
    fn verify(&self, digest: &[u8], signature: &[u8]) -> Result<(), OpError>;
}

/// An encoded log entry together with its signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEntry {
    /// The entry bytes, as written by [`encode_entry`].
    pub bytes: Vec<u8>,
    /// Signature over the digest of `bytes`.
    pub signature: Vec<u8>,
}

/// Encodes `op` for `actor` and signs the result.
pub fn sign_op<O, D, G>(
    actor: ActorId,
    op: &O,
    digest: &D,
    signer: &G,
) -> Result<SignedEntry, OpError>
where
    O: Serialize,
    D: Digest,
    G: Signer,
{
    let mut bytes = Vec::new();
    encode_entry(actor, op, &mut bytes)?;
    let signature = signer.sign(&digest.digest(&bytes))?;
    Ok(SignedEntry { bytes, signature })
}

/// Verifies `entry` and decodes its actor and operation.
///
/// The signature is checked before any decoding, so unsigned or tampered
/// bytes are never parsed.
pub fn verify_entry<O, D, V>(
    entry: &SignedEntry,
    digest: &D,
    verifier: &V,
) -> Result<(ActorId, O), OpError>
where
    O: DeserializeOwned,
    D: Digest,
    V: Verifier,
{
    verifier.verify(&digest.digest(&entry.bytes), &entry.signature)?;

    let ((version, actor, op), rest) = postcard::take_from_bytes::<(u16, ActorId, O)>(&entry.bytes)
        .map_err(|e| OpError::Serialization(format!("decode failed: {e}")))?;
    if !rest.is_empty() {
        return Err(OpError::Serialization(format!(
            "{} trailing bytes after entry",
            rest.len()
        )));
    }
    if version != FORMAT_VERSION {
        return Err(OpError::Serialization(format!(
            "unsupported format version {version}"
        )));
    }
    Ok((actor, op))
}
