use ::core::fmt;
use alloc::string::String;

/// Reasons an operation can fail. Each variant carries a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    /// The operation's inputs are invalid for the current state.
    Validation(String),
    /// A value fell outside its allowed range (overflow, index, coordinate).
    Bounds(String),
    /// The acting identity does not hold the capability this operation needs.
    Capability(String),
    /// A referenced entity does not exist.
    NotFound(String),
    /// The operation conflicts with the current state.
    Conflict(String),
    /// Encoding or decoding of an operation failed.
    Serialization(String),
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, msg) = match self {
            OpError::Validation(m) => ("validation", m),
            OpError::Bounds(m) => ("bounds", m),
            OpError::Capability(m) => ("capability", m),
            OpError::NotFound(m) => ("not found", m),
            OpError::Conflict(m) => ("conflict", m),
            OpError::Serialization(m) => ("serialization", m),
        };
        write!(f, "{kind} error: {msg}")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OpError {}
