use ::core::fmt;
use alloc::string::String;

/// Actor identifier. With the `primitives` feature, [`crate::primitives`] maps
/// it to and from a `tpt-primitives` principal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActorId(pub u64);

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "actor:{}", self.0)
    }
}

impl From<u64> for ActorId {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

impl From<ActorId> for u64 {
    fn from(actor: ActorId) -> Self {
        actor.0
    }
}

/// Stub capability token. Nothing checks it yet: `Operation::validate` takes
/// no capability. The `tpt-primitives` capability model needs epoch and
/// logical-time context that `validate` does not have, so this waits for a
/// real caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Capability {
    /// Name of the permission this token grants, e.g. `"write"`.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn actor_id_converts_displays_and_defaults() {
        assert_eq!(ActorId::from(7), ActorId(7));
        assert_eq!(u64::from(ActorId(7)), 7);
        assert_eq!(ActorId(7).to_string(), "actor:7");
        assert_eq!(ActorId::default(), ActorId(0));
    }
}
