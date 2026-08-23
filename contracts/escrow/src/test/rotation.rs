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

/// Revoking an operator removes the entry rather than writing `false`. Readers
/// answer the same either way, but a removed key stops costing rent.
#[test]
fn revoking_an_operator_removes_the_storage_entry() {
    use crate::storage_types::DataKey;
    let (h, operator) = with_operator();

    let key = DataKey::Operator(operator.clone());
    let present = |k: &DataKey| {
        h.env
            .as_contract(&h.escrow, || h.env.storage().persistent().has(k))
    };

    assert!(present(&key));
    h.client().remove_operator(&operator);
    assert!(!present(&key));
    assert!(!h.client().is_operator(&operator));
}

#[test]
fn blocking_a_mint_removes_the_storage_entry() {
    use crate::storage_types::DataKey;
    let (h, _operator) = with_operator();
    h.client().allow_mint(&h.mint, &0);

    let key = DataKey::AllowedMint(h.mint.clone());
    let present = |k: &DataKey| {
        h.env
            .as_contract(&h.escrow, || h.env.storage().persistent().has(k))
    };

    assert!(present(&key));
    h.client().block_mint(&h.mint);
    assert!(!present(&key));
    assert!(!h.client().is_allowed_mint(&h.mint));
}

/// The constructor stamps the storage-layout revision so a future upgrade can
/// tell what it is migrating from.
#[test]
fn the_constructor_stamps_the_storage_version() {
    let (h, _operator) = with_operator();
    assert_eq!(h.client().version(), crate::storage_types::STORAGE_VERSION);
}
