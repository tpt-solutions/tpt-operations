//! Operation to tpt-uir node conversion (Phase 4).
//!
//! Operations are serialized as IR nodes for network transmission and storage.
//! This module is the local stand-in for `tpt-uir`. [`UirNode`] has the same
//! shape as the stub in `tpt-replicate`, so a node can pass between the two
//! crates unchanged. The payload is the postcard encoding of the operation.
//!
//! When the real `tpt-uir` lands, its canonical encoding replaces postcard
//! here. That also moves the digest from version 1 to version 2 (see `crypto`).

use crate::core::OpError;
use alloc::{format, vec::Vec};
use serde::{de::DeserializeOwned, Serialize};

/// Serialized operation payload, the stand-in for a tpt-uir node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UirNode {
    /// Encoded operation bytes.
    pub bytes: Vec<u8>,
}

/// Encodes `op` as a node.
pub fn to_node<O: Serialize>(op: &O) -> Result<UirNode, OpError> {
    postcard::to_allocvec(op)
        .map(|bytes| UirNode { bytes })
        .map_err(|e| OpError::Serialization(format!("{e}")))
}

/// Decodes a node made by [`to_node`].
///
/// Fails on truncated input, on unknown tags, and on trailing bytes. A node
/// must hold exactly one operation.
pub fn from_node<O: DeserializeOwned>(node: &UirNode) -> Result<O, OpError> {
    let (op, rest) = postcard::take_from_bytes::<O>(&node.bytes)
        .map_err(|e| OpError::Serialization(format!("{e}")))?;
    if !rest.is_empty() {
        return Err(OpError::Serialization(format!(
            "{} trailing bytes after operation",
            rest.len()
        )));
    }
    Ok(op)
}
