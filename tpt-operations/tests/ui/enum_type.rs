use tpt_operations::Operation;
use tpt_operations::core::State;

#[derive(Debug, Clone)]
pub struct Counter { pub value: i64 }
impl State for Counter {}

#[derive(Operation)]
#[tpt(state = Counter)]
pub enum Either {
    A,
    B,
}

fn main() {}
