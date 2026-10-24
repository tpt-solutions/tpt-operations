# tpt-operations — Task Checklist

Source: `spec.txt`. Tick items as they are finished.

## Decisions (confirmed)
- Dependencies (`tpt-uir`, `tpt-primitives`, `tpt-crypto`, `tpt-testkit`) are **local stubs behind traits**, gated by cargo features. Swap in the real crates later.
- Layout: Cargo workspace of 2 crates, `tpt-operations` (lib) and `tpt-operations-derive` (proc-macro).
- Inverses are **snapshot-based**: captured from the pre-application state, never computed arithmetically. This avoids f32 drift.
- Approved to proceed (2026-10-25). Phase 0 and Module A/B skeleton are implemented; the rest is open.

## Assumptions (veto any of these)
- [ ] Rust edition 2021, stable toolchain, MSRV pinned in `rust-toolchain.toml`
- [ ] `#![no_std]` + `alloc` by default; `std` is an optional feature
- [ ] Serialization is `serde` (no_std) with postcard for the wire; hashing goes through canonical `tpt-uir`
- [ ] Add the `uuid` crate (v7, no_std)
- [ ] `#![forbid(unsafe_code)]`, no C-FFI deps (enforced by `cargo deny`)
- [ ] The spec's trait sketches are adjusted where they don't compile (e.g. `CrdtCompatible: Operation` needs a generic `S`; `State::apply` taking `&impl Operation<Self>`)

## Phase 0 — Scaffold
- [x] Create the workspace `Cargo.toml`
- [x] Create the `tpt-operations` crate (no_std + alloc, feature flags: `std`, `derive`, `uir`, `crypto`, `primitives`)
- [x] Create the `tpt-operations-derive` proc-macro crate
- [x] Add `rust-toolchain.toml`, `rustfmt.toml`, clippy config (`clippy.toml`, workspace lints)
- [x] Add `deny.toml` (ban C-FFI / `-sys` crates; explicit list because cargo-deny has no wildcards)
- [~] CI: `.github/workflows/ci.yml` written (test matrix, no_std targets, cargo-deny). Not yet run on GitHub. `no_std` builds verified locally for `thumbv7em-none-eabihf`, `riscv32imac-unknown-none-elf` and `wasm32-unknown-unknown`.
- [x] README and crate-level docs

## Phase 1 — Core Traits & Derive Macros (Weeks 1-3)
### Module A: `core`
- [x] `OpError` enum (validation, bounds, capability, not-found, conflict, serialization)
- [x] `State` trait (`Clone + Sized`, `apply` only, no merge)
- [x] Optional separate `MergeableState` trait
- [x] `Operation<S>` trait (`apply`, `inverse`, `validate`) with associated `type Inverse`
- [x] `inverse(&self, &S) -> Result<Self::Inverse, OpError>`, built from the pre-state snapshot
- [x] Stub `ActorId` and `Capability` (always compiled for now; the `primitives` feature will swap in the real types)
- [x] Docs for the "Inverse Drift" rule
- [x] Unit tests with a toy counter state and a toy map state (10 passing)
### Module B: `derive`
- [x] `#[derive(Operation)]` entry point and `tpt` helper attribute
- [x] Generate `apply`, `inverse` and `validate`. Design: `#[tpt(state = Type)]` on the struct; each named field maps to the state field of the same name. `validate` accepts everything, and you override it by hand-writing the impl.
- [x] `#[tpt(inverse = "...")]` is rejected with a compile error. Inverses are never written as expressions, because that reintroduces drift. Decided on the recommendation.
- [x] `#[tpt(ignore)]` for metadata fields (carried by the op and its inverse, never written to state)
- [x] Snapshot-based inverse generation (no float negation)
- [x] Compile-fail tests (`trybuild`) for bad attributes: inverse override, missing state, tuple struct, generic, enum, typo'd container attribute
- [x] Example: `MoveCadVertex`, with a byte-for-byte round-trip test (`tests/move_cad_vertex.rs`)
- [x] **Milestone:** simple reversible ops with minimal boilerplate (`derive` feature)

## Phase 2 — History & Batching (Weeks 4-5)
### Module C: `compose`
- [x] `OperationBatch` container
- [x] Atomic apply with rollback via the already-applied inverses
- [x] Rollback on validation failure and on apply failure
- [x] The batch's own `inverse` (reverse order)
- [x] `tpt_batch!` macro
- [x] Tests for partial failure at every position in a batch
### Module E (part 1): `history::HistoryStack`
- [x] Applied and inverted (redo) queues
- [x] Cap by op count (default 10,000)
- [x] Cap by memory (default 50 MB), with a size-estimate trait (`SizeEstimate`)
- [x] Redo is cleared when a new op is applied
- [x] Batches are treated as a single undo step
- [x] Tests for eviction ordering and cap boundaries
- ~~[ ] **Milestone:** `tpt-doc` can integrate multi-level undo/redo (needs the derive macro, Module B, to define real ops)~~ dropped from this scope (2026-10-25)

