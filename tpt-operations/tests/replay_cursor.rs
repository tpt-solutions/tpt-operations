//! Stepping through a replay log forward and back.

#![cfg(feature = "serde")]

#[allow(dead_code)]
mod common;

use common::{Counter, CounterOp};
use tpt_operations::{
    core::{ActorId, OpError},
    history::{replay::encode_entry, ReplayCursor, ReplayEngine, ReplayError, ReplayPolicy},
};

fn log_of(ops: &[CounterOp]) -> Vec<u8> {
    let mut log = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        encode_entry(ActorId(i as u64 + 1), op, &mut log).unwrap();
    }
    log
}

fn cursor(ops: &[CounterOp]) -> ReplayCursor<Counter, CounterOp> {
    ReplayCursor::new(Counter::default(), &log_of(ops)).unwrap()
}

#[test]
fn every_position_matches_a_fresh_replay_of_the_same_prefix() {
    let ops = [
        CounterOp::Add(5),
        CounterOp::Set(-3),
        CounterOp::Add(100),
        CounterOp::Set(7),
    ];
    let mut cursor = cursor(&ops);
    assert_eq!((cursor.position(), cursor.len()), (0, 4));

    // Forward through the log, then back, then jump around.
    let order = [0, 1, 2, 3, 4, 3, 2, 1, 0, 4, 1, 3, 0, 2];
    for position in order {
        cursor.seek(position).unwrap();
        assert_eq!(cursor.position(), position);

        let (expected, _) = ReplayEngine::new(ReplayPolicy::FailFast)
            .replay::<Counter, CounterOp>(Counter::default(), &log_of(&ops[..position]))
            .unwrap();
        assert_eq!(cursor.state(), &expected, "at position {position}");
    }
}

#[test]
fn stepping_reports_the_ends_and_the_neighbouring_entries() {
    let mut cursor = cursor(&[CounterOp::Add(1), CounterOp::Add(2)]);
    assert!(!cursor.step_back().unwrap(), "nothing before the start");
    assert!(cursor.previous_entry().is_none());
    assert_eq!(cursor.next_entry(), Some((ActorId(1), &CounterOp::Add(1))));

    assert!(cursor.step_forward().unwrap());
    assert_eq!(
        cursor.previous_entry(),
        Some((ActorId(1), &CounterOp::Add(1)))
    );
    assert_eq!(cursor.next_entry(), Some((ActorId(2), &CounterOp::Add(2))));

    assert!(cursor.step_forward().unwrap());
    assert!(!cursor.step_forward().unwrap(), "nothing after the end");
    assert!(cursor.next_entry().is_none());
    assert_eq!(cursor.state().value, 3);
}

#[test]
fn a_failing_entry_stops_the_cursor_and_can_be_skipped() {
    let mut cursor = cursor(&[
        CounterOp::Set(i64::MAX),
        CounterOp::Add(1), // overflows
        CounterOp::Set(9),
    ]);
    assert!(cursor.step_forward().unwrap());

    let err = cursor.step_forward().unwrap_err();
    assert!(matches!(
        err,
        ReplayError::Apply {
            index: 1,
            error: OpError::Bounds(_)
        }
    ));
    assert_eq!(cursor.position(), 1, "the cursor did not move");
    assert_eq!(cursor.state().value, i64::MAX);

    assert!(cursor.skip_forward());
    assert!(cursor.step_forward().unwrap());
    assert_eq!(cursor.state().value, 9);

    // Going back over the skipped entry changes nothing.
    assert!(cursor.step_back().unwrap());
    assert!(cursor.step_back().unwrap());
    assert_eq!(cursor.state().value, i64::MAX);
    assert_eq!(cursor.position(), 1);
}

#[test]
fn seek_stops_before_a_failing_entry_and_clamps_past_the_end() {
    let mut cursor = cursor(&[CounterOp::Set(i64::MAX), CounterOp::Add(1)]);
    assert!(cursor.seek(2).is_err());
    assert_eq!(cursor.position(), 1);

    let mut cursor = self::cursor(&[CounterOp::Add(1)]);
    cursor.seek(99).unwrap();
    assert_eq!(cursor.position(), 1);
}

#[test]
fn an_unreadable_log_is_refused_when_the_cursor_is_made() {
    let mut log = log_of(&[CounterOp::Add(1)]);
    log.extend_from_slice(&[0xFF, 0xFF]);
    let result = ReplayCursor::<Counter, CounterOp>::new(Counter::default(), &log);
    assert!(matches!(result, Err(ReplayError::Decode { index: 1, .. })));
}

#[test]
fn an_empty_log_gives_an_empty_cursor() {
    let mut cursor = cursor(&[]);
    assert!(cursor.is_empty());
    assert!(!cursor.step_forward().unwrap());
    assert!(!cursor.skip_forward());
}
