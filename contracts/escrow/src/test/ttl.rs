//! Instance lifetime.
//!
//! Instance storage holds the admin, the tree root and the generation counter,
//! and it shares its lifetime with the contract code. Nothing extends it
//! automatically, so every entrypoint that a live escrow actually sees has to
//! push it back out. Deposits are the only user-facing one.

use soroban_sdk::testutils::storage::Instance as _;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::Address;

use super::Harness;
use crate::storage_types::{DAY_IN_LEDGERS, INSTANCE_BUMP_AMOUNT};

fn instance_ttl(h: &Harness) -> u32 {
    h.env
        .as_contract(&h.escrow, || h.env.storage().instance().get_ttl())
}

/// Move far enough forward that the instance drops under the bump threshold.
fn age_past_threshold(h: &Harness) {
    h.env.ledger().with_mut(|li| {
        li.sequence_number += DAY_IN_LEDGERS * 2;
    });
}

#[test]
fn a_fresh_instance_starts_at_the_full_lifetime() {
    let h = Harness::new();
    assert_eq!(instance_ttl(&h), INSTANCE_BUMP_AMOUNT);
}

/// An escrow can take deposits for a long time without a release or a config
/// change. If deposits did not extend the instance, the contract would fall out
/// of the live state while still holding funds.
#[test]
fn deposit_keeps_the_instance_alive() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    age_past_threshold(&h);
    let aged = instance_ttl(&h);
    assert!(aged < INSTANCE_BUMP_AMOUNT);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);
    client.deposit(&user, &h.mint, &400);

    assert_eq!(instance_ttl(&h), INSTANCE_BUMP_AMOUNT);
}

#[test]
fn operator_changes_keep_the_instance_alive() {
    let h = Harness::new();
    let operator = Address::generate(&h.env);

    age_past_threshold(&h);
    assert!(instance_ttl(&h) < INSTANCE_BUMP_AMOUNT);

    h.client().add_operator(&operator);
    assert_eq!(instance_ttl(&h), INSTANCE_BUMP_AMOUNT);
}

#[test]
fn mint_changes_keep_the_instance_alive() {
    let h = Harness::new();

    age_past_threshold(&h);
    assert!(instance_ttl(&h) < INSTANCE_BUMP_AMOUNT);

    h.client().allow_mint(&h.mint);
    assert_eq!(instance_ttl(&h), INSTANCE_BUMP_AMOUNT);
}
