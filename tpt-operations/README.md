# tpt-operations

Deterministic, reversible, CRDT-compatible operation engine for the TPT stack.
Every state mutation is an `Operation` that can be validated, applied, and
inverted for undo. The inverse is built from a snapshot of the state before the
operation runs, so undo restores the exact prior state.

The crate is `no_std` and uses `alloc`. It forbids `unsafe` code.

## Example

Enable the `derive` feature to get `#[derive(Operation)]`:

```toml
[dependencies]
tpt-operations = { version = "0.1", features = ["derive"] }
```

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

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | no | Enables `std::error::Error` for `OpError`. |
| `derive` | no | Re-exports the `#[derive(Operation)]` macro from `tpt-operations-derive`. |
| `serde` | no | Serialized, versioned op log (serde + postcard) for `ReplayEngine`. |
| `crypto` | no | `tpt-crypto` adapter: SHA-256 digests and Ed25519 signatures. Enables `SignedEntry` and the `pipeline` module (`commit` / `receive`). |
| `primitives` | no | `tpt-primitives` adapter: maps `ActorId` to and from `Principal`. |
| `uir` | no | `tpt-uir` node conversion (`to_node` / `from_node`). This is a stub: digests cover postcard bytes, not canonical `tpt-uir` bytes. |

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
cargo build -p tpt-operations --no-default-features --target thumbv7em-none-eabihf
```

The minimum supported Rust version is 1.97. The pinned toolchain is in the
workspace's `rust-toolchain.toml`.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE)
at your option.
