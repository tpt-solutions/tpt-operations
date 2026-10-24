//! Rough throughput numbers: history with eviction, replay, and sign/verify.
//!
//! Run it with `cargo bench -p tpt-operations --features crypto`.
//!
//! This is a plain timing loop, with no statistics and no extra dependencies.
//! Use it to spot a change of an order of magnitude, not a few percent. Numbers
//! depend on the machine, so compare runs on the same one.

use serde::{Deserialize, Serialize};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use tpt_operations::{
    crypto::tpt::{Ed25519Signer, Ed25519Verifier, Sha256},
    history::{replay::encode_entry, ReplayEngine, ReplayPolicy},
    pipeline::{commit, receive, ReplayGuard},
    prelude::*,
};

#[derive(Debug, Clone, PartialEq, Default)]
struct Counter {
    value: i64,
}

impl State for Counter {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Set(i64);

impl Operation<Counter> for Set {
    type Inverse = Set;

    fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
        state.value = self.0;
        Ok(())
    }

    fn inverse(&self, state: &Counter) -> Result<Set, OpError> {
        Ok(Set(state.value))
    }

    fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

impl SizeEstimate for Set {
    fn estimated_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Times `work`, which does `count` units of something, and prints the rate.
fn report(name: &str, count: usize, work: impl FnOnce()) {
    let start = Instant::now();
    work();
    let elapsed = start.elapsed();
    println!(
        "{name:<34} {count:>8} in {:>9.2?}  {:>12.0} per second",
        elapsed,
        count as f64
            / elapsed
                .as_secs_f64()
                .max(Duration::from_nanos(1).as_secs_f64())
    );
}

fn main() {
    let me = ActorId(1);

    // History: every execute past the cap also evicts the oldest step.
    const EXECUTES: usize = 200_000;
    report("history execute (cap 1,000 steps)", EXECUTES, || {
        let mut history = HistoryStack::with_limits(1_000, usize::MAX);
        let mut state = Counter::default();
        for n in 0..EXECUTES as i64 {
            history
                .execute(&mut state, &tpt_batch![Set(n)], &me)
                .unwrap();
        }
        black_box(&state);
    });

    report("history undo + redo", 2_000, || {
        let mut history = HistoryStack::new();
        let mut state = Counter::default();
        for n in 0..1_000 {
            history
                .execute(&mut state, &tpt_batch![Set(n)], &me)
                .unwrap();
        }
        while history.undo(&mut state, &me).unwrap() {}
        while history.redo(&mut state, &me).unwrap() {}
        black_box(&state);
    });

    // Replay: decode and apply a log of 200,000 entries.
    const ENTRIES: usize = 200_000;
    let mut log = Vec::new();
    for n in 0..ENTRIES as i64 {
        encode_entry(me, &Set(n), &mut log).unwrap();
    }
    println!(
        "(replay log is {} bytes, {:.1} bytes per entry)",
        log.len(),
        log.len() as f64 / ENTRIES as f64
    );
    report("replay (FailFast)", ENTRIES, || {
        let (state, _) = ReplayEngine::new(ReplayPolicy::FailFast)
            .replay::<Counter, Set>(Counter::default(), &log)
            .unwrap();
        black_box(state);
    });

    // Sign and verify: commit signs, receive verifies and applies.
    const SIGNED: usize = 5_000;
    let signer = Ed25519Signer::from_seed([7; 32]);
    let verifier = Ed25519Verifier::from_bytes(&signer.public_key()).unwrap();
    let mut entries = Vec::with_capacity(SIGNED);
    let mut author = Counter::default();
    report("commit (validate, sign, apply)", SIGNED, || {
        for seq in 1..=SIGNED as u64 {
            let committed =
                commit(&mut author, &Set(seq as i64), me, seq, &Sha256, &signer).unwrap();
            entries.push(committed.entry);
        }
    });
    report("receive (verify, decode, apply)", SIGNED, || {
        let mut peer = Counter::default();
        let mut guard = ReplayGuard::new();
        for entry in &entries {
            receive::<_, Set, _, _>(&mut peer, entry, me, &mut guard, &Sha256, &verifier).unwrap();
        }
        black_box(peer);
    });
}
