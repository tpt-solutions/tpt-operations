## What and why

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-features --all-targets -- -D warnings` are clean
- [ ] `cargo test --workspace --all-features` passes
- [ ] New behaviour has a test, and a bug fix has a test that failed before the fix
- [ ] Inverses are snapshots of the pre-state, not arithmetic on the op's inputs
- [ ] `CHANGELOG.md` (root and crate) has an entry under Unreleased
