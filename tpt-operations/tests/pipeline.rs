//! The action flow end to end: commit on an author, receive on a peer, and
//! replay the same entries from the log.

#![cfg(feature = "crypto")]

#[allow(dead_code)]
mod common;

use common::{Counter, CounterOp};
use tpt_operations::{
    core::{ActorId, OpError, State},
    crypto::tpt::{Ed25519Signer, Ed25519Verifier, Sha256},
    history::{ReplayEngine, ReplayPolicy},
    pipeline::{commit, receive},
};

const AUTHOR: ActorId = ActorId(1);

fn author_signer() -> Ed25519Signer {
    Ed25519Signer::from_seed([7; 32])
}

fn author_verifier() -> Ed25519Verifier {
    Ed25519Verifier::from_bytes(&author_signer().public_key()).unwrap()
}

#[test]
fn a_peer_and_a_replay_both_reach_the_authors_state() {
    let ops = [
        CounterOp::Add(5),
        CounterOp::Set(10),
        CounterOp::Add(-3),
        CounterOp::Add(100),
    ];
    let mut author = Counter::default();
    let mut entries = Vec::new();
    for op in &ops {
        let committed = commit(&mut author, op, AUTHOR, &Sha256, &author_signer()).unwrap();
        entries.push(committed.entry);
    }
    assert_eq!(author.value, 107);

    let mut peer = Counter::default();
    for entry in &entries {
        let actor =
            receive::<_, CounterOp, _, _>(&mut peer, entry, &Sha256, &author_verifier()).unwrap();
        assert_eq!(actor, AUTHOR);
    }
    assert_eq!(peer, author);

    let log: Vec<u8> = entries.iter().flat_map(|e| e.bytes.clone()).collect();
    let (replayed, trace) = ReplayEngine::new(ReplayPolicy::FailFast)
        .replay::<Counter, CounterOp>(Counter::default(), &log)
        .unwrap();
    assert_eq!(replayed, author);
    assert_eq!(trace.to_string().lines().count(), ops.len());
}

#[test]
fn each_inverse_restores_the_state_before_its_operation() {
    let mut state = Counter { value: 7 };
    for op in [CounterOp::Add(3), CounterOp::Set(-1), CounterOp::Add(50)] {
        let before = state.clone();
        let committed = commit(&mut state, &op, AUTHOR, &Sha256, &author_signer()).unwrap();
        state.apply(&committed.inverse).unwrap();
        assert_eq!(state, before);
    }
}

#[test]
fn a_refused_local_op_leaves_state_and_produces_no_entry() {
    let mut state = Counter { value: i64::MAX };
    let result = commit(
        &mut state,
        &CounterOp::Add(1),
        AUTHOR,
        &Sha256,
        &author_signer(),
    );
    assert!(matches!(result, Err(OpError::Bounds(_))));
    assert_eq!(state, Counter { value: i64::MAX });
}

#[test]
fn a_tampered_entry_is_refused_and_state_is_unchanged() {
    let mut author = Counter::default();
    let mut entry = commit(
        &mut author,
        &CounterOp::Set(42),
        AUTHOR,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;
    let last = entry.bytes.len() - 1;
    entry.bytes[last] ^= 1;

    let mut peer = Counter::default();
    let result = receive::<_, CounterOp, _, _>(&mut peer, &entry, &Sha256, &author_verifier());
    assert!(result.is_err());
    assert_eq!(peer, Counter::default());
}

#[test]
fn an_entry_signed_by_another_key_is_refused() {
    let mut author = Counter::default();
    let entry = commit(
        &mut author,
        &CounterOp::Add(1),
        AUTHOR,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;

    let stranger =
        Ed25519Verifier::from_bytes(&Ed25519Signer::from_seed([9; 32]).public_key()).unwrap();
    let mut peer = Counter::default();
    assert!(receive::<_, CounterOp, _, _>(&mut peer, &entry, &Sha256, &stranger).is_err());
    assert_eq!(peer, Counter::default());
}
