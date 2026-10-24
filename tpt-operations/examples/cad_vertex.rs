//! A CAD vertex move: a derived operation whose undo restores exact bits.
//!
//! Run it with `cargo run -p tpt-operations --example cad_vertex --features derive`.
//!
//! Floating-point positions are where "undo by subtracting" goes wrong. The
//! derived inverse snapshots the old position instead, so the bits come back.
//! Fields marked `#[tpt(ignore)]` are metadata: the op carries them, and the
//! state never sees them.

use tpt_operations::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct CadVertex {
    position: [f32; 3],
}

impl State for CadVertex {}

#[derive(Debug, Clone, PartialEq, Operation, SizeEstimate)]
#[tpt(state = CadVertex)]
struct MoveCadVertex {
    position: [f32; 3],
    // For the audit trail. Never written to the vertex.
    #[tpt(ignore)]
    reason: &'static str,
}

fn bits(vertex: &CadVertex) -> [u32; 3] {
    vertex.position.map(f32::to_bits)
}

fn main() -> Result<(), OpError> {
    let me = ActorId(1);
    let mut vertex = CadVertex {
        position: [0.1, -3.7, 1.0e7 + 0.3],
    };
    let start = bits(&vertex);
    let mut history = HistoryStack::new();

    let moves = [
        ([9.999_999e-5, 2.0 / 3.0, -0.0], "nudge"),
        ([1.0e7, 1.0e-7, 4.5], "snap to grid"),
    ];
    for (position, reason) in moves {
        let op = MoveCadVertex { position, reason };
        history.execute(&mut vertex, &tpt_batch![op], &me)?;
        println!("{reason:>12}: {:?}", vertex.position);
    }

    while history.undo(&mut vertex, &me)? {}
    println!("{:>12}: {:?}", "undone", vertex.position);

    assert_eq!(bits(&vertex), start, "byte-for-byte restore");
    println!("bits match the start: true");
    Ok(())
}
