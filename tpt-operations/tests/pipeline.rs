//! The action flow end to end: commit on an author, receive on a peer, and
//! replay the same entries from the log.

#![cfg(feature = "crypto")]

#[allow(dead_code)]
mod common;

use common::{Counter, CounterOp};
use tpt_operations::{
    core::{ActorId, OpError, State},
    crypto::{
        sign_op,
        tpt::{Ed25519Signer, Ed25519Verifier, Sha256},
        Signer,
    },
    history::{ReplayEngine, ReplayPolicy},
    pipeline::{commit, receive, ReplayGuard},
};

const AUTHOR: ActorId = ActorId(1);

fn author_signer() -> Ed25519Signer {
    Ed25519Signer::from_seed([7; 32])
}

fn author_verifier() -> Ed25519Verifier {
    Ed25519Verifier::from_bytes(&author_signer().public_key()).unwrap()
}

/// Receives `entry` as an entry from `AUTHOR`, verified with the author's key.
fn recv(
    state: &mut Counter,
    entry: &tpt_operations::crypto::SignedEntry,
    guard: &mut ReplayGuard,
) -> Result<(), OpError> {
    receive::<_, CounterOp, _, _>(state, entry, AUTHOR, guard, &Sha256, &author_verifier())
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
    for (seq, op) in (1..).zip(&ops) {
        let committed = commit(&mut author, op, AUTHOR, seq, &Sha256, &author_signer()).unwrap();
        entries.push(committed.entry);
    }
    assert_eq!(author.value, 107);

    let mut peer = Counter::default();
    let mut guard = ReplayGuard::new();
    for entry in &entries {
        recv(&mut peer, entry, &mut guard).unwrap();
    }
    assert_eq!(guard.last_seq(AUTHOR), Some(4));
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
        let committed = commit(&mut state, &op, AUTHOR, 1, &Sha256, &author_signer()).unwrap();
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
        1,
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
        1,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;
    let last = entry.bytes.len() - 1;
    entry.bytes[last] ^= 1;

    let mut peer = Counter::default();
    let result = recv(&mut peer, &entry, &mut ReplayGuard::new());
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
        1,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;

    let stranger =
        Ed25519Verifier::from_bytes(&Ed25519Signer::from_seed([9; 32]).public_key()).unwrap();
    let mut peer = Counter::default();
    let mut guard = ReplayGuard::new();
    assert!(receive::<_, CounterOp, _, _>(
        &mut peer, &entry, AUTHOR, &mut guard, &Sha256, &stranger
    )
    .is_err());
    assert_eq!(peer, Counter::default());
}

#[test]
fn a_key_holder_cannot_act_as_another_actor() {
    // The author's key signs an entry that claims to come from actor 99.
    let entry = sign_op(
        ActorId(99),
        1,
        &CounterOp::Set(666),
        &Sha256,
        &author_signer(),
    )
    .unwrap();

    let mut peer = Counter::default();
    let mut guard = ReplayGuard::new();
    let result = recv(&mut peer, &entry, &mut guard);
    assert!(matches!(result, Err(OpError::Capability(_))));
    assert_eq!(peer, Counter::default());
    assert_eq!(guard.last_seq(AUTHOR), None);
    assert_eq!(guard.last_seq(ActorId(99)), None);
}

#[test]
fn a_replayed_entry_is_refused_even_though_its_op_is_not_idempotent() {
    let mut author = Counter::default();
    let entry = commit(
        &mut author,
        &CounterOp::Add(5),
        AUTHOR,
        1,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;

    let mut peer = Counter::default();
    let mut guard = ReplayGuard::new();
    recv(&mut peer, &entry, &mut guard).unwrap();
    let result = recv(&mut peer, &entry, &mut guard);
    assert!(matches!(result, Err(OpError::Conflict(_))));
    assert_eq!(peer.value, 5);
}

#[test]
fn identical_ops_with_increasing_seq_are_both_accepted() {
    let mut author = Counter::default();
    let mut peer = Counter::default();
    let mut guard = ReplayGuard::new();
    for seq in 1..=3 {
        let entry = commit(
            &mut author,
            &CounterOp::Add(1),
            AUTHOR,
            seq,
            &Sha256,
            &author_signer(),
        )
        .unwrap()
        .entry;
        recv(&mut peer, &entry, &mut guard).unwrap();
    }
    assert_eq!(peer.value, 3);
}

#[test]
fn an_older_seq_is_refused_and_a_failed_entry_does_not_advance_the_guard() {
    let mut author = Counter::default();
    let first = commit(
        &mut author,
        &CounterOp::Add(1),
        AUTHOR,
        5,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;
    let stale = commit(
        &mut author,
        &CounterOp::Add(1),
        AUTHOR,
        4,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;

    // An entry that fails to apply must not use up its sequence number.
    let mut peer = Counter { value: i64::MAX };
    let mut guard = ReplayGuard::new();
    assert!(recv(&mut peer, &first, &mut guard).is_err());
    assert_eq!(guard.last_seq(AUTHOR), None);

    let mut peer = Counter::default();
    recv(&mut peer, &first, &mut guard).unwrap();
    assert!(recv(&mut peer, &stale, &mut guard).is_err());
    assert_eq!(peer.value, 1);
}

#[test]
fn a_failing_signer_produces_no_entry_and_leaves_state() {
    struct Broken;
    impl Signer for Broken {
        fn sign(&self, _digest: &[u8]) -> Result<Vec<u8>, OpError> {
            Err(OpError::Capability("key locked".into()))
        }
    }

    let mut state = Counter { value: 3 };
    let result = commit(&mut state, &CounterOp::Add(1), AUTHOR, 1, &Sha256, &Broken);
    assert!(matches!(result, Err(OpError::Capability(_))));
    assert_eq!(state.value, 3);
}

#[test]
fn a_refused_entry_names_the_author_and_sequence_number() {
    let mut author = Counter::default();
    let entry = commit(
        &mut author,
        &CounterOp::Add(1),
        AUTHOR,
        7,
        &Sha256,
        &author_signer(),
    )
    .unwrap()
    .entry;

    let mut peer = Counter { value: i64::MAX };
    let error = recv(&mut peer, &entry, &mut ReplayGuard::new()).unwrap_err();
    assert!(
        matches!(&error, OpError::Bounds(m) if m.contains("seq 7") && m.contains("actor:1")),
        "{error}"
    );
}
