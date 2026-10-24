# tpt-operations

Deterministic, reversible, CRDT-compatible operation engine for the TPT stack
(Layer 2). Every state mutation is an `Operation` that can be validated,
applied, and inverted for undo. See [`spec.txt`](spec.txt) for the design and
[`TODO.md`](TODO.md) for the implementation checklist.

## Layout

| Crate | Purpose |
| --- | --- |
| `tpt-operations` | Core library (`no_std` + `alloc`). |
| `tpt-operations-derive` | Proc-macro crate for `#[derive(Operation)]`. |

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | no | Enables `std::error::Error` for `OpError`. |
| `derive` | no | Re-exports the derive macro. |
| `uir`, `crypto`, `primitives` | no | Adapter slots for `tpt-uir`, `tpt-crypto`, `tpt-primitives`. Stubbed until those crates are wired in. |

## Rules

- Pure Rust. `#![forbid(unsafe_code)]`, no C-FFI dependencies (checked by `cargo deny`).
- Inverses are built from the **pre-application snapshot**, never by arithmetic on
  the operation's inputs. See the Inverse Drift rule in the `Operation` docs.

## Development

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p tpt-operations --no-default-features --target thumbv7em-none-eabihf
```

The pinned toolchain is in `rust-toolchain.toml`.
