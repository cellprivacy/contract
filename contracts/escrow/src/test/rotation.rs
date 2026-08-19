//! `reset_smt_root`: who may rotate, replay protection, and the effect on the
//! installed generation.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use super::Harness;
use crate::smt;

/// An escrow with one registered operator.
fn with_operator() -> (Harness, Address) {
    let h = Harness::new();
    let operator = Address::generate(&h.env);
    h.client().add_operator(&operator);
    (h, operator)
}

#[test]
fn rotation_starts_a_new_tree_and_bumps_the_generation() {
    let (h, operator) = with_operator();
    let client = h.client();
    let empty = smt::empty_tree_root(&h.env);

    client.reset_smt_root(&operator, &0);

    assert_eq!(client.tree_index(), 1);
    assert_eq!(client.root(), empty);

    client.reset_smt_root(&operator, &1);
    assert_eq!(client.tree_index(), 2);
}

#[test]
#[should_panic]
fn rotation_requires_operator_auth() {
    let (h, operator) = with_operator();
    h.client().mock_auths(&[]).reset_smt_root(&operator, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn rotation_is_rejected_for_an_unregistered_operator() {
    let (h, _operator) = with_operator();
    let stranger = Address::generate(&h.env);

    h.client().reset_smt_root(&stranger, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn rotation_is_rejected_after_the_operator_is_removed() {
    let (h, operator) = with_operator();
    let client = h.client();
    client.remove_operator(&operator);

    client.reset_smt_root(&operator, &0);
}

/// The admin configures operators but does not rotate.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn the_admin_cannot_rotate_unless_also_an_operator() {
    let (h, _operator) = with_operator();
    h.client().reset_smt_root(&h.admin, &0);
}

/// Rotation is not idempotent: replaying it would advance the counter a second
/// time and strand a whole generation of nonces.
#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn a_replayed_rotation_is_rejected() {
    let (h, operator) = with_operator();
    let client = h.client();

    client.reset_smt_root(&operator, &0);
    client.reset_smt_root(&operator, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn rotation_is_rejected_against_a_future_index() {
    let (h, operator) = with_operator();
    h.client().reset_smt_root(&operator, &5);
}
