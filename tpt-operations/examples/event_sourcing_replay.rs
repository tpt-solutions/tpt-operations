//! Event sourcing: write an operation log, then rebuild the state from it.
//!
//! Run it with `cargo run -p tpt-operations --example event_sourcing_replay --features serde`.
//!
//! The log is just bytes, one versioned entry per operation. `ReplayEngine`
//! reads it back and applies each entry in order. With `FailFast` it stops at
//! the first entry that cannot apply. With `SkipInvalid` it records the
//! failure in the trace and carries on, leaving the state as it was before
//! that entry. Either way the same log gives the same state and the same
//! trace.

use serde::{Deserialize, Serialize};
use tpt_operations::{
    history::{replay::encode_entry, ReplayEngine, ReplayPolicy},
    prelude::*,
};

#[derive(Debug, Clone, PartialEq, Default)]
struct Account {
    balance: i64,
}

impl State for Account {}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Tx {
    Deposit(i64),
    Withdraw(i64),
}

impl Operation<Account> for Tx {
    type Inverse = SetBalance;

    fn apply(&self, state: &mut Account) -> Result<(), OpError> {
        self.validate(state, &ActorId(0))?;
        match *self {
            Tx::Deposit(n) => state.balance += n,
            Tx::Withdraw(n) => state.balance -= n,
        }
        Ok(())
    }

    fn inverse(&self, state: &Account) -> Result<SetBalance, OpError> {
        Ok(SetBalance(state.balance)) // the old balance, from the pre-state
    }

    fn validate(&self, state: &Account, _actor: &ActorId) -> Result<(), OpError> {
        match *self {
            Tx::Withdraw(n) if n > state.balance => Err(OpError::Bounds(format!(
                "withdraw {n} from {}",
                state.balance
            ))),
            _ => Ok(()),
        }
    }
}

/// Puts the balance back. The inverse of any transaction.
#[derive(Debug, Clone)]
struct SetBalance(i64);

impl Operation<Account> for SetBalance {
    type Inverse = SetBalance;

    fn apply(&self, state: &mut Account) -> Result<(), OpError> {
        state.balance = self.0;
        Ok(())
    }

    fn inverse(&self, state: &Account) -> Result<SetBalance, OpError> {
        Ok(SetBalance(state.balance))
    }

    fn validate(&self, _state: &Account, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

fn main() {
    // Write the log. Each entry records who did it.
    let history = [
        (ActorId(1), Tx::Deposit(100)),
        (ActorId(2), Tx::Withdraw(30)),
        (ActorId(2), Tx::Withdraw(500)), // would overdraw
        (ActorId(1), Tx::Deposit(5)),
    ];
    let mut log = Vec::new();
    for (actor, tx) in &history {
        encode_entry(*actor, tx, &mut log).expect("encodes");
    }
    println!("log: {} bytes, {} entries\n", log.len(), history.len());

    // Strict replay stops at the bad entry.
    match ReplayEngine::new(ReplayPolicy::FailFast).replay::<Account, Tx>(Account::default(), &log)
    {
        Ok(_) => unreachable!("entry 2 overdraws"),
        Err(err) => println!("FailFast stopped: {err}\n"),
    }

    // Lenient replay skips it and says so in the trace.
    let (account, trace) = ReplayEngine::new(ReplayPolicy::SkipInvalid)
        .replay::<Account, Tx>(Account::default(), &log)
        .expect("only decode errors stop SkipInvalid");
    println!("SkipInvalid trace:\n{trace}");
    println!("final balance: {}", account.balance);
    assert_eq!(account.balance, 75);

    // Replay is deterministic.
    let (again, trace_again) = ReplayEngine::new(ReplayPolicy::SkipInvalid)
        .replay::<Account, Tx>(Account::default(), &log)
        .unwrap();
    assert_eq!((again, trace_again), (account, trace));
}
