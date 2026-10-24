//! A signed ledger: commit on the author's side, receive on a peer's side, and
//! the three ways a hostile entry gets refused.
//!
//! Run it with `cargo run -p tpt-operations --example signed_audit_log --features crypto`.
//!
//! Each entry is signed with the author's Ed25519 key and numbered with a
//! sequence number. The receiver is told which actor the sender is (here, from
//! the "connection") and holds that actor's public key. It refuses:
//!
//! - tampered entries (the signature no longer matches),
//! - entries that claim another actor (the key does not belong to them),
//! - replays (the sequence number is not above the last one accepted).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tpt_operations::{
    crypto::{
        sign_op,
        tpt::{Ed25519Signer, Ed25519Verifier, Sha256},
        SignedEntry,
    },
    pipeline::{commit, receive, ReplayGuard},
    prelude::*,
};

#[derive(Debug, Clone, PartialEq, Default)]
struct Ledger {
    balance: i64,
}

impl State for Ledger {}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Entry {
    Deposit(i64),
    Withdraw(i64),
}

impl Operation<Ledger> for Entry {
    type Inverse = Restore;

    fn apply(&self, state: &mut Ledger) -> Result<(), OpError> {
        match *self {
            Entry::Deposit(n) => state.balance += n,
            Entry::Withdraw(n) => state.balance -= n,
        }
        Ok(())
    }

    fn inverse(&self, state: &Ledger) -> Result<Restore, OpError> {
        Ok(Restore(state.balance))
    }

    fn validate(&self, state: &Ledger, _actor: &ActorId) -> Result<(), OpError> {
        match *self {
            Entry::Deposit(n) | Entry::Withdraw(n) if n <= 0 => {
                Err(OpError::Validation("amount must be positive".into()))
            }
            Entry::Withdraw(n) if n > state.balance => {
                Err(OpError::Bounds("insufficient funds".into()))
            }
            _ => Ok(()),
        }
    }
}

/// Puts the balance back. The inverse of any entry.
#[derive(Debug, Clone)]
struct Restore(i64);

impl Operation<Ledger> for Restore {
    type Inverse = Restore;

    fn apply(&self, state: &mut Ledger) -> Result<(), OpError> {
        state.balance = self.0;
        Ok(())
    }

    fn inverse(&self, state: &Ledger) -> Result<Restore, OpError> {
        Ok(Restore(state.balance))
    }

    fn validate(&self, _state: &Ledger, _actor: &ActorId) -> Result<(), OpError> {
        Ok(())
    }
}

/// The receiver's side: which key belongs to which actor, and what it has seen.
struct Peer {
    ledger: Ledger,
    keys: BTreeMap<ActorId, Ed25519Verifier>,
    guard: ReplayGuard,
}

impl Peer {
    /// `sender` is who the transport says this came from.
    fn accept(&mut self, sender: ActorId, entry: &SignedEntry) -> Result<(), OpError> {
        let key = self
            .keys
            .get(&sender)
            .ok_or_else(|| OpError::Capability(format!("no key for {sender}")))?;
        receive::<_, Entry, _, _>(
            &mut self.ledger,
            entry,
            sender,
            &mut self.guard,
            &Sha256,
            key,
        )
    }
}

fn main() -> Result<(), OpError> {
    let (alice, mallory) = (ActorId(1), ActorId(666));
    let alice_key = Ed25519Signer::from_seed([1; 32]);
    let mallory_key = Ed25519Signer::from_seed([2; 32]);

    let mut peer = Peer {
        ledger: Ledger::default(),
        keys: BTreeMap::from([
            (alice, Ed25519Verifier::from_bytes(&alice_key.public_key())?),
            (
                mallory,
                Ed25519Verifier::from_bytes(&mallory_key.public_key())?,
            ),
        ]),
        guard: ReplayGuard::new(),
    };

    // Alice commits locally, then sends the signed entries.
    let mut alice_ledger = Ledger::default();
    let deposit = commit(
        &mut alice_ledger,
        &Entry::Deposit(100),
        alice,
        1,
        &Sha256,
        &alice_key,
    )?;
    let withdraw = commit(
        &mut alice_ledger,
        &Entry::Withdraw(30),
        alice,
        2,
        &Sha256,
        &alice_key,
    )?;
    println!("alice's ledger:   {}", alice_ledger.balance);

    peer.accept(alice, &deposit.entry)?;
    peer.accept(alice, &withdraw.entry)?;
    println!("peer after sync:  {}", peer.ledger.balance);

    // Alice can undo locally with the inverse that commit returned.
    alice_ledger.apply(&withdraw.inverse)?;
    println!("alice undid it:   {}", alice_ledger.balance);

    // 1. Tampering: change the sequence number of a genuine entry. It is signed too.
    let mut tampered = withdraw.entry.clone();
    tampered.seq += 1;
    println!(
        "tampered entry:    {}",
        describe(peer.accept(alice, &tampered))
    );

    // 2. Impersonation: Mallory signs an entry that says it is from Alice.
    let forged = sign_op(alice, 3, &Entry::Withdraw(70), &Sha256, &mallory_key)?;
    println!(
        "forged, from alice: {}",
        describe(peer.accept(alice, &forged))
    );
    println!(
        "forged, from mallory: {}",
        describe(peer.accept(mallory, &forged))
    );

    // 3. Replay: send Alice's real withdrawal again.
    println!(
        "replayed entry:   {}",
        describe(peer.accept(alice, &withdraw.entry))
    );

    assert_eq!(
        peer.ledger.balance, 70,
        "none of the attacks changed the ledger"
    );
    Ok(())
}

fn describe(result: Result<(), OpError>) -> String {
    match result {
        Ok(()) => "ACCEPTED".into(),
        Err(err) => format!("refused ({err})"),
    }
}
