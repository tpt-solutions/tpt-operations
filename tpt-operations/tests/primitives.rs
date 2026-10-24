//! The `ActorId` <-> `Principal` mapping at the `tpt-primitives` boundary.

#![cfg(feature = "primitives")]

use tpt_operations::{
    core::ActorId,
    primitives::{actor_of, principal_of},
};
use tpt_primitives::capability::Principal;

#[test]
fn every_actor_round_trips_through_its_principal() {
    for n in [0, 1, 7, 42, u64::MAX] {
        let actor = ActorId(n);
        assert_eq!(actor_of(&principal_of(actor).unwrap()).unwrap(), actor);
    }
}

#[test]
fn principal_label_is_actor_colon_number() {
    assert_eq!(principal_of(ActorId(7)).unwrap().label(), "actor:7");
}

#[test]
fn only_the_canonical_label_form_is_accepted() {
    for label in [
        "actor:007",
        "actor:+7",
        "actor:-1",
        "actor:",
        "actor:7x",
        "user:7",
    ] {
        let principal = Principal::new(label).unwrap();
        assert!(actor_of(&principal).is_err(), "accepted {label}");
    }
}

#[test]
fn out_of_range_numbers_are_rejected() {
    let principal = Principal::new("actor:18446744073709551616").unwrap();
    assert!(actor_of(&principal).is_err());
}