## Phase 3 — CRDT Compatibility & Replay (Weeks 6-8)
### Module D: `crdt`
- [x] `CrdtCompatible<S>` marker trait (`IS_COMMUTATIVE`, `IS_ASSOCIATIVE`, `IS_IDEMPOTENT`). Generic over `S`, per the resolved assumption.
- [x] Derive or attribute support for declaring the markers: `#[tpt(commutative | associative | idempotent)]`, each defaulting to `false`
- [x] Op timestamp / ordering metadata (`OpTimestamp` = Lamport counter + `ActorId`, plus `LamportClock`)
- [~] Verification helpers that check the declared properties: `check_commutative` and `check_idempotent` are done. Associativity has no helper, and that is deliberate. Sequential application is associative by construction, so a check would only test the implementation against itself. The useful check is grouping of merges, which needs `tpt-replicate`. Revisit then.
### Module E (part 2): `history::ReplayEngine`
- [x] Reconstruct state from an empty initial state plus a serialized op stream (`history::replay`, `serde` feature)
- [x] Deterministic error handling: `ReplayPolicy::FailFast` or `SkipInvalid`. Skipped ops roll back atomically. Decode errors always stop.
- [x] Replay trace output for auditors (`ReplayTrace`, one line per entry via `Display`)
- [x] Serialized op format with a version tag (`FORMAT_VERSION = 1`, entries are `(version, actor, op)` in postcard)
### Testing infrastructure
- [~] Local stub: `src/testkit.rs` has a seeded xorshift RNG, ranges, and shuffle. No shrinking, no runner. Checked `tpt-testkit::property` (`tpt-solutions/tpt-testkit`): it is proptest strategies for file formats (JSON, CSV, PDF) plus `check_round_trip`. It has no seeded RNG or shuffle, so it is not a drop-in replacement. Keeping the stub. `check_round_trip` could be used for codec round-trip tests later.
- [~] Property test: `state == apply(inverse(state), state)`. Done with 100,000 cases per op type, not millions. `CASES` is the knob.
- [x] Property test: shuffled application of commutative ops gives an identical state (compared by equality, not by hash yet)
- [x] Property test: idempotent ops applied twice give an identical state
- [x] Property test: associativity for marked ops. `tests/replicate.rs` delivers the same stamped ops whole, one at a time, and in random contiguous batches through the `tpt-replicate` merge executor (pinned to `1203d9e`, dev-dependency). Every grouping must match the author's state. Asserts `IS_ASSOCIATIVE` at compile time. Needs `--features uir`.
- [~] Golden file: 1,000-op sequence in `tests/golden/replay_1000.bin`, with a pinned state digest. Checked on Windows only. The Linux and macOS check runs in CI once pushed.
- [x] Build fails if a reversibility test fails (CI gate). The property tests run under `cargo test`, which CI runs on all three OSes.
- ~~[ ] **Milestone:** `tpt-lattice` and `tpt-vertex` can use these ops for real-time collaboration~~ dropped from this scope (2026-10-25)

