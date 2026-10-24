# tpt-operations

Deterministic, reversible, CRDT-compatible operation engine for the TPT stack.
Every state mutation is an `Operation` that can be validated, applied, and
inverted for undo. The inverse is built from a snapshot of the state before the
operation runs, so undo restores the exact prior state.

The crate is `no_std` and uses `alloc`. It forbids `unsafe` code.

## Install

Nothing is published to crates.io yet, so depend on it by git or by path:

```toml
[dependencies]
tpt-operations = { git = "https://github.com/tpt-solutions/tpt-operations", features = ["derive"] }
```

The `derive` feature is **off by default**. Without it, `#[derive(Operation)]`
is not available and you write the `Operation` impl by hand. Both ways are
shown in the examples below.

## Example

With the `derive` feature, `#[derive(Operation)]` writes the operation for you:

```rust
use tpt_operations::core::{Operation as _, OpError, State};
use tpt_operations::Operation;

#[derive(Debug, Clone, PartialEq)]
struct Counter {
    value: i64,
}

impl State for Counter {}

#[derive(Clone, Operation)]
#[tpt(state = Counter)]
struct SetValue {
    value: i64,
}

fn main() -> Result<(), OpError> {
    let mut state = Counter { value: 0 };
    let op = SetValue { value: 5 };

    // Capture the inverse before applying, while the state is still the snapshot.
    let undo = op.inverse(&state)?;
    state.apply(&op)?;
    assert_eq!(state.value, 5);

    state.apply(&undo)?;
    assert_eq!(state.value, 0);
    Ok(())
}
```

For the common imports in one line, use `use tpt_operations::prelude::*;`.

## Examples

Each one is a runnable program that prints what it does. Run it with
`cargo run -p tpt-operations --example <name>`, adding the feature in the
table when there is one.

| Example | Feature | Shows |
| --- | --- | --- |
| `counter` | `derive` | The smallest program: a derived op, undo and redo. |
| `text_doc_undo` | none | A text document where "replace all" is one undo step, and a bad batch changes nothing. |
| `cell_crdt` | none | Last-writer-wins cells with Lamport stamps. Every delivery order converges. |
| `cad_vertex` | `derive` | Float positions that undo to the exact bits. |
| `delta_op` | none | The Inverse Drift rule, by breaking it. Write delta ops by hand like this. |
| `signed_audit_log` | `crypto` | Commit and receive with Ed25519. Tampering, impersonation and replay are refused. |
| `event_sourcing_replay` | `serde` | Write an op log, replay it, and read the audit trace. |

## Features

Pick features by what you need:

| I want to... | Enable |
| --- | --- |
| Use `#[derive(Operation)]` and `#[derive(SizeEstimate)]` | `derive` |
| Replay a serialized op log | `serde` |
| Sign and verify entries, and use `pipeline::commit` / `receive` | `crypto` |
| Map `ActorId` to a `tpt-primitives` principal | `primitives` |
| Convert operations to `tpt-uir` nodes | `uir` |
| Test my operations with property checks and shrinking | `testkit` |
| Get `std::error::Error` for `OpError` | `std` |

Details:

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | no | Enables `std::error::Error` for `OpError`. |
| `derive` | no | Re-exports `#[derive(Operation)]` and `#[derive(SizeEstimate)]` from `tpt-operations-derive`. |
| `testkit` | no | The `testkit` module: `check_reversible`, `check_commutative`, `check_idempotent`, `check_replay_deterministic` (with `serde`), and `assert_property` with a seeded RNG and shrinking. |
| `serde` | no | Serialized, versioned op log (serde + postcard) for `ReplayEngine`. |
| `crypto` | no | `tpt-crypto` adapter: SHA-256 digests and Ed25519 signatures. Enables `SignedEntry` and the `pipeline` module (`commit` / `receive`). |
| `primitives` | no | `tpt-primitives` adapter: maps `ActorId` to and from `Principal`. Needs a target with `std`: `tpt-primitives` does not build for bare-metal targets. |
| `uir` | no | `tpt-uir` node conversion (`to_node` / `from_node`). This is a stub: digests cover postcard bytes, not canonical `tpt-uir` bytes. |

## Testing your own operations

Enable the `testkit` feature and check that your operations keep their promises.
The checks return a `Failure` that shows the states involved, and
`assert_property` shrinks a failing input to a smaller one:

```rust,ignore
use tpt_operations::testkit::{assert_property, check_reversible, shrink_none};

assert_property(
    "my ops are reversible",
    0xFEED,                                 // fixed seed: failures reproduce
    10_000,                                 // cases
    |rng| (rng.range_i64(-1_000, 1_000), random_op(rng)),
    shrink_none,                            // or shrink_i64, shrink_vec
    |(value, op)| check_reversible(&Counter { value: *value }, op),
);
```

`tests/testkit.rs` has a complete version, including a check that catches an
arithmetic inverse and shrinks the failing input.

## Rules

- **Inverse Drift.** `Operation::inverse` is called with the pre-application
  state and must capture the old values from that snapshot. It must not compute
  them back from the operation's inputs. Arithmetic inverses such as `Add(-n)`
  can leave a different value behind.
- **Merging is opt-in.** `State` has no merge method. CRDT-style states implement
  `MergeableState`, and operations declare `CrdtCompatible` markers with
  `#[tpt(commutative | associative | idempotent)]`.

## Status

Nothing is published yet. Some optional dependencies are pinned to git
revisions of the sibling `tpt-*` crates, so this crate cannot be published to
crates.io until those are released.

See [`docs/integration.md`](https://github.com/tpt-solutions/tpt-operations/blob/master/docs/integration.md)
for how these integration points connect to the sibling crates, and
[`CHANGELOG.md`](CHANGELOG.md) for the release history.

## Development

```sh
cargo test -p tpt-operations --all-features
cargo build -p tpt-operations --examples --all-features
cargo build -p tpt-operations --no-default-features --target thumbv7em-none-eabihf
```

The minimum supported Rust version is 1.97. The pinned toolchain is in the
workspace's `rust-toolchain.toml`.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
at your option.
