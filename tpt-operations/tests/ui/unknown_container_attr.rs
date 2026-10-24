use tpt_operations::Operation;

#[derive(Operation)]
#[tpt(stat = Counter)]
pub struct Typo {
    pub value: i64,
}

fn main() {}
