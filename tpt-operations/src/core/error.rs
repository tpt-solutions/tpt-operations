use ::core::fmt;
use alloc::string::String;

/// Reasons an operation can fail. Each variant carries a human-readable message.
///
/// Marked `#[non_exhaustive]` so that new failure kinds, such as capability
/// checks, can be added without breaking callers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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

impl OpError {
    /// Puts `context` in front of the message and keeps the variant.
    ///
    /// Use it where an error crosses a boundary, so the reader knows where it
    /// came from: `err.context("entry 3 from actor:2")` reads
    /// `bounds error: entry 3 from actor:2: counter overflow`. Code that matches
    /// on the variant is not affected.
    #[must_use]
    pub fn context(self, context: impl fmt::Display) -> Self {
        let wrap = |message: String| alloc::format!("{context}: {message}");
        match self {
            OpError::Validation(m) => OpError::Validation(wrap(m)),
            OpError::Bounds(m) => OpError::Bounds(wrap(m)),
            OpError::Capability(m) => OpError::Capability(wrap(m)),
            OpError::NotFound(m) => OpError::NotFound(wrap(m)),
            OpError::Conflict(m) => OpError::Conflict(wrap(m)),
            OpError::Serialization(m) => OpError::Serialization(wrap(m)),
        }
    }
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

#[cfg(feature = "serde")]
impl From<postcard::Error> for OpError {
    fn from(error: postcard::Error) -> Self {
        OpError::Serialization(alloc::format!("{error}"))
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OpError {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn context_prefixes_the_message_and_keeps_the_variant() {
        let error = OpError::Bounds("counter overflow".into()).context("entry 3");
        assert_eq!(error, OpError::Bounds("entry 3: counter overflow".into()));
        assert_eq!(error.to_string(), "bounds error: entry 3: counter overflow");
    }

    #[test]
    fn context_nests_from_the_inside_out() {
        let error = OpError::NotFound("row 4".into())
            .context("table users")
            .context("import");
        assert_eq!(
            error,
            OpError::NotFound("import: table users: row 4".into())
        );
    }

    #[test]
    fn context_works_on_every_variant() {
        for error in [
            OpError::Validation("m".into()),
            OpError::Bounds("m".into()),
            OpError::Capability("m".into()),
            OpError::NotFound("m".into()),
            OpError::Conflict("m".into()),
            OpError::Serialization("m".into()),
        ] {
            assert!(!error.to_string().ends_with("c: m"));
            assert!(error.context("c").to_string().ends_with("c: m"));
        }
    }
}
