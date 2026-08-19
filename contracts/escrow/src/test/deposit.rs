//! Deposit: custody transfer, per-mint accounting, gating, and the event the
//! off-chain indexer consumes.

use soroban_sdk::events::Event as _;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::Address;

use super::Harness;
use crate::Deposit;

#[test]
fn deposit_moves_custody_and_tracks_the_locked_total() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    client.deposit(&user, &h.mint, &400);

    assert_eq!(h.balance_of(&user), 600);
    assert_eq!(h.balance_of(&h.escrow), 400);
    assert_eq!(client.total_locked(&h.mint), 400);
}

#[test]
fn deposits_accumulate() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    client.deposit(&user, &h.mint, &400);
    client.deposit(&user, &h.mint, &250);

    assert_eq!(client.total_locked(&h.mint), 650);
    assert_eq!(h.balance_of(&h.escrow), 650);
}

/// Custody is accounted per asset. Locking asset A must never make asset B's
/// balance releasable.
#[test]
fn locked_totals_are_tracked_per_mint() {
    let h = Harness::new();
    let client = h.client();
    let other = h.other_mint();
    client.allow_mint(&h.mint);
    client.allow_mint(&other);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);
    client.deposit(&user, &h.mint, &400);

    assert_eq!(client.total_locked(&h.mint), 400);
    assert_eq!(client.total_locked(&other), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn deposit_is_rejected_when_the_mint_is_not_allowed() {
    let h = Harness::new();
    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    h.client().deposit(&user, &h.mint, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn deposit_is_rejected_after_the_mint_is_blocked() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);
    client.deposit(&user, &h.mint, &100);

    client.block_mint(&h.mint);
    client.deposit(&user, &h.mint, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn deposit_rejects_a_zero_amount() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    client.deposit(&user, &h.mint, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn deposit_rejects_a_negative_amount() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    client.deposit(&user, &h.mint, &-100);
}

#[test]
#[should_panic]
fn deposit_requires_the_depositor_auth() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);

    client.mock_auths(&[]).deposit(&user, &h.mint, &100);
}

/// The indexer credits channel balance off this event, so its shape is part of
/// the contract's interface.
#[test]
fn deposit_publishes_the_indexer_event() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint);

    let user = Address::generate(&h.env);
    h.fund(&user, 1_000);
    client.deposit(&user, &h.mint, &400);

    let expected = Deposit {
        from: user.clone(),
        mint: h.mint.clone(),
        amount: 400,
        total_locked: 400,
        ledger: h.env.ledger().sequence(),
    };

    let events = h.env.events().all();
    let published = events.filter_by_contract(&h.escrow);
    assert_eq!(
        published.events(),
        &[expected.to_xdr(&h.env, &h.escrow)][..]
    );
}
