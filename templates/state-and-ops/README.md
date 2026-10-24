# state-and-ops

A `cargo-generate` template for a program built on `tpt-operations`: a state, an
operation, undo and redo, and a property test that keeps undo honest.

```sh
cargo install cargo-generate
cargo generate --git https://github.com/tpt-solutions/tpt-operations templates/state-and-ops --name my-app
cd my-app
cargo run
cargo test
```

It generates a small notes list. Replace `Notes` with your state and `Edit` with
your operations. The comments in `src/main.rs` say where.

The generated `Cargo.toml` depends on `tpt-operations` by git, because it is not
on crates.io yet. Pin `rev` to a commit you have checked.
