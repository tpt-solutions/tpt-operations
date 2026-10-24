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
- [ ] Optional separate `MergeableState` trait
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
- [ ] Example: `MoveCadVertex`, with a byte-for-byte round-trip test (`tests/derive.rs` covers the same pattern with a `MoveTo` op, bit-exact on `f32`)
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
- [ ] **Milestone:** `tpt-doc` can integrate multi-level undo/redo (needs the derive macro, Module B, to define real ops)

## Phase 3 — CRDT Compatibility & Replay (Weeks 6-8)
### Module D: `crdt`
- [x] `CrdtCompatible<S>` marker trait (`IS_COMMUTATIVE`, `IS_ASSOCIATIVE`, `IS_IDEMPOTENT`). Generic over `S`, per the resolved assumption.
- [ ] Derive or attribute support for declaring the markers (needs Module B)
- [x] Op timestamp / ordering metadata (`OpTimestamp` = Lamport counter + `ActorId`, plus `LamportClock`)
- [~] Verification helpers that check the declared properties: `check_commutative` and `check_idempotent` are done. Associativity has no helper yet (sequential composition is always associative, so the check needs a batch-grouping definition first).
### Module E (part 2): `history::ReplayEngine`
- [ ] Reconstruct state from an empty initial state plus a serialized op stream
- [ ] Deterministic error handling (fail fast vs. skip policy)
- [ ] Replay trace output for auditors
- [ ] Serialized op format with a version tag
### Testing infrastructure
- [~] Local stub of `tpt-testkit::property`: `src/testkit.rs` has a seeded xorshift RNG, ranges, and shuffle. No shrinking, no runner.
- [~] Property test: `state == apply(inverse(state), state)`. Done with 100,000 cases per op type, not millions. `CASES` is the knob.
- [x] Property test: shuffled application of commutative ops gives an identical state (compared by equality, not by hash yet)
- [x] Property test: idempotent ops applied twice give an identical state
- [ ] Property test: associativity for marked ops
- [ ] Golden file: 1,000-op sequence, identical result on Linux, macOS and Windows (needs a serialized format)
- [x] Build fails if a reversibility test fails (CI gate). The property tests run under `cargo test`, which CI runs on all three OSes.
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
