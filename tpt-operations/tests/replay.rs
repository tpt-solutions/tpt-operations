//! Replay engine and the golden operation log.
//!
//! The golden log is `tests/golden/replay_1000.bin`. Regenerate it with
//! `TPT_UPDATE_GOLDEN=1 cargo test --all-features --test replay`, then update
//! `EXPECTED_DIGEST` if the state encoding changed.

#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tpt_operations::{
    core::{ActorId, OpError, Operation as _, State},
    history::replay::{encode_entry, FORMAT_VERSION},
    history::{ReplayEngine, ReplayError, ReplayPolicy},
};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Map {
    entries: BTreeMap<String, i64>,
}

impl State for Map {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum MapOp {
    Insert { key: String, value: i64 },
    Remove { key: String },
}

impl tpt_operations::core::Operation<Map> for MapOp {
    type Inverse = MapOp;

    fn apply(&self, state: &mut Map) -> Result<(), OpError> {
        self.validate(state, &ActorId(0))?;
        match self {
            MapOp::Insert { key, value } => {
                state.entries.insert(key.clone(), *value);
            }
            MapOp::Remove { key } => {
                state.entries.remove(key);
            }
        }
        Ok(())
    }

    fn inverse(&self, state: &Map) -> Result<MapOp, OpError> {
        let key = match self {
            MapOp::Insert { key, .. } | MapOp::Remove { key } => key.clone(),
        };
        Ok(match state.entries.get(&key) {
            Some(&value) => MapOp::Insert { key, value },
            None => MapOp::Remove { key },
        })
    }

    fn validate(&self, state: &Map, _actor: &ActorId) -> Result<(), OpError> {
        match self {
            MapOp::Remove { key } if !state.entries.contains_key(key) => {
                Err(OpError::NotFound(key.clone()))
            }
            _ => Ok(()),
        }
    }
}

/// Small xorshift generator. Fixed seed, so the golden log is reproducible.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const GOLDEN_SEED: u64 = 0x1000_0000_0000_0001;
const GOLDEN_OPS: usize = 1_000;
/// FNV-1a 64 of the postcard encoding of the state after replaying the golden log.
const EXPECTED_DIGEST: u64 = 7_674_852_373_501_309_116;

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/replay_1000.bin")
}

/// Generates `GOLDEN_OPS` operations, all valid when applied in order.
/// Each op is checked against a shadow state, so the log replays without failures.
fn generate_golden_ops() -> Vec<(ActorId, MapOp)> {
    let mut rng = Xorshift(GOLDEN_SEED);
    let mut shadow = Map::default();
    let mut ops = Vec::with_capacity(GOLDEN_OPS);
    for i in 0..GOLDEN_OPS {
        let key = format!("k{}", rng.next() % 8);
        let mut op = if rng.next().is_multiple_of(3) {
            MapOp::Remove { key: key.clone() }
        } else {
            MapOp::Insert {
                key: key.clone(),
                value: (rng.next() % 2001) as i64 - 1000,
            }
        };
        if op.validate(&shadow, &ActorId(0)).is_err() {
            op = MapOp::Insert {
                key,
                value: i as i64,
            };
        }
        op.apply(&mut shadow).unwrap();
        ops.push((ActorId((i % 3) as u64 + 1), op));
    }
    ops
}

fn encode_log(ops: &[(ActorId, MapOp)]) -> Vec<u8> {
    let mut log = Vec::new();
    for (actor, op) in ops {
        encode_entry(*actor, op, &mut log).unwrap();
    }
    log
}

/// FNV-1a 64 over the postcard encoding of `state`.
fn digest(state: &Map) -> u64 {
    let bytes = postcard::to_allocvec(state).unwrap();
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn replay_fail_fast(
    log: &[u8],
) -> Result<(Map, tpt_operations::history::ReplayTrace), ReplayError> {
    ReplayEngine::new(ReplayPolicy::FailFast).replay::<Map, MapOp>(Map::default(), log)
}

#[test]
fn golden_log_matches_the_generated_sequence() {
    let log = encode_log(&generate_golden_ops());

    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_path().parent().unwrap()).unwrap();
        std::fs::write(golden_path(), &log).unwrap();
    }

    let golden =
        std::fs::read(golden_path()).expect("golden log missing; run with TPT_UPDATE_GOLDEN=1");
    assert_eq!(
        log, golden,
        "encoding changed: the golden log no longer matches"
    );
}

#[test]
fn golden_log_replays_to_the_expected_state() {
    let golden = std::fs::read(golden_path()).expect("golden log missing");
    let (state, trace) = replay_fail_fast(&golden).unwrap();

    assert_eq!(trace.entries.len(), GOLDEN_OPS);
    assert!(trace
        .entries
        .iter()
        .all(|e| e.outcome == tpt_operations::history::replay::Outcome::Applied));
    assert_eq!(digest(&state), EXPECTED_DIGEST, "state digest changed");
}