## Phase 4 — Ecosystem Integration & Crypto (Weeks 9-10)
- [x] `Signer`/`Verifier`/`Digest` trait boundary (`crypto` feature), with the real `tpt-crypto` adapter (`crypto::tpt`: SHA-256 and Ed25519). Pinned to `tpt-crypto` commit `8b73d9b`. Checked against the FIPS 180 and RFC 8032 vectors.
- [ ] Hash the canonical `tpt-uir` bytes; postcard is the transmission format only. **Temporary deviation:** the digest is SHA-256 over postcard bytes (digest version 1). Each `SignedEntry` records its digest version, so the canonical form lands as version 2 without invalidating existing entries. Blocked on `tpt-uir` defining a canonical encoding; its spec does not yet.
- [x] `SignedEntry` wrapper with `sign_op` and `verify_entry`. Verification runs before any decoding, and tests cover tampering and wrong keys (toy primitives only).
- [~] Adapter for the real `tpt-primitives`. `ActorId` ↔ `Principal` is done (`primitives` feature, label `actor:<n>`, canonical form only, pinned to `336292c`). `Capability` is open. Its mint/grant/lease model needs epoch and logical-time context that our `validate` does not have, so it waits for a real caller.
- [ ] Capability-based `validate`
- [x] `tpt-uir` node conversion (`Operation` <-> IR node). `uir` feature: `uir::to_node` and `uir::from_node` over a `UirNode` with the same shape as the `tpt-replicate` stub. The payload is postcard, and decoding rejects truncated input, unknown variants and trailing bytes. Swap in the canonical `tpt-uir` encoding when it exists.
- [x] The full action flow: Validate → Inverse → Serialize → Sign → Apply (`pipeline::commit`), and Verify → Decode → Validate → Apply on the receiving side (`pipeline::receive`). Broadcast belongs to the transport (`tpt-replicate`). Intent and Create are the caller's op value.
- [x] End-to-end integration test of the pipeline (`tests/pipeline.rs`): author → peer and author → replay agree; each inverse restores its pre-state; refused ops and tampered or wrong-key entries leave state unchanged
- [x] Docs for the ecosystem integration points (`docs/integration.md`). `vertex`, `lattice` and `doc` are out of scope, so only replicate, crypto, primitives and concord are covered.
- [~] API review, semver audit, changelog. `CHANGELOG.md` (Unreleased) is written. Done in the review pass: every public item is documented, and `OpError`, `ReplayError` and `Outcome` are `#[non_exhaustive]`, so new failure kinds can be added later. `cargo doc -D warnings` is clean. There is no published baseline, so a semver diff has nothing to compare against. **Decided (2026-10-25):** (1) Capability validation stays blocked until a real caller exists. Changing `Operation::validate` now is a breaking trait change with nothing to design it against. (2) Keep `ActorId(pub u64)`, which matches `tpt-replicate`, and keep `Capability { pub name }`, a stub the primitives adapter will replace. (3) Pedantic lints (`# Errors`, `#[must_use]`) are not adopted.
- [ ] Publish `v1.0.0`

## Phase 5 — Review follow-ups (2026-10-25)
Source: review plan. Bug claims come from code reading. Confirm each with a failing test before fixing.

### Correctness and security (fix first)
- [x] Confirm A1-A5 below with tests (partial-apply, spoofing, replay, version order). Written alongside the fixes, not as separate failing commits.
- [x] A1: bind the signing key to the `ActorId`. Today `verify_entry` and `pipeline::receive` trust the actor inside the signed bytes, so any key holder can claim any actor `pipeline::receive` now takes the expected `author` and refuses a different claimed actor.
- [x] A2: refuse replayed entries. `SignedEntry.seq` is signed with a domain tag, and `ReplayGuard` keeps the highest accepted `seq` per author.
- [x] A3: roll back a failed op's own partial mutation in `OperationBatch::apply` Done by applying on a clone (one state clone per batch).
- [x] A4: a failed rollback can no longer happen. Batches work on a clone, so there is no rollback, and a failed undo/redo leaves the state untouched.
- [x] A5: decode the format version before the rest of the entry so a future layout gives `UnsupportedVersion`, not `Decode`.
- [x] A6: `LamportClock::tick` returns `Result` and errors once the counter is exhausted.
- [x] A7: `digest_version` is part of the signing input.
- [x] A8: `HistoryStack` measures each stored batch. Redo is not capped on purpose: it only holds undone steps, so it cannot outgrow the undo history.
- [x] Replace the `expect` in `primitives.rs` (`principal_of` returns `Result`)
- [x] Fix the divide-by-zero in `testkit::Rng::range_i64` for the full `i64` range
- [~] Stale "stub" docs in `core/identity.rs` refreshed. Still open: note in `lib.rs` that `uir` is a stub

### Missing tests
- [x] Partial-apply op (the failed-rollback `Conflict` path no longer exists)
- [x] Entry replay and actor spoofing through `receive`
- [x] `LamportClock` exhaustion after `observe` with a huge stamp
- [~] `HistoryStack`: zero limits, a failed `execute` leaves redo intact, failed undo/redo, and byte accounting are done. Eviction with redo is open.
- [x] Replay: trailing garbage and a version mismatch with a changed layout (a partially applying op is covered by the batch tests)
- [~] A failing `Signer` through `commit` is done. Trailing bytes in `verify_entry` and empty or oversized signatures are open.
- [ ] `uir` with an empty node
- [ ] Golden file on Linux and macOS (runs in CI once pushed)

### Adoption quick wins
- [ ] `tpt_operations::prelude` (`State`, `Operation`, `OpError`, `ActorId`, `OperationBatch`, `tpt_batch!`, `HistoryStack`, `SizeEstimate`, derive when enabled)
- [ ] `#![doc = include_str!("../README.md")]` so README snippets compile; replace the `ignore` on the `tpt_batch!` example
- [ ] Features decision table ("I want X, enable Y") in the README
- [ ] `#[derive(SizeEstimate)]`
- [ ] Ergonomics: `Display`, `From<u64>` and `Default` for `ActorId`; `IntoIterator`, `Extend`, `From<Vec>` and `into_ops()` for `OperationBatch`; `ReplayEngine::default()`; `HistoryStack::clear()` and peek; take `ActorId` by value consistently
- [ ] Doc-tests for the main types
- [ ] README: say `derive` is off by default, and replace the unpublished `version = "0.1"` snippet until the crate is published

