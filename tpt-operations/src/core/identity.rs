use alloc::string::String;

/// Stub actor identifier. Replaced by `tpt-primitives::ActorId` when the
/// `primitives` adapter is wired in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActorId(pub u64);

/// Stub capability token. Replaced by `tpt-primitives::Capability` when the
/// `primitives` adapter is wired in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Capability {
    /// Name of the permission this token grants, e.g. `"write"`.
    pub name: String,
}