#[test]
fn replay_is_deterministic() {
    let golden = std::fs::read(golden_path()).expect("golden log missing");
    let first = replay_fail_fast(&golden).unwrap();
    let second = replay_fail_fast(&golden).unwrap();
    assert_eq!(first, second);
}

#[test]
fn fail_fast_stops_at_the_first_invalid_entry() {
    let log = encode_log(&[
        (
            ActorId(1),
            MapOp::Insert {
                key: "a".into(),
                value: 1,
            },
        ),
        (ActorId(2), MapOp::Remove { key: "z".into() }),
        (
            ActorId(3),
            MapOp::Insert {
                key: "b".into(),
                value: 2,
            },
        ),
    ]);
    match replay_fail_fast(&log) {
        Err(ReplayError::Apply { index, error }) => {
            assert_eq!(index, 1);
            assert_eq!(error, OpError::NotFound("z".into()));
        }
        other => panic!("expected an apply error, got {other:?}"),
    }
}

#[test]
fn skip_invalid_records_the_failure_and_continues() {
    let log = encode_log(&[
        (
            ActorId(1),
            MapOp::Insert {
                key: "a".into(),
                value: 1,
            },
        ),
        (ActorId(2), MapOp::Remove { key: "z".into() }),
        (
            ActorId(3),
            MapOp::Insert {
                key: "b".into(),
                value: 2,
            },
        ),
    ]);
    let (state, trace) = ReplayEngine::new(ReplayPolicy::SkipInvalid)
        .replay::<Map, MapOp>(Map::default(), &log)
        .unwrap();

    assert_eq!(state.entries.get("a"), Some(&1));
    assert_eq!(state.entries.get("b"), Some(&2));
    assert_eq!(trace.entries.len(), 3);
    assert_eq!(trace.entries[1].actor, ActorId(2));
    assert!(matches!(
        trace.entries[1].outcome,
        tpt_operations::history::replay::Outcome::Skipped(OpError::NotFound(_))
    ));

    let text = trace.to_string();
    assert!(
        text.contains("#1 actor=2 skipped: not found error: z"),
        "{text}"
    );
}

#[test]
fn unknown_format_version_is_rejected() {
    let bytes = postcard::to_allocvec(&(
        FORMAT_VERSION + 1,
        ActorId(1),
        MapOp::Remove { key: "a".into() },
    ))
    .unwrap();
    match replay_fail_fast(&bytes) {
        Err(ReplayError::UnsupportedVersion { index, version }) => {
            assert_eq!(index, 0);
            assert_eq!(version, FORMAT_VERSION + 1);
        }
        other => panic!("expected a version error, got {other:?}"),
    }
}

#[test]
fn truncated_log_is_a_decode_error_even_in_skip_mode() {
    let golden = std::fs::read(golden_path()).expect("golden log missing");
    let truncated = &golden[..golden.len() - 3];
    let result = ReplayEngine::new(ReplayPolicy::SkipInvalid)
        .replay::<Map, MapOp>(Map::default(), truncated);
    match result {
        Err(ReplayError::Decode { index, .. }) => assert_eq!(index, GOLDEN_OPS - 1),
        other => panic!("expected a decode error, got {other:?}"),
    }
}

#[test]
fn empty_log_gives_the_initial_state() {
    let initial = Map::default();
    let (state, trace) = replay_fail_fast(&[]).unwrap();
    assert_eq!(state, initial);
    assert!(trace.entries.is_empty());
}

#[test]
fn a_newer_entry_layout_is_reported_as_an_unsupported_version() {
    // Version 2 followed by bytes that do not decode as an actor and an op.
    let mut bytes = postcard::to_allocvec(&(FORMAT_VERSION + 1)).unwrap();
    bytes.extend_from_slice(&[0xFF; 3]);
    match replay_fail_fast(&bytes) {
        Err(ReplayError::UnsupportedVersion { index, version }) => {
            assert_eq!((index, version), (0, FORMAT_VERSION + 1));
        }
        other => panic!("expected a version error, got {other:?}"),
    }
}

#[test]
fn trailing_garbage_is_a_decode_error_at_the_next_index() {
    let mut log = encode_log(&[(
        ActorId(1),
        MapOp::Insert {
            key: "a".into(),
            value: 1,
        },
    )]);
    log.extend_from_slice(&[0xFF; 2]);
    match replay_fail_fast(&log) {
        Err(ReplayError::Decode { index, .. }) => assert_eq!(index, 1),
        other => panic!("expected a decode error, got {other:?}"),
    }
}
