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

/// Decodes the entry at the start of `rest`. Returns its actor, its operation and the bytes after it.
///
/// The version comes first, so a layout from a newer format reports
/// `UnsupportedVersion` instead of failing to decode.
fn decode_entry<O: DeserializeOwned>(
    rest: &[u8],
    index: usize,
) -> Result<(ActorId, O, &[u8]), ReplayError> {
    let decode = |e: postcard::Error| ReplayError::Decode {
        index,
        message: format!("{e}"),
    };
    let (version, tail) = postcard::take_from_bytes::<u16>(rest).map_err(decode)?;
    if version != FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion { index, version });
    }
    let ((actor, op), tail) = postcard::take_from_bytes::<(ActorId, O)>(tail).map_err(decode)?;
    Ok((actor, op, tail))
}

/// Rebuilds states from operation logs.
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tpt_operations::{
///     history::{replay::encode_entry, ReplayEngine, ReplayPolicy},
///     prelude::*,
/// };
///
/// #[derive(Clone, Debug, Default, PartialEq)]
/// struct Counter(i64);
/// impl State for Counter {}
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// struct Set(i64);
/// impl Operation<Counter> for Set {
///     type Inverse = Set;
///     fn apply(&self, s: &mut Counter) -> Result<(), OpError> { s.0 = self.0; Ok(()) }
///     fn inverse(&self, s: &Counter) -> Result<Set, OpError> { Ok(Set(s.0)) }
///     fn validate(&self, _: &Counter, _: &ActorId) -> Result<(), OpError> { Ok(()) }
/// }
///
/// let mut log = Vec::new();
/// encode_entry(ActorId(1), &Set(4), &mut log).unwrap();
/// encode_entry(ActorId(2), &Set(9), &mut log).unwrap();
///
/// let (state, trace) = ReplayEngine::default()
///     .replay::<Counter, Set>(Counter(0), &log)
///     .unwrap();
/// assert_eq!(state, Counter(9));
/// assert_eq!(trace.entries.len(), 2);
/// ```
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
            let (actor, op, tail) = decode_entry::<O>(rest, index)?;
            rest = tail;

            // A batch applies to a copy of the state, so a skipped entry leaves no trace in it.
            match OperationBatch::from_ops(vec![op]).apply(&mut state, actor) {
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

/// A position in a replay that can move forward and back.
///
/// The cursor decodes the whole log when it is created, and then applies one
/// entry at a time. Stepping forward keeps each entry's inverse, so stepping
/// back restores the state exactly, without replaying from the start. Use it to
/// walk an audit log and look at the state at each entry.
///
/// An entry that fails to apply is an error from [`ReplayCursor::step_forward`],
/// and the cursor stays where it was. Call [`ReplayCursor::skip_forward`] to pass
/// over it, as [`ReplayPolicy::SkipInvalid`] does.
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tpt_operations::{
///     history::{replay::encode_entry, ReplayCursor},
///     prelude::*,
/// };
///
/// #[derive(Clone, Debug, Default, PartialEq)]
/// struct Counter(i64);
/// impl State for Counter {}
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// struct Set(i64);
/// impl Operation<Counter> for Set {
///     type Inverse = Set;
///     fn apply(&self, s: &mut Counter) -> Result<(), OpError> { s.0 = self.0; Ok(()) }
///     fn inverse(&self, s: &Counter) -> Result<Set, OpError> { Ok(Set(s.0)) }
///     fn validate(&self, _: &Counter, _: &ActorId) -> Result<(), OpError> { Ok(()) }
/// }
///
/// let mut log = Vec::new();
/// for n in [10, 20, 30] {
///     encode_entry(ActorId(1), &Set(n), &mut log).unwrap();
/// }
///
/// let mut cursor = ReplayCursor::<Counter, Set>::new(Counter(0), &log).unwrap();
/// cursor.seek(2).unwrap();
/// assert_eq!(cursor.state(), &Counter(20));
/// cursor.seek(1).unwrap();
/// assert_eq!(cursor.state(), &Counter(10));
/// cursor.seek(0).unwrap();
/// assert_eq!(cursor.state(), &Counter(0));
/// ```
pub struct ReplayCursor<S, O: Operation<S>>
where
    S: State,
{
    state: S,
    entries: Vec<(ActorId, O)>,
    /// One slot per applied entry, `None` for a skipped one.
    undo: Vec<Option<O::Inverse>>,
}

impl<S, O> ReplayCursor<S, O>
where
    S: State,
    O: Operation<S> + DeserializeOwned,
{
    /// Decodes `log` and places the cursor before its first entry, on `initial`.
    ///
    /// Decode errors and unknown format versions are reported here, with the
    /// index of the entry, so a cursor never holds a log it cannot read.
    pub fn new(initial: S, log: &[u8]) -> Result<Self, ReplayError> {
        let mut entries = Vec::new();
        let mut rest = log;
        while !rest.is_empty() {
            let (actor, op, tail) = decode_entry::<O>(rest, entries.len())?;
            entries.push((actor, op));
            rest = tail;
        }
        Ok(Self {
            state: initial,
            entries,
            undo: Vec::new(),
        })
    }
}

impl<S, O> ReplayCursor<S, O>
where
    S: State,
    O: Operation<S>,
{
    /// The state after the entries before the cursor.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Number of entries before the cursor. It is 0 at the start.
    pub fn position(&self) -> usize {
        self.undo.len()
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the log has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry that [`ReplayCursor::step_forward`] would apply next.
    pub fn next_entry(&self) -> Option<(ActorId, &O)> {
        self.entries
            .get(self.position())
            .map(|(actor, op)| (*actor, op))
    }

    /// The entry that [`ReplayCursor::step_back`] would undo next.
    pub fn previous_entry(&self) -> Option<(ActorId, &O)> {
        self.position()
            .checked_sub(1)
            .and_then(|index| self.entries.get(index))
            .map(|(actor, op)| (*actor, op))
    }

    /// Applies the next entry. Returns `Ok(false)` at the end of the log.
    ///
    /// If the entry fails, the state and the position are unchanged.
    pub fn step_forward(&mut self) -> Result<bool, ReplayError> {
        let index = self.position();
        let Some((actor, op)) = self.entries.get(index) else {
            return Ok(false);
        };
        let actor = *actor;
        let result = (|| {
            op.validate(&self.state, &actor)?;
            let inverse = op.inverse(&self.state)?;
            let mut next = self.state.clone();
            op.apply(&mut next)?;
            Ok::<_, OpError>((inverse, next))
        })();
        match result {
            Ok((inverse, next)) => {
                self.state = next;
                self.undo.push(Some(inverse));
                Ok(true)
            }
            Err(error) => Err(ReplayError::Apply { index, error }),
        }
    }

    /// Passes over the next entry without applying it. Returns `false` at the end of the log.
    pub fn skip_forward(&mut self) -> bool {
        if self.position() >= self.entries.len() {
            return false;
        }
        self.undo.push(None);
        true
    }

    /// Undoes the entry before the cursor. Returns `Ok(false)` at the start.
    pub fn step_back(&mut self) -> Result<bool, OpError> {
        let Some(slot) = self.undo.last() else {
            return Ok(false);
        };
        if let Some(inverse) = slot {
            let mut previous = self.state.clone();
            inverse.apply(&mut previous)?;
            self.state = previous;
        }
        self.undo.pop();
        Ok(true)
    }

    /// Moves the cursor to `position`, which is the number of entries applied.
    ///
    /// Stops with an error at an entry that fails going forward. The cursor is
    /// left just before it. `position` past the end of the log stops at the end.
    pub fn seek(&mut self, position: usize) -> Result<(), ReplayError> {
        while self.position() > position {
            self.step_back().map_err(|error| ReplayError::Apply {
                index: self.position() - 1,
                error,
            })?;
        }
        while self.position() < position && self.step_forward()? {}
        Ok(())
    }
}
