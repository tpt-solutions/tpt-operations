//! The `tpt-crypto` adapters, checked against published test vectors and
//! run through the signing boundary.

#![cfg(feature = "crypto")]

use serde::{Deserialize, Serialize};
use tpt_operations::{
    core::ActorId,
    crypto::{
        sign_op,
        tpt::{Ed25519Signer, Ed25519Verifier, Sha256, SHA256_POSTCARD_VERSION},
        verify_entry, Digest, Signer, Verifier,
    },
};

fn hex<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

#[test]
fn sha256_matches_fips_180_vector() {
    assert_eq!(
        Sha256.digest(b"abc"),
        hex::<32>("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
}

#[test]
fn ed25519_matches_rfc_8032_test_1() {
    // RFC 8032, section 7.1, TEST 1: the empty message.
    let seed = hex::<32>("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let signer = Ed25519Signer::from_seed(seed);
    assert_eq!(
        signer.public_key(),
        hex::<32>("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
    );
    let sig = signer.sign(b"").unwrap();
    assert_eq!(
        sig,
        hex::<64>(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        )
        .to_vec()
    );
}

#[test]
fn verifier_rejects_bad_lengths_and_bad_signatures() {
    let signer = Ed25519Signer::from_seed([3; 32]);
    let verifier = Ed25519Verifier::from_bytes(&signer.public_key()).unwrap();
    let digest = Sha256.digest(b"op");
    let sig = signer.sign(&digest).unwrap();

    assert!(verifier.verify(&digest, &sig).is_ok());
    assert!(verifier.verify(&digest, &sig[..63]).is_err(), "short");
    assert!(
        verifier
            .verify(&digest, &[sig.as_slice(), &[0]].concat())
            .is_err(),
        "long"
    );

    let mut flipped = sig.clone();
    flipped[0] ^= 1;
    assert!(verifier.verify(&digest, &flipped).is_err(), "tampered");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Note {
    text: String,
}

#[test]
fn signed_entry_round_trips_and_detects_tampering() {
    let signer = Ed25519Signer::from_seed([9; 32]);
    let verifier = Ed25519Verifier::from_bytes(&signer.public_key()).unwrap();
    let op = Note {
        text: "move vertex 4".into(),
    };

    let entry = sign_op(ActorId(42), 1, &op, &Sha256, &signer).unwrap();
    assert_eq!(entry.digest_version, SHA256_POSTCARD_VERSION);
    let (actor, decoded): (ActorId, Note) = verify_entry(&entry, &Sha256, &verifier).unwrap();
    assert_eq!((actor, decoded), (ActorId(42), op));

    let mut tampered = entry.clone();
    let last = tampered.bytes.len() - 1;
    tampered.bytes[last] ^= 0x01;
    assert!(verify_entry::<Note, _, _>(&tampered, &Sha256, &verifier).is_err());
}

#[test]
fn entry_signed_by_another_key_is_rejected() {
    let signer = Ed25519Signer::from_seed([1; 32]);
    let other = Ed25519Signer::from_seed([2; 32]);
    let verifier = Ed25519Verifier::from_bytes(&other.public_key()).unwrap();
    let op = Note { text: "x".into() };

    let entry = sign_op(ActorId(1), 1, &op, &Sha256, &signer).unwrap();
    assert!(verify_entry::<Note, _, _>(&entry, &Sha256, &verifier).is_err());
}

#[test]
fn signatures_of_the_wrong_length_are_rejected() {
    let signer = Ed25519Signer::from_seed([3; 32]);
    let verifier = Ed25519Verifier::from_bytes(&signer.public_key()).unwrap();
    let entry = sign_op(ActorId(1), 1, &Note { text: "x".into() }, &Sha256, &signer).unwrap();
    assert_eq!(entry.signature.len(), 64);

    let mut empty = entry.clone();
    empty.signature.clear();
    let mut short = entry.clone();
    short.signature.truncate(63);
    let mut long = entry.clone();
    long.signature.push(0);

    for bad in [empty, short, long] {
        assert!(verify_entry::<Note, _, _>(&bad, &Sha256, &verifier).is_err());
    }
    assert!(verify_entry::<Note, _, _>(&entry, &Sha256, &verifier).is_ok());
}
