# Migrating from hand-rolled undo

If your app already has an undo stack, this is how to move it onto
`tpt-operations` a piece at a time. Each step leaves the app working.

## What you probably have

Something like a `Vec<Command>` where each command has `do()` and `undo()`, or a
stack of full state copies. The usual problems are:

- `undo()` is written separately from `do()`, and the two disagree in corner
  cases.
- A command that fails halfway leaves the document half changed.
- Grouping several edits into one undo step is done by hand, if at all.
- There is no log you could replay or send to another machine.

## Step 1: put the data in a `State`

Your document, or the part commands change, becomes a type that implements
`State`:

```rust,ignore
#[derive(Clone)]
struct Document { /* the fields commands change */ }
impl State for Document {}
```

`State` only needs `Clone`. If the document is large, wrap the big parts in
`Arc` or a persistent collection, so the clone a batch makes is cheap.

## Step 2: turn each command into an operation

For each command, implement `Operation<Document>`:

| Your command | In `tpt-operations` |
| --- | --- |
| `do()` | `apply` |
| `undo()` | `inverse`, which returns the operation that does the undo |
| "can this run now?" | `validate` |

The only real change is how undo is written. Instead of code that reverses the
edit, `inverse` returns an operation that restores what was there, read from the
state. [Why](why-snapshot-inverses.md). The `text_doc_undo` example shows a
range replace done this way: insert, delete and replace are one operation.

If a command only sets fields, `#[derive(Operation)]` writes all three methods.

## Step 3: run commands through batches and a history

Replace your stack with `HistoryStack`:

```rust,ignore
history.execute(&mut doc, &tpt_batch![op_a, op_b], actor)?;   // one undo step
history.undo(&mut doc, actor)?;
history.redo(&mut doc, actor)?;
```

A batch is atomic, so a command that fails halfway no longer leaves the document
changed. Several operations in one batch are one undo step. Set the caps with
`HistoryStack::with_limits`, and give each operation a `SizeEstimate`.

To enable or disable undo buttons, use `can_undo`, `can_redo`, and
`next_undo` / `next_redo` to see what the next step will do.

## Step 4: test it

Add the `testkit` feature as a dev dependency and check every operation:

```rust,ignore
assert_property("edit ops are reversible", 0xC0DE, 10_000,
    |rng| (random_document(rng), random_op(rng)),
    shrink_none,
    |(doc, op)| check_reversible(doc, op));
```

This catches the old disagreements between `do()` and `undo()`, and shows the
states when it does.

## Step 5 (optional): log, replay and sign

Once commands are operations, you get more for little work:

- **Audit or event sourcing.** Write each operation with
  `history::replay::encode_entry`, and rebuild the document with `ReplayEngine`.
  The trace lists what happened to every entry.
- **Collaboration.** Mark operations that commute with `CrdtCompatible`, give
  them Lamport stamps, and send them over a transport. See the `cell_crdt`
  example.
- **Non-repudiation.** With the `crypto` feature, `pipeline::commit` signs each
  operation and `pipeline::receive` verifies it, refuses impersonation and
  refuses replays. See the `signed_audit_log` example.

## Things that catch people out

- Call `inverse` **before** `apply`. The batch and history types do this for you.
  If you call the operation by hand, take the inverse first.
- `State::apply` can leave a state half changed when it fails. Run operations
  through a batch or `pipeline` to get the all-or-nothing behaviour.
- `ActorId` is passed to `validate` so that operations can check permissions.
  The derive accepts everyone, so use `#[tpt(validate = path)]` or write the
  impl if you need a check.
