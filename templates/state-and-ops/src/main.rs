//! A notes list with undo and redo, built on `tpt-operations`.
//!
//! Three things to change to make it yours:
//!
//! 1. `Notes` is the state: the data your operations change.
//! 2. `Edit` is the operation: every way the state can change.
//! 3. `main` and the tests: the app, and the checks that keep undo honest.
//!
//! The rule to keep: `inverse` reads old values from the state it is given, and
//! never works them out from the operation's inputs.

use tpt_operations::prelude::*;

/// The state: everything an operation can change.
#[derive(Debug, Clone, PartialEq, Default)]
struct Notes {
    lines: Vec<String>,
}

impl State for Notes {}

/// Every way `Notes` can change.
#[derive(Debug, Clone, PartialEq)]
enum Edit {
    /// Put `text` at position `at`, moving later lines down.
    Insert { at: usize, text: String },
    /// Take out the line at `at`.
    Remove { at: usize },
}

impl Operation<Notes> for Edit {
    type Inverse = Edit;

    fn apply(&self, state: &mut Notes) -> Result<(), OpError> {
        self.validate(state, &ActorId(0))?;
        match self {
            Edit::Insert { at, text } => state.lines.insert(*at, text.clone()),
            Edit::Remove { at } => {
                state.lines.remove(*at);
            }
        }
        Ok(())
    }

    /// Called before `apply`, with the state as it is now.
    fn inverse(&self, state: &Notes) -> Result<Edit, OpError> {
        self.validate(state, &ActorId(0))?;
        Ok(match self {
            Edit::Insert { at, .. } => Edit::Remove { at: *at },
            // The removed text comes from the state, not from the operation.
            Edit::Remove { at } => Edit::Insert {
                at: *at,
                text: state.lines[*at].clone(),
            },
        })
    }

    /// Say no here. Anything that would panic or break an invariant goes in this method.
    fn validate(&self, state: &Notes, _actor: &ActorId) -> Result<(), OpError> {
        match self {
            Edit::Insert { at, .. } if *at > state.lines.len() => Err(OpError::Bounds(format!(
                "cannot insert at {at}, there are {} lines",
                state.lines.len()
            ))),
            Edit::Remove { at } if *at >= state.lines.len() => {
                Err(OpError::NotFound(format!("no line {at}")))
            }
            _ => Ok(()),
        }
    }
}

/// Lets the history cap its memory. Count the heap data the operation carries.
impl SizeEstimate for Edit {
    fn estimated_size(&self) -> usize {
        let text = match self {
            Edit::Insert { text, .. } => text.len(),
            Edit::Remove { .. } => 0,
        };
        std::mem::size_of::<Self>() + text
    }
}

fn main() -> Result<(), OpError> {
    let me = ActorId(1);
    let mut notes = Notes::default();
    let mut history = HistoryStack::new();

    let steps = [
        tpt_batch![Edit::Insert {
            at: 0,
            text: "buy milk".into()
        }],
        tpt_batch![Edit::Insert {
            at: 1,
            text: "call Sam".into()
        }],
        // Two edits, one undo step. If either fails, neither is applied.
        tpt_batch![
            Edit::Remove { at: 0 },
            Edit::Insert {
                at: 0,
                text: "buy oat milk".into()
            }
        ],
    ];
    for step in &steps {
        history.execute(&mut notes, step, &me)?;
        println!("{:?}", notes.lines);
    }

    history.undo(&mut notes, &me)?;
    println!("undo -> {:?}", notes.lines);
    history.redo(&mut notes, &me)?;
    println!("redo -> {:?}", notes.lines);

    // A bad edit is refused and changes nothing.
    let refused = history.execute(&mut notes, &tpt_batch![Edit::Remove { at: 9 }], &me);
    println!("remove 9 -> {}", refused.unwrap_err());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_operations::testkit::{assert_property, check_reversible, shrink_none};

    fn random_notes(rng: &mut tpt_operations::testkit::Rng) -> Notes {
        Notes {
            lines: (0..rng.below(5)).map(|i| format!("line {i}")).collect(),
        }
    }

    fn random_edit(rng: &mut tpt_operations::testkit::Rng) -> Edit {
        if rng.below(2) == 0 {
            Edit::Insert {
                at: rng.below(7),
                text: format!("new {}", rng.below(100)),
            }
        } else {
            Edit::Remove { at: rng.below(7) }
        }
    }

    /// Applying any edit and then its inverse gives back the same notes.
    #[test]
    fn every_edit_is_reversible() {
        assert_property(
            "edits are reversible",
            0xC0DE,
            10_000,
            |rng| (random_notes(rng), random_edit(rng)),
            shrink_none,
            |(notes, edit)| check_reversible(notes, edit),
        );
    }

    #[test]
    fn a_failed_batch_changes_nothing() {
        let mut notes = Notes::default();
        let mut history = HistoryStack::new();
        let batch = tpt_batch![
            Edit::Insert {
                at: 0,
                text: "kept?".into()
            },
            Edit::Remove { at: 5 },
        ];
        assert!(history.execute(&mut notes, &batch, &ActorId(1)).is_err());
        assert_eq!(notes, Notes::default());
        assert!(!history.can_undo());
    }
}
