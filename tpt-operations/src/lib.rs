//! Deterministic, reversible, CRDT-compatible operation engine.
//!
//! Every state mutation in the TPT stack is an [`core::Operation`]. Each one
//! can be validated, applied, and inverted (for undo). The inverse is built
//! from a snapshot of the pre-application state, never by arithmetic on the
//! operation's inputs. This avoids "Inverse Drift": floating-point error or
//! lost metadata making undo leave the state different from where it started.
//!
//! The crate is `no_std` and uses `alloc`. The `std` feature is optional.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Lets `::tpt_operations::...` paths emitted by the derive resolve inside this crate too.
extern crate self as tpt_operations;

#[cfg(feature = "derive")]
pub use tpt_operations_derive::Operation;

pub mod compose;
pub mod core;
pub mod crdt;
#[cfg(feature = "crypto")]
pub mod crypto;
pub mod history;

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod testkit;
