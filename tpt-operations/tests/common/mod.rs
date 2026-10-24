//! Shared test types: a counter state and its operations.
//!
//! The operations derive serde, so they can be converted to `uir` nodes.

use serde::{Deserialize, Serialize};
use tpt_operations::{
    core::{ActorId, OpError, Operation, State},
    crdt::CrdtCompatible,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counter {
    pub value: i64,
}

impl State for Counter {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterOp {
    Add(i64),
    Set(i64),
}

impl Operation<Counter> for CounterOp {
    type Inverse = CounterOp;

    fn apply(&self, state: &mut Counter) -> Result<(), OpError> {
        match self {
            CounterOp::Add(n) => {
                state.value = state
                    .value
                    .checked_add(*n)
                    .ok_or_else(|| OpError::Bounds("counter overflow".into()))?;
            }
            CounterOp::Set(v) => state.value = *v,
        }
        Ok(())
    }

    fn inverse(&self, state: &Counter) -> Result<CounterOp, OpError> {
        Ok(CounterOp::Set(state.value))
    }

    fn validate(&self, _state: &Counter, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

impl CrdtCompatible<Counter> for CounterOp {
    // Add commutes, Set does not. Add is not idempotent. Sequential application
    // groups freely, so the associativity marker is true.
    const IS_COMMUTATIVE: bool = false;
    const IS_ASSOCIATIVE: bool = true;
    const IS_IDEMPOTENT: bool = false;
}
