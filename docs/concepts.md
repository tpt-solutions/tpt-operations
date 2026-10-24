# Concepts

The ideas in `tpt-operations`, in the order you meet them. Each section ends with
where to see it run.

## State and operation

A **state** is plain data that implements `State` (it must be `Clone`). An
**operation** changes a state, and implements `Operation<S>` with three methods:

| Method | Job |
| --- | --- |
| `validate(&state, &actor)` | Say whether `actor` may do this now. Changes nothing. |
| `inverse(&state)` | Build the operation that undoes this one. Called **before** `apply`, with the state as it is now. |
| `apply(&mut state)` | Make the change. |

The inverse is a snapshot. For "set `x` to 5" it is "set `x` to whatever `x` is
now". See [why snapshot inverses](why-snapshot-inverses.md).

Run it: `cargo run -p tpt-operations --example counter --features derive`.

## Batches: all or nothing

An `OperationBatch` is a list of operations that apply together. If any
operation fails, the batch returns the error and the state is exactly as it was,
even when the failing operation had already changed part of it. A successful
batch returns its own undo batch: the inverses, in reverse order.

The batch works on a copy of the state, so one batch costs one clone of the
state. Keep that in mind for very large states.

Run it: `cargo run -p tpt-operations --example text_doc_undo`.

## History: undo and redo

`HistoryStack` runs batches and remembers how to undo them. A batch is one undo
step, however many operations it holds. Running a new batch clears redo. The
stack is capped by step count (default 10,000) and by estimated memory (default
50 MB), and drops the oldest steps first. The memory estimate comes from
`SizeEstimate`, which you implement or derive.

## Replay: rebuild a state from a log

An operation log is a byte string of versioned entries, each holding the actor
and the operation (postcard encoded). `ReplayEngine` applies them in order to an
initial state and returns the final state and a trace, one line per entry for an
auditor.

- `FailFast` stops at the first entry that fails to apply.
- `SkipInvalid` records the failure in the trace and carries on. The failed
  entry leaves the state untouched.
- A log that cannot be decoded always stops the replay.

Replay is deterministic: the same log and policy give the same state and trace.

Run it: `cargo run -p tpt-operations --example event_sourcing_replay --features serde`.

## CRDT markers

`CrdtCompatible<S>` is a promise about an operation type:

- **commutative**: `a` then `b` equals `b` then `a`,
- **associative**: grouping does not change the result,
- **idempotent**: applying twice equals applying once.

Replicas that apply the same commutative, idempotent operations in any order,
with duplicates, end in the same state. Declare a marker only if it is true for
every value of the type, and test the claim with the `testkit` feature.
`LamportClock` and `OpTimestamp` give operations a total order for the
"last writer wins" style.

Run it: `cargo run -p tpt-operations --example cell_crdt`.

## Signed entries and the pipeline

With the `crypto` feature, `pipeline::commit` validates an operation, takes its
inverse, signs it, applies it, and returns the signed entry to send plus the
inverse to keep. On the other side `pipeline::receive` checks the signature,
decodes, validates and applies.

Three checks protect a receiver, and the example shows each one refused:

1. **Signature.** The signature covers a domain tag, the digest version, the
   sequence number and the entry bytes.
2. **Author.** The signature proves a key signed the entry. `receive` is told
   which actor that key belongs to and refuses an entry claiming another actor.
   You keep the key-to-actor registry.
3. **Replay.** Each author numbers entries with a rising `seq`. A `ReplayGuard`
   remembers the highest accepted one per author and refuses the rest.

Run it: `cargo run -p tpt-operations --example signed_audit_log --features crypto`.

## What the crate leaves to you

- Transport and ordering across authors. Hand the `SignedEntry` to your
  transport (for the TPT stack, `tpt-replicate`) and deliver each author's
  entries in order.
- Storage of logs and snapshots.
- The key-to-actor registry.
- Capability checks. `Capability` is a stub until a real caller needs it.

More on the sibling crates: [integration points](integration.md).
