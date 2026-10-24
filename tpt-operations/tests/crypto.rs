//! Signing boundary: sign an entry, verify it, and reject tampering.
//!
//! The digest and signer here are toys, for exercising the boundary only.
//! They are not secure. Real algorithms come from `tpt-crypto`.

#![cfg(feature = "crypto")]

use serde::{Deserialize, Serialize};
use tpt_operations::{
    core::{ActorId, OpError},
    crypto::{sign_op, verify_entry, Digest, Signer, Verifier},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Note {
    text: String,
}

fn fnv1a(seed: u64, parts: &[&[u8]]) -> u64 {
    parts.iter().flat_map(|p| p.iter()).fold(seed, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Toy digest: FNV-1a 64, eight bytes out.
struct ToyDigest;

impl Digest for ToyDigest {
    fn digest(&self, bytes: &[u8]) -> Vec<u8> {
        fnv1a(0xcbf2_9ce4_8422_2325, &[bytes])
            .to_be_bytes()
            .to_vec()
    }
}

/// Toy keyed signer: `FNV(key || digest)`. Not a MAC. Test-only.
struct ToySigner {
    key: u64,
}

impl ToySigner {
    fn tag(&self, digest: &[u8]) -> Vec<u8> {
        fnv1a(self.key, &[&self.key.to_be_bytes(), digest])
            .to_be_bytes()
            .to_vec()
    }
}

impl Signer for ToySigner {
    fn sign(&self, digest: &[u8]) -> Result<Vec<u8>, OpError> {
        Ok(self.tag(digest))
    }
}

impl Verifier for ToySigner {
    fn verify(&self, digest: &[u8], signature: &[u8]) -> Result<(), OpError> {
        if self.tag(digest) == signature {
            Ok(())
        } else {
            Err(OpError::Validation("signature mismatch".into()))
        }
    }
}

fn note(text: &str) -> Note {
    Note {
        text: text.to_string(),
    }
}

#[test]
fn signed_entry_verifies_and_decodes() {
    let signer = ToySigner { key: 42 };
    let entry = sign_op(ActorId(3), &note("hello"), &ToyDigest, &signer).unwrap();

    let (actor, decoded): (ActorId, Note) = verify_entry(&entry, &ToyDigest, &signer).unwrap();
    assert_eq!(actor, ActorId(3));
    assert_eq!(decoded, note("hello"));
}

#[test]
fn tampered_bytes_fail_verification() {
    let signer = ToySigner { key: 42 };
    let mut entry = sign_op(ActorId(3), &note("hello"), &ToyDigest, &signer).unwrap();
    let last = entry.bytes.len() - 1;
    entry.bytes[last] ^= 0x01;

    let result: Result<(ActorId, Note), _> = verify_entry(&entry, &ToyDigest, &signer);
    assert!(matches!(result, Err(OpError::Validation(_))));
}

#[test]
fn wrong_key_fails_verification() {
    let entry = sign_op(
        ActorId(3),
        &note("hello"),
        &ToyDigest,
        &ToySigner { key: 42 },
    )
    .unwrap();

    let result: Result<(ActorId, Note), _> =
        verify_entry(&entry, &ToyDigest, &ToySigner { key: 43 });
    assert!(result.is_err());
}

#[test]
fn unsigned_garbage_is_rejected_before_decoding() {
    let signer = ToySigner { key: 42 };
    let entry = tpt_operations::crypto::SignedEntry {
        bytes: vec![0xFF, 0xFF, 0xFF],
        signature: vec![0; 8],
    };
    let result: Result<(ActorId, Note), _> = verify_entry(&entry, &ToyDigest, &signer);
    // The signature check runs first, so the error is a validation failure, not a decode failure.
    assert!(matches!(result, Err(OpError::Validation(_))));
}
