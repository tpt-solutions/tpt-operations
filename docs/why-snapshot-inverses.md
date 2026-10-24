# Why inverses are snapshots

The rule: `Operation::inverse` reads the old values from the state it is given.
It never works them out from the operation's own inputs. We call breaking the
rule **Inverse Drift**.

## The problem

The obvious inverse of "move by `dx`" is "move by `-dx`". With exact numbers
that works. With floating point it does not:

```text
x = 0.1            (as f32)
x + 1.0e8  ->  1.0e8      the 0.1 is too small to survive
x - 1.0e8  ->  0.0        not 0.1
```

The undo ran, and the state is still wrong. It is wrong by a tiny amount, which
is the worst kind: nothing fails, and the error compounds with every undo and
redo. In CAD that is a vertex that creeps. In a spreadsheet it is a total that
no longer matches its cells.

Integers can break too. A field that saturates or clamps loses information, so
`Add(-n)` does not undo `Add(n)` near the limit. And undoing a delete needs the
deleted content, which only the state has.

## The fix

Capture the old value before the change:

```rust,ignore
fn inverse(&self, state: &Point) -> Result<SetX, OpError> {
    Ok(SetX(state.x))   // the value that is about to be overwritten
}
```

Applying the op and then this inverse writes back the exact bits that were
there. There is no arithmetic to drift.

This is why the order is fixed: call `inverse` on the **pre-state**, then
`apply`. `OperationBatch`, `HistoryStack` and `pipeline::commit` do it in that
order for you.

## What it costs

An inverse carries the old value, so it can be as big as the data it protects.
Undoing the deletion of a 10 KB paragraph stores 10 KB. `HistoryStack` counts
that through `SizeEstimate` and drops the oldest steps when the memory cap is
reached, so history stays bounded.

## What the derive does

`#[derive(Operation)]` follows the rule for you. Each field maps to a state field
of the same name, `apply` writes it, and `inverse` copies the current state
values into a new operation of the same type. It cannot produce an arithmetic
inverse, and `#[tpt(inverse = "...")]` is a compile error for that reason.

Operations that are not "set these fields", such as deltas, inserts into a list,
or edits to a text range, are written by hand. Keep the same shape: read the old
value from the state.

## See it break

`cargo run -p tpt-operations --example delta_op` runs the same move twice, once
with `-dx` and once with a snapshot, and prints the bits of the result.

You can also catch it in your own tests. With the `testkit` feature,
`check_reversible` applies an operation and its inverse and reports the states
if they differ, and `assert_property` shrinks the failing input.
