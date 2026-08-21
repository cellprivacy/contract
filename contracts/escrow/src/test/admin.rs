//! Initialization, admin handover, operator management, mint gating, and tree
//! rotation: the admin and operator control surface.

use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, IntoVal};

use super::Harness;
use crate::smt;
use crate::{AdminChanged, MintSet, OperatorSet};
use soroban_sdk::events::Event as _;
use soroban_sdk::testutils::Events as _;

#[test]
fn the_constructor_sets_admin_and_an_empty_tree() {
    let h = Harness::new();
    let client = h.client();

    assert_eq!(client.admin(), h.admin);
    assert_eq!(client.root(), smt::empty_tree_root(&h.env));
    assert_eq!(client.tree_index(), 0);
    assert_eq!(client.total_locked(&h.mint), 0);
}

/// The instance is fully configured the moment it exists. There is no window
/// in which it sits on chain unclaimed, and no second-call path to guard: the
/// host calls `__constructor` during deployment and reserved names cannot be
/// invoked afterwards.
#[test]
fn the_instance_is_usable_immediately_after_deployment() {
    let h = Harness::new();
    let client = h.client();
    let operator = Address::generate(&h.env);

    client.add_operator(&operator);

    assert_eq!(client.admin(), h.admin);
    assert!(client.is_operator(&operator));
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

/// Handover needs both signatures. A one-sided transfer to a mistyped address
/// would strand admin rights, and with them the ability to upgrade.
#[test]
#[should_panic]
fn handover_requires_the_incoming_admin_auth() {
    let h = Harness::new();
    let next = Address::generate(&h.env);

    h.client()
        .mock_auths(&[MockAuth {
            address: &h.admin,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "set_new_admin",
                args: (next.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .set_new_admin(&next);
}

#[test]
#[should_panic]
fn handover_requires_the_outgoing_admin_auth() {
    let h = Harness::new();
    let next = Address::generate(&h.env);

    h.client()
        .mock_auths(&[MockAuth {
            address: &next,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "set_new_admin",
                args: (next.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .set_new_admin(&next);
}

/// The control surface has to be visible to off-chain monitoring. Freezing an
/// asset in particular is an incident lever, and one that emits nothing cannot
/// be alerted on.
#[test]
fn the_control_surface_publishes_events() {
    let h = Harness::new();
    let client = h.client();
    let operator = Address::generate(&h.env);
    let next = Address::generate(&h.env);

    client.add_operator(&operator);
    assert_last_event(
        &h,
        OperatorSet {
            operator: operator.clone(),
            enabled: true,
            ledger: h.env.ledger().sequence(),
        }
        .to_xdr(&h.env, &h.escrow),
    );

    client.remove_operator(&operator);
    assert_last_event(
        &h,
        OperatorSet {
            operator,
            enabled: false,
            ledger: h.env.ledger().sequence(),
        }
        .to_xdr(&h.env, &h.escrow),
    );

    client.allow_mint(&h.mint);
    assert_last_event(
        &h,
        MintSet {
            mint: h.mint.clone(),
            allowed: true,
            ledger: h.env.ledger().sequence(),
        }
        .to_xdr(&h.env, &h.escrow),
    );

    client.block_mint(&h.mint);
    assert_last_event(
        &h,
        MintSet {
            mint: h.mint.clone(),
            allowed: false,
            ledger: h.env.ledger().sequence(),
        }
        .to_xdr(&h.env, &h.escrow),
    );

    client.set_new_admin(&next);
    assert_last_event(
        &h,
        AdminChanged {
            previous: h.admin.clone(),
            next,
            ledger: h.env.ledger().sequence(),
        }
        .to_xdr(&h.env, &h.escrow),
    );
}

fn assert_last_event(h: &Harness, expected: soroban_sdk::xdr::ContractEvent) {
    let events = h.env.events().all();
    let published = events.filter_by_contract(&h.escrow);
    assert_eq!(published.events(), &[expected][..]);
}
