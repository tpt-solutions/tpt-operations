# tpt-operations — Task Checklist

Source: `spec.txt`. Tick items as they are finished.

## Decisions (confirmed)
- Dependencies (`tpt-uir`, `tpt-primitives`, `tpt-crypto`, `tpt-testkit`) are **local stubs behind traits**, gated by cargo features. Swap in the real crates later.
- Layout: Cargo workspace of 2 crates, `tpt-operations` (lib) and `tpt-operations-derive` (proc-macro).
- Inverses are **snapshot-based**: captured from the pre-application state, never computed arithmetically. This avoids f32 drift.
- Current deliverable: this checklist only. No code until it is approved.

## Assumptions (veto any of these)
- [ ] Rust edition 2021, stable toolchain, MSRV pinned in `rust-toolchain.toml`
- [ ] `#![no_std]` + `alloc` by default; `std` is an optional feature
- [ ] Serialization is `serde` (no_std) with postcard for the wire; hashing goes through canonical `tpt-uir`
- [ ] Add the `uuid` crate (v7, no_std)
- [ ] `#![forbid(unsafe_code)]`, no C-FFI deps (enforced by `cargo deny`)
- [ ] The spec's trait sketches are adjusted where they don't compile (e.g. `CrdtCompatible: Operation` needs a generic `S`; `State::apply` taking `&impl Operation<Self>`)

## Phase 0 — Scaffold
- [ ] Create the workspace `Cargo.toml`
- [ ] Create the `tpt-operations` crate (no_std + alloc, feature flags: `std`, `derive`, `uir`, `crypto`, `primitives`)
- [ ] Create the `tpt-operations-derive` proc-macro crate
- [ ] Add `rust-toolchain.toml`, `rustfmt.toml`, clippy config
- [ ] Add `deny.toml` (ban C-FFI / `-sys` crates)
- [ ] CI: build and test on Linux, macOS and Windows; `no_std` target build (`thumbv7em-none-eabi` / `riscv32imac`, `wasm32-unknown-unknown`)
- [ ] README and crate-level docs

## Phase 1 — Core Traits & Derive Macros (Weeks 1-3)
### Module A: `core`
- [ ] `OpError` enum (validation, bounds, capability, not-found, conflict, serialization)
- [ ] `State` trait (`Clone + Sized`, `apply` only, no merge)
- [ ] Optional separate `MergeableState` trait
- [ ] `Operation<S>` trait (`apply`, `inverse`, `validate`) with associated `type Inverse`
- [ ] `inverse(&self, &S) -> Result<Self::Inverse, OpError>`, built from the pre-state snapshot
- [ ] Stub `ActorId` and `Capability` (feature-gated, later replaced by `tpt-primitives`)
- [ ] Docs for the "Inverse Drift" rule
- [ ] Unit tests with a toy counter state and a toy map state
### Module B: `derive`
- [ ] `#[derive(Operation)]` skeleton
- [ ] Generate `apply`, `inverse` and `validate` stubs
- [ ] `#[tpt(inverse = "...")]` attribute parsing
- [ ] `#[tpt(ignore)]` attribute for metadata fields
- [ ] Snapshot-based inverse generation (no float negation)
- [ ] Compile-fail tests (`trybuild`) for bad attributes
- [ ] Example: `MoveCadVertex`, with a byte-for-byte round-trip test
- [ ] **Milestone:** simple reversible ops with minimal boilerplate

## Phase 2 — History & Batching (Weeks 4-5)
### Module C: `compose`
- [ ] `OperationBatch` container
- [ ] Atomic apply with rollback via the already-applied inverses
- [ ] Rollback on validation failure and on apply failure
- [ ] The batch's own `inverse` (reverse order)
- [ ] `tpt_batch!` macro
- [ ] Tests for partial failure at every position in a batch
### Module E (part 1): `history::HistoryStack`
- [ ] Applied and inverted (redo) queues
- [ ] Cap by op count (default 10,000)
- [ ] Cap by memory (default 50 MB), with a size-estimate trait
- [ ] Redo is cleared when a new op is applied
- [ ] Batches are treated as a single undo step
- [ ] Tests for eviction ordering and cap boundaries
- [ ] **Milestone:** `tpt-doc` can integrate multi-level undo/redo

## Phase 3 — CRDT Compatibility & Replay (Weeks 6-8)
### Module D: `crdt`
- [ ] `CrdtCompatible` marker trait (`IS_COMMUTATIVE`, `IS_ASSOCIATIVE`, `IS_IDEMPOTENT`)
- [ ] Derive or attribute support for declaring the markers
- [ ] Op timestamp / ordering metadata (e.g. Lamport clock + ActorId) for deterministic tie-breaks
- [ ] Verification helpers that check the declared properties
### Module E (part 2): `history::ReplayEngine`
- [ ] Reconstruct state from an empty initial state plus a serialized op stream
- [ ] Deterministic error handling (fail fast vs. skip policy)
- [ ] Replay trace output for auditors
- [ ] Serialized op format with a version tag
### Testing infrastructure
- [ ] Local stub of `tpt-testkit::property` (generators, shrinking, runner)
- [ ] Property test: `state == apply(inverse(state), state)` over millions of cases
- [ ] Property test: shuffled application of commutative ops gives an identical state hash
- [ ] Property test: idempotent ops applied twice give an identical state hash
- [ ] Property test: associativity for marked ops
- [ ] Golden file: 1,000-op sequence, identical result on Linux, macOS and Windows
- [ ] Build fails if a reversibility test fails (CI gate)
- [ ] **Milestone:** `tpt-lattice` and `tpt-vertex` can use these ops for real-time collaboration

## Phase 4 — Ecosystem Integration & Crypto (Weeks 9-10)
- [ ] `Signer`/`Verifier` trait boundary, with a feature-gated adapter for `tpt-crypto`
- [ ] Hash the canonical `tpt-uir` bytes; postcard is the transmission format only
- [ ] `SignedOperation` wrapper
- [ ] Adapter for the real `tpt-primitives` (`ActorId`, `Capability`) once available
- [ ] Capability-based `validate`
- [ ] `tpt-uir` node conversion (`Operation` <-> IR node)
- [ ] The full action flow: Intent → Create → Validate → Apply → Inverse → Serialize → Sign → Broadcast
- [ ] End-to-end integration test of the pipeline
- [ ] Docs for the ecosystem integration points (vertex, lattice, doc, concord)
- [ ] API review, semver audit, changelog
- [ ] Publish `v1.0.0`

## Resolved questions
- [x] `State` has no merge method. It is a minimal data container with `apply`. Merging belongs to `tpt-replicate`, which uses the CRDT markers.
- [x] An optional, separate merge trait (e.g. `MergeableState`) exists only for CRDT cases such as `tpt-lattice` cell states.
- [x] `Operation` gets an associated type `Inverse`. `inverse` returns `Self::Inverse`. `#[derive(Operation)]` defaults to `Inverse = Self`.
- [x] Hash the canonical `tpt-uir` form of the op. Postcard is the wire format only, and signatures cover the hash, not the postcard bytes.
- [x] `UuidV7` comes from the `uuid` crate (no_std, feature `v7`).
