//! `sweep`: recovering balance the contract holds beyond recorded custody.
//!
//! The whole point is the bound. A sweep moves `balance - TotalLocked` and
//! never touches `TotalLocked`, so backed custody is unreachable by
//! construction rather than by a check that could be got wrong.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::Address;

use super::Harness;

fn balance_of(h: &Harness, mint: &Address, who: &Address) -> i128 {
    TokenClient::new(&h.env, mint).balance(who)
}

/// Somebody transfers the asset straight to the contract address, bypassing
/// `deposit`. Nothing records it, and before `sweep` nothing could move it.
fn strand(h: &Harness, mint: &Address, amount: i128) {
    let stranger = Address::generate(&h.env);
    StellarAssetClient::new(&h.env, mint).mint(&stranger, &amount);
    TokenClient::new(&h.env, mint).transfer(&stranger, &h.escrow, &amount);
}

#[test]
fn sweep_recovers_exactly_the_surplus() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &1_000);
    strand(&h, &h.mint, 250);

    let to = Address::generate(&h.env);
    let recovered = client.sweep(&h.mint, &to);

    assert_eq!(recovered, 250);
    assert_eq!(balance_of(&h, &h.mint, &to), 250);
    assert_eq!(client.total_locked(&h.mint), 1_000);
    assert_eq!(balance_of(&h, &h.mint, &h.escrow), 1_000);
}

/// After a sweep the books and the balance agree exactly, which is the
/// invariant the design note claims for the pair.
#[test]
fn after_a_sweep_the_balance_equals_the_record() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &400);
    strand(&h, &h.mint, 77);

    client.sweep(&h.mint, &Address::generate(&h.env));

    assert_eq!(
        balance_of(&h, &h.mint, &h.escrow),
        client.total_locked(&h.mint)
    );
}

/// Backed custody is out of reach: with no surplus there is nothing to take,
/// however much the contract holds.
#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn sweep_cannot_reach_backed_custody() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &1_000);

    client.sweep(&h.mint, &Address::generate(&h.env));
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn sweep_of_an_untouched_asset_is_rejected() {
    let h = Harness::new();
    h.client().sweep(&h.mint, &Address::generate(&h.env));
}

/// Strays usually arrive in assets nobody opened, so the sweep deliberately
/// does not require the asset to be allowed.
#[test]
fn sweep_works_for_an_asset_that_was_never_allowed() {
    let h = Harness::new();
    let other = h.other_mint();
    strand(&h, &other, 900);

    let to = Address::generate(&h.env);
    assert!(!h.client().is_allowed_mint(&other));
    assert_eq!(h.client().sweep(&other, &to), 900);
    assert_eq!(balance_of(&h, &other, &to), 900);
}

#[test]
#[should_panic]
fn sweep_requires_admin_auth() {
    let h = Harness::new();
    strand(&h, &h.mint, 100);

    h.client()
        .mock_auths(&[])
        .sweep(&h.mint, &Address::generate(&h.env));
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn sweep_rejects_the_escrow_as_its_own_recipient() {
    let h = Harness::new();
    strand(&h, &h.mint, 100);
    let escrow = h.escrow.clone();

    h.client().sweep(&h.mint, &escrow);
}

/// An operator has no say in this.
#[test]
#[should_panic]
fn an_operator_cannot_sweep() {
    let h = Harness::new();
    let operator = Address::generate(&h.env);
    h.client().add_operator(&operator);
    strand(&h, &h.mint, 100);

    use soroban_sdk::testutils::{MockAuth, MockAuthInvoke};
    use soroban_sdk::IntoVal;
    let to = Address::generate(&h.env);
    h.client()
        .mock_auths(&[MockAuth {
            address: &operator,
            invoke: &MockAuthInvoke {
                contract: &h.escrow,
                fn_name: "sweep",
                args: (h.mint.clone(), to.clone()).into_val(&h.env),
                sub_invokes: &[],
            },
        }])
        .sweep(&h.mint, &to);
}
