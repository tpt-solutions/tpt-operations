//! Worked example: a CAD vertex move as a derived, reversible operation.
//!
//! Shows the intended shape of a real op: a few named fields that map to
//! state fields, metadata marked `#[tpt(ignore)]`, and an undo that restores
//! the exact bits of the original position.

#![cfg(feature = "derive")]

use tpt_operations::{
    core::{ActorId, Operation as _, State},
    Operation,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CadVertex {
    pub position: [f32; 3],
}

impl State for CadVertex {}

#[derive(Debug, Clone, PartialEq, Operation)]
#[tpt(state = CadVertex)]
pub struct MoveCadVertex {
    pub position: [f32; 3],
    // Metadata for the audit trace: carried by the op, never written to state.
    #[tpt(ignore)]
    pub reason: &'static str,
}

fn bits(v: &CadVertex) -> [u32; 3] {
    v.position.map(f32::to_bits)
}

#[test]
fn move_writes_the_new_position() {
    let mut vertex = CadVertex {
        position: [0.0, 0.0, 0.0],
    };
    let op = MoveCadVertex {
        position: [1.5, -2.25, 1e-6],
        reason: "snap to grid",
    };

    vertex.apply(&op).unwrap();
    assert_eq!(vertex.position, [1.5, -2.25, 1e-6]);
}

#[test]
fn undo_restores_the_exact_bits_after_a_lossy_move() {
    // Values chosen so that moving back by subtraction would drift.
    let mut vertex = CadVertex {
        position: [0.1, -3.7, 1.0e7 + 0.3],
    };
    let start = bits(&vertex);
    let op = MoveCadVertex {
        position: [9.999_999e-5, 2.0 / 3.0, -0.0],
        reason: "nudge",
    };

    let undo = op.inverse(&vertex).unwrap();
    vertex.apply(&op).unwrap();
    vertex.apply(&undo).unwrap();

    assert_eq!(bits(&vertex), start, "byte-for-byte restore");
}

#[test]
fn undo_carries_the_metadata_without_writing_it() {
    let vertex = CadVertex { position: [0.0; 3] };
    let op = MoveCadVertex {
        position: [1.0; 3],
        reason: "audit me",
    };

    let undo = op.inverse(&vertex).unwrap();
    assert_eq!(undo.reason, "audit me");
    assert_eq!(undo.position, [0.0; 3], "snapshot of the pre-state");
}

#[test]
fn validate_accepts_any_actor_by_default() {
    let vertex = CadVertex { position: [0.0; 3] };
    let op = MoveCadVertex {
        position: [1.0; 3],
        reason: "",
    };
    assert!(op.validate(&vertex, &ActorId(1)).is_ok());
}
