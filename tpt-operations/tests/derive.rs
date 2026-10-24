//! Behaviour of `#[derive(Operation)]`.

#![cfg(feature = "derive")]

use tpt_operations::{
    compose::OperationBatch,
    core::{ActorId, Operation as _, State},
    history::{HistoryStack, SizeEstimate},
    Operation,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub x: f32,
    pub y: f32,
}

impl State for Vehicle {}

#[derive(Debug, Clone, PartialEq, Operation)]
#[tpt(state = Vehicle)]
pub struct MoveTo {
    pub x: f32,
    pub y: f32,
    // Metadata: carried by the op, never written to the state.
    #[tpt(ignore)]
    pub note: &'static str,
}

impl SizeEstimate for MoveTo {
    fn estimated_size(&self) -> usize {
        core::mem::size_of::<Self>()
    }
}

#[derive(Debug, Clone, PartialEq, Operation)]
#[tpt(state = Vehicle)]
pub struct Noop;

fn actor() -> ActorId {
    ActorId(7)
}

fn bits(v: &Vehicle) -> (u32, u32) {
    (v.x.to_bits(), v.y.to_bits())
}

#[test]
fn apply_writes_the_fields_and_inverse_restores_exact_bits() {
    let mut state = Vehicle { x: 0.1, y: -3.7 };
    let start = bits(&state);
    let op = MoveTo {
        x: 1e-7,
        y: 2.5,
        note: "test",
    };

    let inverse = op.inverse(&state).unwrap();
    state.apply(&op).unwrap();
    assert_eq!(state, Vehicle { x: 1e-7, y: 2.5 });

    state.apply(&inverse).unwrap();
    assert_eq!(bits(&state), start, "byte-for-byte restore");
}

#[test]
fn inverse_is_a_snapshot_of_the_pre_state() {
    let state = Vehicle { x: 0.1, y: -3.7 };
    let op = MoveTo {
        x: 9.0,
        y: 9.0,
        note: "",
    };
    let inverse = op.inverse(&state).unwrap();
    // Not computed from the op's inputs. The snapshot holds the old values.
    assert_eq!(inverse.x, 0.1);
    assert_eq!(inverse.y, -3.7);
}

#[test]
fn ignored_fields_are_carried_by_the_inverse_but_not_written() {
    let state = Vehicle { x: 1.0, y: 1.0 };
    let op = MoveTo {
        x: 2.0,
        y: 2.0,
        note: "carried",
    };
    let inverse = op.inverse(&state).unwrap();
    assert_eq!(inverse.note, "carried");
}

#[test]
fn unit_struct_op_is_a_no_op_with_itself_as_inverse() {
    let mut state = Vehicle { x: 4.0, y: 5.0 };
    let before = state.clone();
    let inverse = Noop.inverse(&state).unwrap();
    state.apply(&Noop).unwrap();
    assert_eq!(state, before);
    assert_eq!(inverse, Noop);
}

#[test]
fn derived_ops_work_with_batches_and_history() {
    let mut history = HistoryStack::new();
    let mut state = Vehicle { x: 0.0, y: 0.0 };
    let start = state.clone();

    let batch: OperationBatch<MoveTo> = [
        MoveTo {
            x: 1.0,
            y: 1.0,
            note: "a",
        },
        MoveTo {
            x: 2.0,
            y: 3.0,
            note: "b",
        },
    ]
    .into_iter()
    .collect();

    history.execute(&mut state, &batch, &actor()).unwrap();
    assert_eq!(state, Vehicle { x: 2.0, y: 3.0 });

    history.undo(&mut state, &actor()).unwrap();
    assert_eq!(state, start);

    history.redo(&mut state, &actor()).unwrap();
    assert_eq!(state, Vehicle { x: 2.0, y: 3.0 });
}

#[derive(Debug, Clone, PartialEq, Operation)]
#[tpt(state = Vehicle, idempotent, associative)]
pub struct Teleport {
    pub x: f32,
    pub y: f32,
}

#[test]
fn declared_crdt_markers_are_emitted() {
    use tpt_operations::crdt::CrdtCompatible;
    const { assert!(<Teleport as CrdtCompatible<Vehicle>>::IS_IDEMPOTENT) };
    const { assert!(<Teleport as CrdtCompatible<Vehicle>>::IS_ASSOCIATIVE) };
    // Not declared, so the conservative default applies.
    const { assert!(!<Teleport as CrdtCompatible<Vehicle>>::IS_COMMUTATIVE) };
    const { assert!(!<MoveTo as CrdtCompatible<Vehicle>>::IS_IDEMPOTENT) };
}

#[test]
fn declared_idempotence_holds_on_samples() {
    use tpt_operations::crdt::check_idempotent;
    let bases = [
        Vehicle { x: 0.0, y: 0.0 },
        Vehicle { x: -3.5, y: 9.25 },
        Vehicle {
            x: f32::MAX,
            y: f32::MIN_POSITIVE,
        },
    ];
    let samples = [Teleport { x: 1.0, y: 2.0 }, Teleport { x: -0.0, y: 1e-30 }];
    for base in &bases {
        for op in &samples {
            assert!(check_idempotent(base, op));
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, tpt_operations::SizeEstimate)]
struct Tagged<T> {
    tag: T,
    payload: [u8; 16],
}

#[allow(dead_code)]
#[derive(Debug, Clone, tpt_operations::SizeEstimate)]
enum Shape {
    Dot,
    Line(f64, f64),
}

#[test]
fn derived_size_estimate_is_the_inline_size_for_structs_enums_and_generics() {
    let tagged = Tagged {
        tag: 1u32,
        payload: [0; 16],
    };
    assert_eq!(tagged.estimated_size(), core::mem::size_of::<Tagged<u32>>());
    assert_eq!(Shape::Dot.estimated_size(), core::mem::size_of::<Shape>());
    assert!(Shape::Line(1.0, 2.0).estimated_size() >= 16);
}
