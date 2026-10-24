use alloc::string::String;

/// Actor identifier. With the `primitives` feature, [`crate::primitives`] maps
/// it to and from a `tpt-primitives` principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActorId(pub u64);

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
