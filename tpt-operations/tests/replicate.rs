//! Grouping of merges, checked through the tpt-replicate merge executor.
//!
//! A remote batch can arrive whole, split into smaller batches, or one
//! operation at a time. For an operation type whose `IS_ASSOCIATIVE` marker is
//! true, every grouping must reach the same state as the author's own
//! sequential state.

#![cfg(feature = "uir")]

#[allow(dead_code)]
mod common;

use common::{Counter, CounterOp};
use tpt_operations::{
    core::State as OpState,
    crdt::CrdtCompatible,
    uir::{from_node, to_node, UirNode},
};
use tpt_replicate::{
    clock::{ManualClock, Timestamp},
    engine::Replica,
    error::{ApplyError, SnapshotError},
    log::{ReplicatedLog, StampedOperation},
    merge::{AllowAll, MergeLimits, Merger},
    stubs::{
        crypto::KeyedHashVerifier, ActorId as ReplicaId, CrdtCompatible as ReplicaCrdt,
        State as ReplicaState, UirNode as ReplicaNode,
    },
};

/// Random splits per generated op sequence.
const SPLITS: usize = 8;
/// Generated op sequences.
const CASES: usize = 200;
/// Shared key for the stand-in signatures.
const SECRET: u64 = 42;
/// Local time on the receiving side. It is ahead of every author timestamp, so
/// the drift check passes.
const RECEIVE_AT: u64 = 10_000;

const AUTHOR: ReplicaId = ReplicaId(1);

/// Runs a [`Counter`] under the replication engine. Each replicated payload is a
/// tpt-operations node.
#[derive(Clone, Debug, PartialEq)]
struct Bridge(Counter);

impl ReplicaCrdt for Bridge {
    fn apply(&mut self, _stamp: &Timestamp, payload: &ReplicaNode) -> Result<(), ApplyError> {
        let op: CounterOp = from_node(&UirNode {
            bytes: payload.bytes.clone(),
        })
        .map_err(|_| ApplyError::MalformedOp)?;
        OpState::apply(&mut self.0, &op).map_err(|_| ApplyError::Invalid("counter refused"))
    }
}

impl ReplicaState for Bridge {
    // Injective over i64, so equal hashes mean equal counter values.
    fn state_hash(&self) -> u64 {
        self.0.value as u64
    }

    fn compact(&mut self, _cutoff: &Timestamp) {}

    fn encode_state(&self) -> Vec<u8> {
        self.0.value.to_le_bytes().to_vec()
    }

    fn decode_state(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let raw: [u8; 8] = bytes.try_into().map_err(|_| SnapshotError::Malformed)?;
        Ok(Bridge(Counter {
            value: i64::from_le_bytes(raw),
        }))
    }
}

/// Applies `ops` on an author replica and returns the stamped operations with
/// the author's resulting state.
fn author_run(ops: &[CounterOp], clock: &ManualClock) -> (Vec<StampedOperation>, Bridge) {
    let mut author = Replica::builder(AUTHOR, Bridge(Counter::default()))
        .shared_secret(SECRET)
        .clock(Box::new(clock.clone()))
        .build()
        .unwrap();
    let mut stamped = Vec::with_capacity(ops.len());
    for op in ops {
        clock.advance(1);
        let payload = to_node(op).unwrap();
        stamped.push(
            author
                .local_op(ReplicaNode {
                    bytes: payload.bytes,
                })
                .unwrap(),
        );
    }
    (stamped, author.state().clone())
}

/// Delivers `batches` in order to a fresh replica and returns its final state.
/// Every operation must be applied, and nothing may be left waiting.
fn deliver(batches: &[&[StampedOperation]]) -> Bridge {
    let verifier = KeyedHashVerifier::new(SECRET);
    let mut merger = Merger::new(MergeLimits::default());
    let mut state = Bridge(Counter::default());
    let mut log = ReplicatedLog::new();
    let mut applied = 0;
    for batch in batches {
        let report = merger
            .apply_remote_ops(
                &verifier,
                &AllowAll,
                RECEIVE_AT,
                &mut state,
                &mut log,
                batch.to_vec(),
            )
            .unwrap();
        applied += report.applied;
    }
    assert_eq!(applied, batches.iter().map(|b| b.len()).sum::<usize>());
    assert_eq!(merger.pending_len(), 0);
    state
}

/// Cuts `ops` into contiguous batches, cutting after each position with
/// probability one half.
fn random_split<'a>(ops: &'a [StampedOperation], rng: &mut Rng) -> Vec<&'a [StampedOperation]> {
    let mut batches = Vec::new();
    let mut start = 0;
    for end in 1..=ops.len() {
        if end == ops.len() || rng.below(2) == 0 {
            batches.push(&ops[start..end]);
            start = end;
        }
    }
    batches
}

fn random_op(rng: &mut Rng) -> CounterOp {
    if rng.below(2) == 0 {
        CounterOp::Add(rng.range_i64(-500, 500))
    } else {
        CounterOp::Set(rng.range_i64(-1_000, 1_000))
    }
}

/// Seeded xorshift64*. Enough for generating test inputs.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo) as u64 + 1) as i64
    }
}

#[test]
fn every_grouping_reaches_the_authors_state() {
    // Checked at compile time: the property below only means something if the
    // marker is declared.
    const { assert!(<CounterOp as CrdtCompatible<Counter>>::IS_ASSOCIATIVE) };

    let mut rng = Rng(0x00A5_50C1_A7E5_EED1);
    for _ in 0..CASES {
        let len = 1 + rng.below(12) as usize;
        let ops: Vec<CounterOp> = (0..len).map(|_| random_op(&mut rng)).collect();
        let clock = ManualClock::new(1_000);
        let (stamped, author_state) = author_run(&ops, &clock);

        assert_eq!(deliver(&[&stamped]), author_state, "one batch");

        let singles: Vec<&[StampedOperation]> = stamped.chunks(1).collect();
        assert_eq!(deliver(&singles), author_state, "one op per batch");

        for _ in 0..SPLITS {
            let batches = random_split(&stamped, &mut rng);
            assert_eq!(
                deliver(&batches),
                author_state,
                "split into {} batches",
                batches.len()
            );
        }
    }
}
