//! Deterministic, reversible, CRDT-compatible operation engine.
//!
//! Every state mutation in the TPT stack is an [`core::Operation`]. Each one
//! can be validated, applied, and inverted (for undo). The inverse is built
//! from a snapshot of the pre-application state, never by arithmetic on the
//! operation's inputs. This avoids "Inverse Drift": floating-point error or
//! lost metadata making undo leave the state different from where it started.
//!
//! The crate is `no_std` and uses `alloc`. The `std` feature is optional.
//!
//! # Quick start
//!
//! A state, one operation, and undo/redo. The inverse is a snapshot of the
//! value the operation is about to overwrite:
//!
//! ```
//! use tpt_operations::prelude::*;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Counter { value: i64 }
//! impl State for Counter {}
//!
//! #[derive(Clone, Debug)]
//! struct SetValue(i64);
//!
//! impl Operation<Counter> for SetValue {
//!     type Inverse = SetValue;
//!
//!     fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
//!         state.value = self.0;
//!         Ok(())
//!     }
//!
//!     // Capture the old value from the pre-state. Never compute it back from `self`.
//!     fn inverse(&self, state: &Counter) -> Result<SetValue, OpError> {
//!         Ok(SetValue(state.value))
//!     }
//!
//!     fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
//!         Ok(())
//!     }
//! }
//!
//! impl SizeEstimate for SetValue {
//!     fn estimated_size(&self) -> usize { core::mem::size_of::<Self>() }
//! }
//!
//! let actor = ActorId(1);
//! let mut state = Counter { value: 0 };
//! let mut history = HistoryStack::new();
//!
//! history.execute(&mut state, &tpt_batch![SetValue(5)], &actor)?;
//! assert_eq!(state.value, 5);
//!
//! history.undo(&mut state, &actor)?;
//! assert_eq!(state.value, 0);
//!
//! history.redo(&mut state, &actor)?;
//! assert_eq!(state.value, 5);
//! # Ok::<(), OpError>(())
//! ```
//!
//! With the `derive` feature, `#[derive(Operation)]` writes the operation for
//! you when each field maps to a field of the state. See the README.
//!
//! # Features
//!
//! | I want to... | Enable |
//! | --- | --- |
//! | Use `#[derive(Operation)]` and `#[derive(SizeEstimate)]` | `derive` |
//! | Replay a serialized op log | `serde` |
//! | Sign and verify entries, and use `pipeline::commit` / `receive` | `crypto` |
//! | Map `ActorId` to a `tpt-primitives` principal | `primitives` |
//! | Convert operations to `tpt-uir` nodes | `uir` |
//! | Get `std::error::Error` for `OpError` | `std` |

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Lets `::tpt_operations::...` paths emitted by the derive resolve inside this crate too.
extern crate self as tpt_operations;

#[cfg(feature = "derive")]
pub use tpt_operations_derive::{Operation, SizeEstimate};

pub mod compose;
pub mod core;
pub mod crdt;
#[cfg(feature = "crypto")]
pub mod crypto;
pub mod history;
#[cfg(feature = "crypto")]
pub mod pipeline;
pub mod prelude;
#[cfg(feature = "primitives")]
pub mod primitives;
#[cfg(feature = "uir")]
pub mod uir;

// Compiles the README's example as a doc-test. It needs the derive macro.
#[cfg(all(doctest, feature = "derive"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod testkit;
