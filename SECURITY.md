# Security policy

## Reporting a vulnerability

Please report security problems privately, using GitHub's private vulnerability
reporting for this repository (Security tab, "Report a vulnerability"). Do not
open a public issue for them.

Include what you found, how to reproduce it, and which commit or version you
tested. Nothing is published to crates.io yet, so only the `master` branch is
supported.

## What is in scope

This crate signs and verifies operation logs, so these matter most:

- Accepting an entry that should be refused: a bad signature, an entry that
  claims an actor other than the key's owner, or a replayed sequence number
  (`pipeline::receive`, `crypto::verify_entry`, `pipeline::ReplayGuard`).
- Any way for a failed operation to change a state it should have left alone
  (`OperationBatch::apply`, `pipeline::commit`, `pipeline::receive`).
- Panics or unbounded memory use on untrusted input, such as a malformed log
  (`ReplayEngine::replay`, `uir::from_node`).
- Use of `unsafe` code or C-FFI dependencies. The crate forbids both.

## What the crate does not do

- It does not map keys to actors. The caller supplies the actor that a key
  belongs to, and keeps that registry.
- It does not order or deduplicate entries across authors. `ReplayGuard` only
  checks that each author's sequence numbers rise.
- It does not check capabilities yet. `Capability` is a stub.
- The digest covers the postcard encoding of an entry (digest version 1), not a
  canonical `tpt-uir` form.
