//! `upgrade`: who may replace the executable, and what survives it.

use soroban_sdk::testutils::{Address as _, BytesN as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, BytesN, IntoVal};

use super::Harness;

/// A hash that was never uploaded. Good enough for the authorization tests,
/// which all fail before the executable would be swapped in. The successful
/// path needs a real uploaded wasm, so it is covered on testnet instead; see
/// `docs/deploy.md`.
fn some_hash(h: &Harness) -> BytesN<32> {
    BytesN::random(&h.env)
}

#[test]
#[should_panic]
fn upgrade_requires_admin_auth() {
    let h = Harness::new();
    let hash = some_hash(&h);

    h.client().mock_auths(&[]).upgrade(&hash);
}

#[test]
#[should_panic]
fn upgrade_rejects_a_non_admin_signer() {
    let h = Harness::new();
    let intruder = Address::generate(&h.env);
    let hash = some_hash(&h);

    h.client()
        .mock_auths(&[MockAuth {
            address: &intruder,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "upgrade",
                args: (hash.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .upgrade(&hash);
}

/// An operator has no say in what code runs.
#[test]
#[should_panic]
fn an_operator_cannot_upgrade() {
    let h = Harness::new();
    let operator = Address::generate(&h.env);
    h.client().add_operator(&operator);
    let hash = some_hash(&h);

    h.client()
        .mock_auths(&[MockAuth {
            address: &operator,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "upgrade",
                args: (hash.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .upgrade(&hash);
}

#[test]
#[should_panic]
fn the_previous_admin_cannot_upgrade_after_handover() {
    let h = Harness::new();
    let next = Address::generate(&h.env);
    h.client().set_new_admin(&next);
    let hash = some_hash(&h);

    h.client()
        .mock_auths(&[MockAuth {
            address: &h.admin,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "upgrade",
                args: (hash.clone(),).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .upgrade(&hash);
}
