//! Initialization, admin handover, operator management, mint gating, and tree
//! rotation: the admin and operator control surface.

use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, IntoVal};

use super::Harness;
use crate::smt;

#[test]
fn initialize_sets_admin_and_an_empty_tree() {
    let h = Harness::new();
    let client = h.client();

    assert_eq!(client.admin(), h.admin);
    assert_eq!(client.root(), smt::empty_tree_root(&h.env));
    assert_eq!(client.tree_index(), 0);
    assert_eq!(client.total_locked(&h.mint), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn initialize_twice_is_rejected() {
    let h = Harness::new();
    h.client().initialize(&h.admin);
}

#[test]
fn set_new_admin_hands_over_control() {
    let h = Harness::new();
    let client = h.client();
    let next = Address::generate(&h.env);

    client.set_new_admin(&next);

    assert_eq!(client.admin(), next);
}

#[test]
#[should_panic]
fn set_new_admin_requires_admin_auth() {
    let h = Harness::new();
    let next = Address::generate(&h.env);

    h.client().mock_auths(&[]).set_new_admin(&next);
}

#[test]
fn admin_can_add_and_remove_operators() {
    let h = Harness::new();
    let client = h.client();
    let operator = Address::generate(&h.env);

    assert!(!client.is_operator(&operator));

    client.add_operator(&operator);
    assert!(client.is_operator(&operator));

    client.remove_operator(&operator);
    assert!(!client.is_operator(&operator));
}

#[test]
#[should_panic]
fn add_operator_requires_admin_auth() {
    let h = Harness::new();
    let operator = Address::generate(&h.env);

    h.client().mock_auths(&[]).add_operator(&operator);
}

/// Authorization is bound to the stored admin, not merely to *some* signature:
/// a fully-formed auth entry from another address must not open the gate.
#[test]
#[should_panic]
fn add_operator_rejects_a_non_admin_signer() {
    let h = Harness::new();
    let intruder = Address::generate(&h.env);
    let operator = Address::generate(&h.env);

    h.client()
        .mock_auths(&[MockAuth {
            address: &intruder,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "add_operator",
                args: (operator.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .add_operator(&operator);
}

#[test]
#[should_panic]
fn remove_operator_requires_admin_auth() {
    let h = Harness::new();
    let operator = Address::generate(&h.env);
    h.client().add_operator(&operator);

    h.client().mock_auths(&[]).remove_operator(&operator);
}

#[test]
fn admin_can_allow_and_block_a_mint() {
    let h = Harness::new();
    let client = h.client();

    assert!(!client.is_allowed_mint(&h.mint));

    client.allow_mint(&h.mint);
    assert!(client.is_allowed_mint(&h.mint));

    client.block_mint(&h.mint);
    assert!(!client.is_allowed_mint(&h.mint));
}

#[test]
fn mint_permissions_are_independent_per_asset() {
    let h = Harness::new();
    let client = h.client();
    let other = h.other_mint();

    client.allow_mint(&h.mint);

    assert!(client.is_allowed_mint(&h.mint));
    assert!(!client.is_allowed_mint(&other));
}

#[test]
#[should_panic]
fn allow_mint_requires_admin_auth() {
    let h = Harness::new();
    h.client().mock_auths(&[]).allow_mint(&h.mint);
}

#[test]
#[should_panic]
fn block_mint_requires_admin_auth() {
    let h = Harness::new();
    h.client().allow_mint(&h.mint);

    h.client().mock_auths(&[]).block_mint(&h.mint);
}

/// After handover the new admin holds the privilege, verified by having the
/// new admin sign a call that only the admin may make.
#[test]
fn handover_moves_privilege_to_the_new_admin() {
    let h = Harness::new();
    let client = h.client();
    let next = Address::generate(&h.env);
    let operator = Address::generate(&h.env);

    client.set_new_admin(&next);

    client
        .mock_auths(&[MockAuth {
            address: &next,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "add_operator",
                args: (operator.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .add_operator(&operator);

    assert!(client.is_operator(&operator));
}

#[test]
#[should_panic]
fn the_previous_admin_loses_privilege_after_handover() {
    let h = Harness::new();
    let client = h.client();
    let next = Address::generate(&h.env);
    let operator = Address::generate(&h.env);

    client.set_new_admin(&next);

    client
        .mock_auths(&[MockAuth {
            address: &h.admin,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "add_operator",
                args: (operator.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .add_operator(&operator);
}
