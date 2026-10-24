//! Signing boundary for operation logs (Phase 4).
//!
//! The traits here are the boundary. The `tpt` submodule adapts them to
//! `tpt-crypto` (SHA-256 and Ed25519), which is pure Rust with no C-FFI.
//!
//! Each signed entry records the version of the digest that covers it. Version
//! 1 hashes the entry's postcard encoding with SHA-256. The spec says the
//! digest should cover the canonical `tpt-uir` form instead. That will be a
//! new version, so existing signatures stay verifiable.

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

    /// Identifies what this digest hashes and how. Stored in each
    /// [`SignedEntry`], so a verifier can refuse entries it cannot check.
    /// Change it whenever the digest input or the algorithm changes.
    fn version(&self) -> u8;
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
    /// The author's sequence number for this entry. Receivers use it to refuse
    /// replays, see [`crate::pipeline::ReplayGuard`].
    pub seq: u64,
    /// The [`Digest::version`] that covers the entry.
    pub digest_version: u8,
    /// Signature over the digest of the signing input. The input is a domain
    /// tag, `digest_version`, `seq` and `bytes`, so none of them can be changed
    /// without breaking the signature.
    pub signature: Vec<u8>,
}

/// Domain tag, so a signature made here cannot be valid for another protocol.
const SIGNING_DOMAIN: &[u8] = b"tpt-operations/signed-entry\0";

/// The bytes that are hashed and signed for an entry.
fn signing_input(digest_version: u8, seq: u64, bytes: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(SIGNING_DOMAIN.len() + 1 + 8 + bytes.len());
    input.extend_from_slice(SIGNING_DOMAIN);
    input.push(digest_version);
    input.extend_from_slice(&seq.to_be_bytes());
    input.extend_from_slice(bytes);
    input
}

/// Encodes `op` for `actor` and signs the result.
///
/// `seq` must increase with every entry the actor signs. Receivers refuse an
/// entry whose `seq` is not above the last one they accepted from that actor.
pub fn sign_op<O, D, G>(
    actor: ActorId,
    seq: u64,
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
    let digest_version = digest.version();
    let signature = signer.sign(&digest.digest(&signing_input(digest_version, seq, &bytes)))?;
    Ok(SignedEntry {
        bytes,
        seq,
        digest_version,
        signature,
    })
}

/// Verifies `entry` and decodes its actor and operation.
///
/// The signature is checked before any decoding, so unsigned or tampered
/// bytes are never parsed. The returned actor is only what the author claimed.
/// The signature proves a key signed it, not that the key belongs to that
/// actor, so the caller must check that. [`crate::pipeline::receive`] does.
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
    if entry.digest_version != digest.version() {
        return Err(OpError::Validation(format!(
            "digest version {} is not supported; expected {}",
            entry.digest_version,
            digest.version()
        )));
    }
    let input = signing_input(entry.digest_version, entry.seq, &entry.bytes);
    verifier.verify(&digest.digest(&input), &entry.signature)?;

    // The version is read first, so a newer layout is reported as such.
    let (version, tail) = postcard::take_from_bytes::<u16>(&entry.bytes)
        .map_err(|e| OpError::Serialization(format!("decode failed: {e}")))?;
    if version != FORMAT_VERSION {
        return Err(OpError::Serialization(format!(
            "unsupported format version {version}"
        )));
    }
    let ((actor, op), rest) = postcard::take_from_bytes::<(ActorId, O)>(tail)
        .map_err(|e| OpError::Serialization(format!("decode failed: {e}")))?;
    if !rest.is_empty() {
        return Err(OpError::Serialization(format!(
            "{} trailing bytes after entry",
            rest.len()
        )));
    }
    Ok((actor, op))
}

/// Adapters over `tpt-crypto`: SHA-256 digests and Ed25519 signatures.
#[cfg(feature = "crypto")]
pub mod tpt {
    use super::{Digest, Signer, Verifier};
    use crate::core::OpError;
    use alloc::{format, string::ToString, vec::Vec};
    use tpt_crypto_sig::ed25519::{Signature, SigningKey, VerifyingKey, SIGNATURE_LEN};

    /// SHA-256 from `tpt-crypto-hash`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Sha256;

    /// Digest version for SHA-256 over an entry's postcard encoding.
    pub const SHA256_POSTCARD_VERSION: u8 = 1;

    impl Digest for Sha256 {
        fn digest(&self, bytes: &[u8]) -> Vec<u8> {
            tpt_crypto_hash::sha2::sha256(bytes).to_vec()
        }

        fn version(&self) -> u8 {
            SHA256_POSTCARD_VERSION
        }
    }

    /// Ed25519 signer (RFC 8032) from `tpt-crypto-sig`.
    pub struct Ed25519Signer {
        key: SigningKey,
    }

    impl Ed25519Signer {
        /// Builds a signer from a 32-byte seed.
        pub fn from_seed(seed: [u8; 32]) -> Self {
            Self {
                key: SigningKey::from_seed(seed),
            }
        }

        /// The matching public key, as 32 bytes.
        pub fn public_key(&self) -> [u8; 32] {
            self.key.verifying_key().to_bytes()
        }
    }

    impl Signer for Ed25519Signer {
        fn sign(&self, digest: &[u8]) -> Result<Vec<u8>, OpError> {
            Ok(self.key.sign(digest).to_bytes().to_vec())
        }
    }

    /// Ed25519 verifier from `tpt-crypto-sig`.
    #[derive(Debug, Clone, Copy)]
    pub struct Ed25519Verifier {
        key: VerifyingKey,
    }

    impl Ed25519Verifier {
        /// Builds a verifier from a 32-byte public key. Rejects invalid encodings.
        pub fn from_bytes(public_key: &[u8; 32]) -> Result<Self, OpError> {
            let key = VerifyingKey::from_bytes(public_key)
                .map_err(|e| OpError::Validation(format!("bad public key: {e}")))?;
            Ok(Self { key })
        }
    }

    impl Verifier for Ed25519Verifier {
        fn verify(&self, digest: &[u8], signature: &[u8]) -> Result<(), OpError> {
            let sig: [u8; SIGNATURE_LEN] = signature
                .try_into()
                .map_err(|_| OpError::Validation("signature must be 64 bytes".to_string()))?;
            self.key
                .verify(digest, &Signature::from_bytes(sig))
                .map_err(|_| OpError::Validation("signature mismatch".to_string()))
        }
    }
}
