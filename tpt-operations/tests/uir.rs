//! Operation to `uir` node conversion.

#![cfg(feature = "uir")]

#[allow(dead_code)]
mod common;

use common::CounterOp;
use tpt_operations::{
    core::OpError,
    uir::{from_node, to_node, UirNode},
};

#[test]
fn round_trip_preserves_every_variant() {
    let ops = [
        CounterOp::Add(-7),
        CounterOp::Set(i64::MIN),
        CounterOp::Set(i64::MAX),
    ];
    for op in ops {
        let node = to_node(&op).unwrap();
        assert_eq!(from_node::<CounterOp>(&node).unwrap(), op);
    }
}

#[test]
fn encoding_is_deterministic() {
    let op = CounterOp::Add(42);
    assert_eq!(to_node(&op).unwrap(), to_node(&op).unwrap());
}

#[test]
fn truncated_node_is_rejected() {
    let mut node = to_node(&CounterOp::Set(1_000_000)).unwrap();
    node.bytes.pop();
    assert!(matches!(
        from_node::<CounterOp>(&node),
        Err(OpError::Serialization(_))
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut node = to_node(&CounterOp::Add(1)).unwrap();
    node.bytes.push(0);
    assert!(matches!(
        from_node::<CounterOp>(&node),
        Err(OpError::Serialization(_))
    ));
}

#[test]
fn unknown_variant_is_rejected() {
    // Variant index 9 does not exist on CounterOp.
    let node = UirNode { bytes: vec![9, 0] };
    assert!(matches!(
        from_node::<CounterOp>(&node),
        Err(OpError::Serialization(_))
    ));
}

#[test]
fn empty_node_is_rejected() {
    assert!(matches!(
        from_node::<CounterOp>(&UirNode::default()),
        Err(OpError::Serialization(_))
    ));
}
