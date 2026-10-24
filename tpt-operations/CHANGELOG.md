# Changelog

All notable changes to this crate are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Nothing is published yet, so there are no released versions.

## [Unreleased]

### Added
- `State`, `Operation<S>` and `OpError` core traits. Inverses are built from a
  pre-application snapshot, never by arithmetic on the op's inputs.
- `#[derive(Operation)]` re-export behind the `derive` feature.
- `OperationBatch` with atomic apply and rollback, and the `tpt_batch!` macro.
- `HistoryStack` for undo and redo, capped by op count and by memory.
- `CrdtCompatible<S>` marker trait, declared with
  `#[tpt(commutative | associative | idempotent)]`, plus `OpTimestamp` and
  `LamportClock`.
- `ReplayEngine` over a versioned, serialized op log (`serde` feature), with
  `FailFast` and `SkipInvalid` policies and an auditor-readable trace.
- `MergeableState`, an opt-in join trait for CRDT-style states. It is separate
  from `State`, which has no merge.
- `SignedEntry` with `sign_op` and `verify_entry` (`crypto` feature). Each
  entry records its digest version, and a mismatch is refused.
- `crypto::tpt` adapter over `tpt-crypto`: SHA-256 (digest version 1) and
  Ed25519 signing and verification.
- `primitives` feature: maps `ActorId` to and from `tpt-primitives`' `Principal`
  using the label `actor:<n>`. Only the canonical form is accepted.
- `uir` feature: `uir::to_node` and `uir::from_node` convert an operation to and
  from a `UirNode`. Decoding rejects truncated input, unknown variants and
  trailing bytes.
- `pipeline` module (`crypto` feature): `commit` for local operations and
  `receive` for remote entries. Both apply to a copy of the state and commit
  only on success.
- `docs/integration.md`, covering the integration points with the sibling crates.

### Changed
- `OpError`, `ReplayError` and `Outcome` are `#[non_exhaustive]`.
- Public fields of `ReplayError` variants are documented.

### Notes
- The `uir` adapter is a stub. Digests cover postcard bytes, not canonical
  `tpt-uir` bytes, until `tpt-uir` defines that form.
- `Capability` is still the stub. The `tpt-primitives` model is not mapped yet.
- Licensed under `MIT OR Apache-2.0`.