### Examples (`examples/`, each runnable with `cargo run --example`, built in CI)
- [ ] `counter`
- [ ] `text_doc_undo` (HistoryStack with batches)
- [ ] `cell_crdt` (Lamport stamps and a convergence check)
- [ ] `cad_vertex` (promote `tests/move_cad_vertex.rs`)
- [ ] `signed_audit_log` (commit, receive, tamper detection)
- [ ] `event_sourcing_replay` (write a log, replay, print the trace)
- [ ] `delta_op` (hand-written inverse, the Inverse Drift lesson)
- [ ] `no_std`

### Tooling and templates
- [ ] Public `testkit` feature: `assert_reversible`, `assert_commutative`, `assert_idempotent`, `assert_replay_deterministic`, with a seeded RNG, shrinking and a diagnostic result (not a bare `bool`). `check_commutative` and `check_idempotent` should say which step failed
- [ ] `cargo-generate` templates: `state-and-ops`, `collab-ledger`, `audit-log`
- [ ] `cargo tpt-ops` CLI: `new`, `inspect`, `replay --trace`, `verify`, `check`
- [ ] Structured `OpError` context (op, field, actor, source) with hints, and `From<postcard::Error>`
- [ ] Derive: `SizeEstimate`, enum ops, `#[tpt(validate = path)]`, delta fields, generics; the derive error for unsupported shapes says "write the impl by hand"
- [ ] Benches: replay throughput, sign/verify, history eviction
- [ ] Docs: concepts guide (mdBook), "why snapshot inverses", "migrating from hand-rolled undo"

### Release hygiene
- [ ] Publish blockers: git-pinned optional dependencies (`tpt-crypto`, `tpt-primitives`, dev-dep `tpt-replicate`); add a `version` to the derive crate's path dependency
- [ ] Per-crate `LICENSE-MIT` and `LICENSE-APACHE` (the README links break on crates.io)
- [ ] `[package.metadata.docs.rs]` (`all-features`, `--cfg docsrs`) and `doc(cfg)` on gated modules
- [ ] Cargo metadata (`homepage`, `documentation`, `authors`) and `exclude` for `tests/golden` and `tests/ui`; consider `publish = false` until ready
- [ ] `SECURITY.md`, `CONTRIBUTING.md`, issue and PR templates, README badges
- [ ] CI: MSRV job, `cargo doc -D warnings`, `cargo semver-checks`, `cargo package --dry-run`, scheduled `cargo-deny` advisories run, Dependabot, `rust-cache`, examples build, `permissions:` and SHA-pinned actions, no_std job for `uir` and `primitives`
- [ ] `deny.toml`: add `[graph]` targets and advisories settings, and review the license allow-list (`Unicode-DFS-2016`, `Zlib`, `MPL-2.0`, `CC0-1.0`). The `-sys` ban list misses crates that bundle C without that name
- [ ] Remove the invalid `msrv` key from `rust-toolchain.toml` (the real MSRV is `rust-version` in `Cargo.toml`)
- [ ] Reconcile `spec.txt` with the code (`inverse` return type, generic `CrdtCompatible`, derive attributes, pipeline order) or mark it historical
- [ ] Tick the stale "Assumptions" boxes, and fix the `uuid` item (ticked as resolved but not in any `Cargo.toml`)
- [ ] Commit the untracked work (`docs/`, `pipeline.rs`, `uir.rs`, tests)

### Ideas (post-1.0)
- [ ] Time-travel debugger over `ReplayTrace`
- [ ] `#[tpt(test)]` on the derive, generating the reversible, commutative and idempotent property tests
- [ ] Log compaction and signed snapshot checkpoints (uses digest versioning)
- [ ] Replay-and-verify against a golden digest as a CI regression gate
- [ ] Tamper-evident log chaining (each entry commits to the previous digest)
- [ ] Capability-gated ops once `tpt-primitives` has a real caller (decide the `validate` signature before 1.0)

## Resolved questions
- [x] `State` has no merge method. It is a minimal data container with `apply`. Merging belongs to `tpt-replicate`, which uses the CRDT markers.
- [x] An optional, separate merge trait (e.g. `MergeableState`) exists only for CRDT cases such as `tpt-lattice` cell states.
- [x] `Operation` gets an associated type `Inverse`. `inverse` returns `Self::Inverse`. `#[derive(Operation)]` defaults to `Inverse = Self`.
- [x] Hash the canonical `tpt-uir` form of the op. Postcard is the wire format only, and signatures cover the hash, not the postcard bytes.
- [x] `UuidV7` comes from the `uuid` crate (no_std, feature `v7`).
