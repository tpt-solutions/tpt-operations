//! The names most programs need, in one import:
//!
//! ```
//! use tpt_operations::prelude::*;
//! ```
//!
//! With the `derive` feature this also brings in `#[derive(Operation)]` and
//! `#[derive(SizeEstimate)]`.

pub use crate::{
    compose::OperationBatch,
    core::{ActorId, OpError, Operation, State},
    history::{HistoryStack, SizeEstimate},
    tpt_batch,
};

#[cfg(feature = "derive")]
pub use tpt_operations_derive::{Operation, SizeEstimate};
