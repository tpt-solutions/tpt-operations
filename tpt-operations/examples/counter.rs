//! The smallest useful program: one derived operation, undo and redo.
//!
//! Run it with `cargo run -p tpt-operations --example counter --features derive`.
//!
//! `#[derive(Operation)]` maps each field of `SetValue` to the state field of
//! the same name. The inverse is a snapshot of the old value, so undo is exact.

use tpt_operations::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Counter {
    value: i64,
}

impl State for Counter {}

#[derive(Debug, Clone, Operation, SizeEstimate)]
#[tpt(state = Counter)]
struct SetValue {
    value: i64,
}

fn main() -> Result<(), OpError> {
    let me = ActorId(1);
    let mut state = Counter { value: 0 };
    let mut history = HistoryStack::new();

    for value in [5, 8, 13] {
        history.execute(&mut state, &tpt_batch![SetValue { value }], &me)?;
        println!("set    -> {}", state.value);
    }

    while history.undo(&mut state, &me)? {
        println!("undo   -> {}", state.value);
    }

    while history.redo(&mut state, &me)? {
        println!("redo   -> {}", state.value);
    }

    assert_eq!(state.value, 13);
    Ok(())
}
