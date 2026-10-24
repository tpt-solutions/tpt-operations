# Contributing

## Set up

The toolchain is pinned in `rust-toolchain.toml`. Everything below runs from the
repository root:

```sh
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo build -p tpt-operations --no-default-features --target thumbv7em-none-eabihf
cargo deny check
```

The property tests run 100,000 cases per property, so a full test run takes a
little while.

## Rules

- **Inverse Drift.** `Operation::inverse` captures old values from the
  pre-application state. It never computes them back from the operation's
  inputs. See `examples/delta_op.rs`.
- **No `unsafe`, no C-FFI.** `#![forbid(unsafe_code)]` is on, and `cargo deny`
  bans native-code crates. If a new `-sys` crate shows up in the tree, add it to
  the list in `deny.toml`.
- **`no_std` + `alloc`.** The core must keep building for bare-metal targets.
  Use `::core` and `alloc` paths, and put `std` behind the `std` feature.
- **Failures leave state alone.** A failing operation, batch, `commit` or
  `receive` must not change the caller's state.

## Changes

- Test new operations with the `testkit` feature (`check_reversible` and the
  others). `tpt-operations/tests/testkit.rs` shows how.
- Add a test with every change. For a bug fix, write the test first and check
  that it fails.
- Add an entry under Unreleased in `CHANGELOG.md` and in the crate's own
  `CHANGELOG.md`.
- Tick or add the matching item in `TODO.md`.
- Examples in `tpt-operations/examples/` are run by CI. If you add one, add it to
  the list in `.github/workflows/ci.yml` and to the table in the crate README.

## Licence

By contributing you agree that your work is licensed under `MIT OR Apache-2.0`,
the same as the project.
