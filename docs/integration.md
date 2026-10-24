# Integration points

How `tpt-operations` connects to the rest of the TPT stack. Each point names
the module, the feature it needs, and the sibling crate it stands in for.
Crates that are out of scope for now (`tpt-doc`, `tpt-lattice`, `tpt-vertex`)
are left out.

## Action flow (`pipeline`, `crypto` feature)

- **Local:** `pipeline::commit` runs Validate, Inverse, Serialize, Sign and
  Apply. It returns a `SignedEntry` to broadcast and an `inverse` to keep for
  undo. Nothing is signed or applied unless validation and the inverse succeed.
- **Remote:** `pipeline::receive` runs Verify, Decode, Validate and Apply. The
  signature is checked before anything is decoded.
- **Author binding:** a signature proves that a key signed the entry, not that
  the key belongs to the actor named inside it. `receive` takes the `author`
  that the verifier's key belongs to and refuses an entry that claims another
  actor. Keep the key-to-actor registry on your side.
- **Replay protection:** each author numbers their entries with a rising `seq`,
  which is covered by the signature. `receive` keeps the highest accepted `seq`
  per author in a `ReplayGuard` and refuses anything not above it. The guard
  only advances when an entry applies. Deliver each author's entries in order.
- **Broadcast:** not part of this crate. Pass the `SignedEntry` to the
  transport layer.

## `tpt-replicate` (`uir` feature, dev-dependency for tests)

- `uir::to_node` and `uir::from_node` convert an operation to and from a
  `UirNode`. The node has the same shape as the `tpt-replicate` stub, so the
  payload passes between the two crates unchanged.
- To replicate an operation, send `uir::to_node(&op)?.bytes` to
  `Replica::local_op`. The receiving side decodes the same bytes with
  `uir::from_node`, inside its `CrdtCompatible::apply`.
- `tests/replicate.rs` shows the full bridge. It also checks that every batch
  grouping of the same operations reaches the author's state.

## `tpt-crypto` (`crypto` feature)

- `crypto::Digest`, `crypto::Signer` and `crypto::Verifier` are the boundary.
  `crypto::tpt` holds the SHA-256 and Ed25519 adapters.
- Each `SignedEntry` records its digest version. A verifier refuses a version
  it does not support. Digest version 1 covers the postcard entry (with a domain
  tag, the digest version and `seq`), not the canonical `tpt-uir` form. That change is blocked until `tpt-uir` defines a
  canonical encoding.

## `tpt-primitives` (`primitives` feature)

- `ActorId` maps to and from `Principal`, with the label `actor:<n>`. Only the
  canonical form is accepted.
- `Capability` is still a stub. Its mint, grant and lease model needs epoch and
  logical-time context that `validate` does not yet receive.

## `tpt-concord` (audit)

- `history::ReplayEngine` rebuilds a state from a serialized log, with an
  auditor-readable trace (one line per entry). Its inputs are the signed
  entries' `bytes` concatenated in order. Verify each entry with `crypto` first.
