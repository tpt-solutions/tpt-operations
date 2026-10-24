//! Fundamental traits: [`State`] and [`Operation`], plus the shared error and
//! identity types they use.

mod error;
mod identity;
mod traits;

pub use error::OpError;
pub use identity::{ActorId, Capability};
pub use traits::{MergeableState, Operation, State};

#[cfg(test)]
pub(crate) mod toy;
