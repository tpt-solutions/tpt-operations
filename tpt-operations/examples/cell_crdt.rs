//! A last-writer-wins spreadsheet cell, and a check that replicas converge.
//!
//! Run it with `cargo run -p tpt-operations --example cell_crdt`.
//!
//! `SetCell` carries a Lamport stamp, and a write only wins if its stamp is
//! higher than the cell's current one. That makes it commutative, associative
//! and idempotent, so replicas that receive the same writes in any order, with
//! duplicates, end in the same state. The `CrdtCompatible` markers declare it,
//! and `check_commutative` and `check_idempotent` test the claim on samples.

use std::collections::BTreeMap;
use tpt_operations::{
    crdt::{check_commutative, check_idempotent, CrdtCompatible, LamportClock, OpTimestamp},
    prelude::*,
};

type Cell = (OpTimestamp, i64);

#[derive(Debug, Clone, PartialEq, Default)]
struct Sheet {
    cells: BTreeMap<String, Cell>,
}

impl State for Sheet {}

/// A stamped write. The highest stamp wins.
#[derive(Debug, Clone)]
struct SetCell {
    key: String,
    value: i64,
    stamp: OpTimestamp,
}

/// Puts a cell back to exactly what it was, including the stamp. This is the
/// inverse, used for undo. It is not a CRDT operation: it ignores stamps.
#[derive(Debug, Clone)]
struct RestoreCell {
    key: String,
    previous: Option<Cell>,
}

impl Operation<Sheet> for SetCell {
    type Inverse = RestoreCell;

    fn apply(&self, state: &mut Sheet) -> Result<(), OpError> {
        let wins = state
            .cells
            .get(&self.key)
            .is_none_or(|(current, _)| self.stamp > *current);
        if wins {
            state
                .cells
                .insert(self.key.clone(), (self.stamp, self.value));
        }
        Ok(())
    }

    fn inverse(&self, state: &Sheet) -> Result<RestoreCell, OpError> {
        Ok(RestoreCell {
            key: self.key.clone(),
            previous: state.cells.get(&self.key).copied(),
        })
    }

    fn validate(&self, _state: &Sheet, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

impl Operation<Sheet> for RestoreCell {
    type Inverse = RestoreCell;

    fn apply(&self, state: &mut Sheet) -> Result<(), OpError> {
        match self.previous {
            Some(cell) => state.cells.insert(self.key.clone(), cell),
            None => state.cells.remove(&self.key),
        };
        Ok(())
    }

    fn inverse(&self, state: &Sheet) -> Result<RestoreCell, OpError> {
        Ok(RestoreCell {
            key: self.key.clone(),
            previous: state.cells.get(&self.key).copied(),
        })
    }

    fn validate(&self, _state: &Sheet, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

impl CrdtCompatible<Sheet> for SetCell {
    const IS_COMMUTATIVE: bool = true;
    const IS_ASSOCIATIVE: bool = true;
    const IS_IDEMPOTENT: bool = true;
}

/// Every ordering of `items`, by Heap's algorithm.
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    fn go<T: Clone>(k: usize, items: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        for i in 0..k {
            go(k - 1, items, out);
            let j = if k.is_multiple_of(2) { i } else { 0 };
            items.swap(j, k - 1);
        }
    }
    let mut out = Vec::new();
    go(items.len(), &mut items.to_vec(), &mut out);
    out
}

fn main() {
    // Two users edit the same sheet at the same time, each with their own clock.
    let (alice, bob) = (ActorId(1), ActorId(2));
    let (mut alice_clock, mut bob_clock) = (LamportClock::new(), LamportClock::new());
    let write = |clock: &mut LamportClock, actor, key: &str, value| SetCell {
        key: key.into(),
        value,
        stamp: clock.tick(actor).expect("clock has room"),
    };

    let writes = vec![
        write(&mut alice_clock, alice, "A1", 10),
        write(&mut bob_clock, bob, "A1", 20), // concurrent with Alice's write
        write(&mut alice_clock, alice, "B2", 7),
        write(&mut bob_clock, bob, "B2", 8),
    ];

    // The declared properties hold on these samples.
    let base = Sheet::default();
    assert!(check_commutative(&base, &writes[0], &writes[1]));
    assert!(check_idempotent(&base, &writes[0]));

    // Every delivery order, and every order with the writes delivered twice, converges.
    let mut finals = Vec::new();
    for order in permutations(&writes) {
        let mut sheet = Sheet::default();
        for op in order.iter().chain(order.iter()) {
            sheet.apply(op).unwrap();
        }
        finals.push(sheet);
    }
    assert!(finals.windows(2).all(|pair| pair[0] == pair[1]));
    println!(
        "{} delivery orders (each applied twice) all converge to:",
        finals.len()
    );
    for (key, (stamp, value)) in &finals[0].cells {
        println!(
            "  {key} = {value}  (lamport {}, {})",
            stamp.lamport, stamp.actor
        );
    }

    // Undo restores the cell, stamp included.
    let mut sheet = finals[0].clone();
    let edit = write(&mut bob_clock, bob, "A1", 99);
    let undo = edit.inverse(&sheet).unwrap();
    let before = sheet.clone();
    sheet.apply(&edit).unwrap();
    println!("bob edits A1 -> {}", sheet.cells["A1"].1);
    sheet.apply(&undo).unwrap();
    assert_eq!(sheet, before);
    println!("undo         -> {}", sheet.cells["A1"].1);
}
