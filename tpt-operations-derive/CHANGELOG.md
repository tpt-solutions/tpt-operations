# Changelog

All notable changes to this crate are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Nothing is published yet, so there are no released versions.

## [Unreleased]

### Added
- Generic structs are supported. Each type parameter gets a `Clone` bound.
- `#[tpt(validate = path)]`: `validate` calls a function you name.
- `#[derive(SizeEstimate)]`: the inline size of the type, for the history memory cap.
- `#[derive(Operation)]` for structs. It implements `Operation<S>` with `apply`,
  `inverse` and a `validate` that accepts every operation.
- `#[tpt(state = Type)]` container attribute, which names the state type `S`.
- `#[tpt(ignore)]` field attribute. Ignored fields are metadata: carried by the
  operation and never written to the state.
- `#[tpt(commutative)]`, `#[tpt(associative)]` and `#[tpt(idempotent)]` container
  attributes, which declare the matching `CrdtCompatible` markers. Each defaults
  to `false`.
- Compile errors for enums, unions, tuple structs, generic types, a missing
  `state` attribute, unknown container attributes, and inverse overrides.
  Inverses are always snapshots of the pre-state.

### Notes
- Licensed under `MIT OR Apache-2.0`.
