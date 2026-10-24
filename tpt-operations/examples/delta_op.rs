//! The Inverse Drift rule, shown by breaking it.
//!
//! Run it with `cargo run -p tpt-operations --example delta_op`.
//!
//! A "move by dx" operation looks like it should undo with "move by -dx".
//! With floating point it does not: the move can lose low bits, and moving back
//! does not bring them back. `NaiveMove` does it that way, and the state ends
//! up different. `Move` is the correct hand-written version: its inverse is a
//! `SetX` that holds the old value, read from the pre-state.
//!
//! The derive macro only covers "set these fields" operations. A delta like
//! `Move` is written by hand, and this is the shape to follow.

use tpt_operations::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: f32,
}

impl State for Point {}

/// Sets `x` to a value. This is the inverse of `Move`.
#[derive(Debug, Clone)]
struct SetX(f32);

impl Operation<Point> for SetX {
    type Inverse = SetX;

    fn apply(&self, state: &mut Point) -> Result<(), OpError> {
        state.x = self.0;
        Ok(())
    }

    fn inverse(&self, state: &Point) -> Result<SetX, OpError> {
        Ok(SetX(state.x))
    }

    fn validate(&self, _state: &Point, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

/// Moves `x` by a delta. Correct: the inverse is a snapshot.
#[derive(Debug, Clone)]
struct Move(f32);

impl Operation<Point> for Move {
    type Inverse = SetX;

    fn apply(&self, state: &mut Point) -> Result<(), OpError> {
        state.x += self.0;
        Ok(())
    }

    fn inverse(&self, state: &Point) -> Result<SetX, OpError> {
        Ok(SetX(state.x)) // the old value, from the pre-state
    }

    fn validate(&self, _state: &Point, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

/// Moves `x` by a delta. WRONG: the inverse is computed from the input.
#[derive(Debug, Clone)]
struct NaiveMove(f32);

impl Operation<Point> for NaiveMove {
    type Inverse = NaiveMove;

    fn apply(&self, state: &mut Point) -> Result<(), OpError> {
        state.x += self.0;
        Ok(())
    }

    fn inverse(&self, _state: &Point) -> Result<NaiveMove, OpError> {
        Ok(NaiveMove(-self.0)) // drifts
    }

    fn validate(&self, _state: &Point, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

fn main() -> Result<(), OpError> {
    let start = Point { x: 0.1 };
    let delta = 1.0e8;

    let mut naive = start.clone();
    let naive_op = NaiveMove(delta);
    let naive_undo = naive_op.inverse(&naive)?;
    naive.apply(&naive_op)?;
    naive.apply(&naive_undo)?;
    println!(
        "start:             {:?} (bits {:#010x})",
        start.x,
        start.x.to_bits()
    );
    println!(
        "naive undo (-dx):  {:?} (bits {:#010x})",
        naive.x,
        naive.x.to_bits()
    );

    let mut exact = start.clone();
    let op = Move(delta);
    let undo = op.inverse(&exact)?;
    exact.apply(&op)?;
    exact.apply(&undo)?;
    println!(
        "snapshot undo:     {:?} (bits {:#010x})",
        exact.x,
        exact.x.to_bits()
    );

    assert_ne!(naive, start, "the naive inverse lost the original value");
    assert_eq!(exact, start, "the snapshot inverse restores it exactly");
    Ok(())
}
