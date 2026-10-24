# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Nothing is published yet, so there are no released versions.

## [Unreleased]

### Added
- `prelude` module: `use tpt_operations::prelude::*;` brings in the common names.
- `#[derive(SizeEstimate)]` (`derive` feature), the inline size of the type.
- Runnable examples: `counter`, `text_doc_undo`, `cell_crdt`, `cad_vertex`,
  `delta_op`, `signed_audit_log` and `event_sourcing_replay`.
- `ActorId` implements `Display`, `Default` and the `u64` conversions.
- `OperationBatch` implements `From<Vec>`, `Extend` and `IntoIterator`, and has
  `ops` and `into_ops`.
- `HistoryStack::clear` and `ReplayEngine::default`.
- Quick-start and feature table in the crate docs, and the README example is
  compiled as a doc-test.
- `ReplayGuard`, `SignedEntry::seq` and the author check in `pipeline::receive`.
- `State`, `Operation<S>` and `OpError` core traits. Inverses are built from a
  pre-application snapshot, never by arithmetic on the op's inputs.
- `#[derive(Operation)]` with `#[tpt(state = ...)]` and `#[tpt(ignore)]`.
  Inverse overrides are rejected at compile time.
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
- `OperationBatch::apply` runs on a copy of the state and replaces the state only
  if every operation succeeds. A failing `apply` that had already changed part of
  a state no longer leaks that change, and rollback (with its `Conflict` error)
  is gone. The cost is one state clone per batch.
- `pipeline::commit` takes a `seq`, and `pipeline::receive` takes the expected
  `author`, a `ReplayGuard` and no longer returns the actor. `crypto::sign_op`
  takes a `seq`, and `SignedEntry` has a `seq` field.
- The signature covers a domain tag, the digest version, `seq` and the entry
  bytes, not only the entry bytes. `receive` refuses an entry that claims an
  actor other than the key's owner, and one whose `seq` is not above the last
  accepted from that author.
- The format version is decoded before the rest of an entry, so a newer layout
  reports `UnsupportedVersion` instead of a decode error.
- `LamportClock::tick` returns `Result` and errors when the counter is exhausted,
  instead of saturating and repeating a stamp.
- `primitives::principal_of` returns `Result` instead of calling `expect`.
- `HistoryStack` measures each step from the batch it stores, so `undo_bytes`
  stays accurate across undo and redo.
- `OpError`, `ReplayError` and `Outcome` are `#[non_exhaustive]`.
- Public fields of `ReplayError` variants are documented.

### Notes
- The `uir` adapter is a stub. Digests cover postcard bytes, not canonical
  `tpt-uir` bytes, until `tpt-uir` defines that form.
- `Capability` is still the stub. The `tpt-primitives` model is not mapped yet.
- Licensed under `MIT OR Apache-2.0`.
