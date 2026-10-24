//! A text document with undo, where "replace all" is a single undo step.
//!
//! Run it with `cargo run -p tpt-operations --example text_doc_undo`.
//!
//! There is one operation, `Replace`: it swaps a byte range for new text.
//! Inserting is a replace of zero bytes, and deleting is a replace with empty
//! text. The inverse is a `Replace` that puts back the exact bytes the
//! operation removed, read from the pre-state. A batch groups several edits
//! into one undo step.

use tpt_operations::prelude::*;

#[derive(Debug, Clone, PartialEq, Default)]
struct Doc {
    text: String,
}

impl State for Doc {}

#[derive(Debug, Clone)]
struct Replace {
    at: usize,
    len: usize,
    text: String,
}

impl Replace {
    fn insert(at: usize, text: &str) -> Self {
        Self {
            at,
            len: 0,
            text: text.into(),
        }
    }

    fn delete(at: usize, len: usize) -> Self {
        Self {
            at,
            len,
            text: String::new(),
        }
    }
}

impl Operation<Doc> for Replace {
    type Inverse = Replace;

    fn apply(&self, state: &mut Doc) -> Result<(), OpError> {
        self.validate(state, &ActorId(0))?;
        state
            .text
            .replace_range(self.at..self.at + self.len, &self.text);
        Ok(())
    }

    fn inverse(&self, state: &Doc) -> Result<Replace, OpError> {
        self.validate(state, &ActorId(0))?;
        Ok(Replace {
            at: self.at,
            len: self.text.len(),
            // The bytes about to be removed, taken from the pre-state.
            text: state.text[self.at..self.at + self.len].to_string(),
        })
    }

    fn validate(&self, state: &Doc, _actor: &ActorId) -> Result<(), OpError> {
        let end = self
            .at
            .checked_add(self.len)
            .filter(|&end| end <= state.text.len())
            .ok_or_else(|| {
                OpError::Bounds(format!("range {}+{} is past the end", self.at, self.len))
            })?;
        if state.text.is_char_boundary(self.at) && state.text.is_char_boundary(end) {
            Ok(())
        } else {
            Err(OpError::Validation("range splits a character".into()))
        }
    }
}

impl SizeEstimate for Replace {
    fn estimated_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.text.len()
    }
}

fn main() -> Result<(), OpError> {
    let me = ActorId(1);
    let mut doc = Doc::default();
    let mut history = HistoryStack::new();

    history.execute(
        &mut doc,
        &tpt_batch![Replace::insert(0, "the cat sat on the mat")],
        me,
    )?;
    println!("typed:    {:?}", doc.text);

    // "Replace all": three edits, one undo step. Edits go right to left so the
    // earlier offsets stay valid.
    let replace_all = tpt_batch![
        Replace {
            at: 19,
            len: 3,
            text: "rug".into()
        },
        Replace {
            at: 8,
            len: 3,
            text: "dog".into()
        },
        Replace {
            at: 4,
            len: 3,
            text: "owl".into()
        },
    ];
    history.execute(&mut doc, &replace_all, me)?;
    println!("replaced: {:?}", doc.text);

    // A bad edit fails as a whole and leaves the document alone.
    let broken = tpt_batch![Replace::delete(0, 4), Replace::delete(500, 1)];
    let err = history.execute(&mut doc, &broken, me).unwrap_err();
    println!("refused:  {err} (document unchanged: {:?})", doc.text);

    history.undo(&mut doc, me)?;
    println!("undo:     {:?}", doc.text);
    history.undo(&mut doc, me)?;
    println!("undo:     {:?}", doc.text);
    assert!(doc.text.is_empty());

    history.redo(&mut doc, me)?;
    history.redo(&mut doc, me)?;
    assert_eq!(doc.text, "the owl dog on the rug");
    println!("redo x2:  {:?}", doc.text);
    Ok(())
}
