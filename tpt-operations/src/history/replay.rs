//! Event sourcing: rebuild a state from an empty start and a serialized operation log.
//!
//! A log is a sequence of postcard-encoded entries `(version, actor, op)`
//! written back to back, with no outer framing. The version tag lets the
//! entry layout change later without breaking older logs.
//!
//! A decode failure always stops the replay, because the position of the
//! next entry is unknown once an entry cannot be read. Apply failures follow
//! the [`ReplayPolicy`].

use crate::{
    compose::OperationBatch,
    core::{ActorId, OpError, Operation, State},
};
use alloc::{format, string::String, vec, vec::Vec};
use serde::{de::DeserializeOwned, Serialize};

/// Version tag written into every log entry. Bump it when the entry layout changes.
pub const FORMAT_VERSION: u16 = 1;

/// What the engine does when an operation fails validation or application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayPolicy {
    /// Stop at the first failing entry and return its error.
    FailFast,
    /// Record the failure in the trace, leave the state unchanged, and continue.
    SkipInvalid,
}

/// Why a replay stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayError {
    /// The entry at `index` could not be decoded. This is always fatal.
    Decode {
        /// Position of the entry in the log, starting at 0.
        index: usize,
        /// What the decoder reported.
        message: String,
    },
    /// The entry at `index` uses a format version this build does not know.
    UnsupportedVersion {
        /// Position of the entry in the log, starting at 0.
        index: usize,
        /// The version tag found in the entry.
        version: u16,
    },
    /// The operation at `index` failed under [`ReplayPolicy::FailFast`].
    Apply {
        /// Position of the entry in the log, starting at 0.
        index: usize,
        /// The operation's error.
        error: OpError,
    },
}

impl core::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReplayError::Decode { index, message } => {
                write!(f, "entry {index}: decode failed: {message}")
            }
            ReplayError::UnsupportedVersion { index, version } => {
                write!(f, "entry {index}: unsupported format version {version}")
            }
            ReplayError::Apply { index, error } => write!(f, "entry {index}: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReplayError {}

/// The result for one log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The operation was applied.
    Applied,
    /// The operation failed and was skipped. The state is unchanged.
    Skipped(OpError),
}

/// One line of the audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    /// Position of the entry in the log, starting at 0.
    pub index: usize,
    /// Actor recorded with the entry.
    pub actor: ActorId,
    /// What happened to the operation.
    pub outcome: Outcome,
}

/// Ordered audit trail of a replay. Its `Display` output is one line per entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayTrace {
    /// Entries in log order.
    pub entries: Vec<TraceEntry>,
}

impl core::fmt::Display for ReplayTrace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for entry in &self.entries {
            match &entry.outcome {
                Outcome::Applied => {
                    writeln!(f, "#{} actor={} applied", entry.index, entry.actor.0)?
                }
                Outcome::Skipped(error) => writeln!(
                    f,
                    "#{} actor={} skipped: {error}",
                    entry.index, entry.actor.0
                )?,
            }
        }
        Ok(())
    }
}

/// Appends one encoded entry for `op` to `log`.
pub fn encode_entry<O: Serialize>(
    actor: ActorId,
    op: &O,
    log: &mut Vec<u8>,
) -> Result<(), OpError> {
    let bytes = postcard::to_allocvec(&(FORMAT_VERSION, actor, op))
        .map_err(|e| OpError::Serialization(format!("encode failed: {e}")))?;
    log.extend_from_slice(&bytes);
    Ok(())
}

/// Rebuilds states from operation logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayEngine {
    policy: ReplayPolicy,
}

impl Default for ReplayEngine {
    /// An engine that stops at the first failed entry ([`ReplayPolicy::FailFast`]).
    fn default() -> Self {
        Self::new(ReplayPolicy::FailFast)
    }
}

impl ReplayEngine {
    /// Creates an engine with the given failure policy.
    pub fn new(policy: ReplayPolicy) -> Self {
        Self { policy }
    }

    /// Applies every entry of `log` to `initial`, in order.
    ///
    /// Returns the final state and an audit trail. Replay is deterministic:
    /// the same log and policy always give the same state and trace.
    pub fn replay<S, O>(&self, initial: S, log: &[u8]) -> Result<(S, ReplayTrace), ReplayError>
    where
        S: State,
        O: Operation<S> + DeserializeOwned,
    {
        let mut state = initial;
        let mut trace = ReplayTrace::default();
        let mut rest = log;
        let mut index = 0;

        while !rest.is_empty() {
            // The version comes first, so a layout from a newer format reports
            // `UnsupportedVersion` instead of failing to decode.
            let (version, tail) =
                postcard::take_from_bytes::<u16>(rest).map_err(|e| ReplayError::Decode {
                    index,
                    message: format!("{e}"),
                })?;
            if version != FORMAT_VERSION {
                return Err(ReplayError::UnsupportedVersion { index, version });
            }
            let ((actor, op), tail) =
                postcard::take_from_bytes::<(ActorId, O)>(tail).map_err(|e| {
                    ReplayError::Decode {
                        index,
                        message: format!("{e}"),
                    }
                })?;
            rest = tail;

            // A batch applies to a copy of the state, so a skipped entry leaves no trace in it.
            match OperationBatch::from_ops(vec![op]).apply(&mut state, &actor) {
                Ok(_) => trace.entries.push(TraceEntry {
                    index,
                    actor,
                    outcome: Outcome::Applied,
                }),
                Err(error) => match self.policy {
                    ReplayPolicy::FailFast => return Err(ReplayError::Apply { index, error }),
                    ReplayPolicy::SkipInvalid => trace.entries.push(TraceEntry {
                        index,
                        actor,
                        outcome: Outcome::Skipped(error),
                    }),
                },
            }
            index += 1;
        }

        Ok((state, trace))
    }
}
