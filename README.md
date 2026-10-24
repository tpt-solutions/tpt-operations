# tpt-operations

[![CI](https://github.com/tpt-solutions/tpt-operations/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-operations/actions/workflows/ci.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.97-blue)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

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
| `derive` | no | Re-exports `#[derive(Operation)]` and `#[derive(SizeEstimate)]`. |
| `testkit` | no | Property checks and a shrinking runner for testing your own operations. |
| `serde` | no | Serialized, versioned op log (serde + postcard) for `ReplayEngine`. |
| `crypto` | no | Real `tpt-crypto` adapter: SHA-256 digests and Ed25519 signatures. Enables `SignedEntry` and the `pipeline` module (`commit` / `receive`). |
| `primitives` | no | Real `tpt-primitives` adapter: maps `ActorId` to and from `Principal`. Needs a target with `std`: `tpt-primitives` does not build for bare-metal targets. |
| `uir` | no | `tpt-uir` node conversion (`to_node` / `from_node`). Still a stub: digests cover postcard bytes, not canonical `tpt-uir` bytes. |

Runnable examples are in [`tpt-operations/examples/`](tpt-operations/examples),
listed in the [crate README](tpt-operations/README.md#examples).

See [`docs/integration.md`](docs/integration.md) for how these integration
points connect to the sibling crates.

## Rules

- Pure Rust. `#![forbid(unsafe_code)]`, no C-FFI dependencies (checked by `cargo deny`).
- Inverses are built from the **pre-application snapshot**, never by arithmetic on
  the operation's inputs. See the Inverse Drift rule in the `Operation` docs.

## Contributing and security

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the rules and the checks to run, and
[`SECURITY.md`](SECURITY.md) for how to report a vulnerability.

## Development

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p tpt-operations --no-default-features --target thumbv7em-none-eabihf
```

The pinned toolchain is in `rust-toolchain.toml`.
