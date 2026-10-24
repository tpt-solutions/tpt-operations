# tpt-operations-derive

Derive macros for [`tpt-operations`](../tpt-operations/README.md). The crate
provides `#[derive(Operation)]`, which implements `Operation<S>` for a struct
whose named fields mirror fields of a state type `S`.

Most users do not depend on this crate directly. Enable the `derive` feature of
`tpt-operations` and use the re-exported macro:

```toml
[dependencies]
tpt-operations = { version = "0.1", features = ["derive"] }
```

## Example

```rust
use tpt_operations::core::{Operation as _, OpError, State};
use tpt_operations::Operation;

#[derive(Debug, Clone, PartialEq)]
struct Counter {
    value: i64,
}

impl State for Counter {}

#[derive(Clone, Operation)]
#[tpt(state = Counter)]
struct SetValue {
    value: i64,
    // Metadata: carried by the operation, never written to the state.
    #[tpt(ignore)]
    author: String,
}
```

## Generated behaviour

- `apply` writes each non-ignored field into the state field of the same name.
- `inverse` snapshots the pre-application state values into a new `Self`.
  Inverses are never written as expressions, so they cannot drift.
- `validate` accepts every operation by default. `#[tpt(validate = path)]` calls
  `path(&self, &state, &actor)` instead. To change more than that, write the
  trait impl by hand.
- `Inverse` is `Self`.
- Generic structs work. Every type parameter gets a `Clone` bound.

## Attributes

| Attribute | Where | Effect |
| --- | --- | --- |
| `#[tpt(state = Type)]` | struct | Required. The state type `S` that the operation mutates. |
| `#[tpt(validate = path)]` | struct | A function `fn(&Op, &State, &ActorId) -> Result<(), OpError>` that `validate` calls. |
| `#[tpt(ignore)]` | field | Excluded from `apply` and `inverse`. The field is carried as metadata. |
| `#[tpt(commutative)]` | struct | Declares the `CrdtCompatible` commutative marker. Defaults to `false`. |
| `#[tpt(associative)]` | struct | Declares the `CrdtCompatible` associative marker. Defaults to `false`. |
| `#[tpt(idempotent)]` | struct | Declares the `CrdtCompatible` idempotent marker. Defaults to `false`. |

Declare a marker only if it holds for every value of the type. Field-setting
operations are idempotent but not commutative in general, so they declare
`idempotent` only.

## Compile-time errors

The derive rejects these inputs with a compile error:

- Enums and unions. Only structs are supported.
- Tuple structs. Use named fields, or a unit struct.

Each message says to write the `Operation` impl by hand for those shapes. The
`text_doc_undo` and `delta_op` examples in `tpt-operations` show what that looks like.
- A missing `#[tpt(state = ...)]`.
- Unknown container attributes.
- Inverse overrides. Inverses are always snapshots of the pre-state, so write
  the `Operation` impl by hand if a custom inverse is needed.

## Status

Nothing is published yet. See the
[`tpt-operations` README](../tpt-operations/README.md#status) for the
publishing constraints, and [`CHANGELOG.md`](CHANGELOG.md) for the release
history.

## Development

```sh
cargo test -p tpt-operations --features derive
```

The `trybuild` tests in `tpt-operations/tests/ui/` cover the compile errors above.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
at your option.
