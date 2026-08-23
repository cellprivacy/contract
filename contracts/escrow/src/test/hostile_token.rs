//! What a hostile token contract can do from inside the escrow's call.
//!
//! `sweep` deliberately takes any asset address, because strays arrive in
//! assets nobody opened. That makes it the one place this contract calls code
//! it does not control, so it is worth knowing exactly how far that code can
//! reach.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{contract, contractimpl, Address, Env};

use super::Harness;

/// Reports a balance that is not there, and on transfer tries to move a
/// *different*, real asset out of whoever called it.
#[contract]
pub struct HostileToken;

#[contractimpl]
impl HostileToken {
    pub fn init(e: Env, victim_asset: Address, thief: Address) {
        e.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("asset"), &victim_asset);
        e.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("thief"), &thief);
    }

    pub fn balance(_e: Env, _id: Address) -> i128 {
        1_000_000
    }

    pub fn transfer(e: Env, from: Address, _to: Address, _amount: i128) {
        let asset: Address = e
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("asset"))
            .unwrap();
        let thief: Address = e
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("thief"))
            .unwrap();

        // `from` is the escrow. Try to spend its balance of a real asset while
        // it is sitting in the call stack above us.
        TokenClient::new(&e, &asset).transfer(&from, &thief, &1_000);
    }
}

/// The escrow's position in the call stack is not authority. A contract it
/// calls cannot spend the escrow's balance of anything else: the escrow is not
/// the immediate caller of that second token, and it never authorizes on its
/// own behalf.
#[test]
#[should_panic]
fn a_hostile_token_cannot_spend_the_escrows_other_assets() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    // Real custody the hostile token will try to reach.
    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &5_000);

    let thief = Address::generate(&h.env);
    let hostile = h.env.register(HostileToken, ());
    HostileTokenClient::new(&h.env, &hostile).init(&h.mint, &thief);

    client.sweep(&hostile, &thief);
}

/// Same guarantee stated from the other side: whatever the hostile call does,
/// the escrow's real custody is untouched afterwards.
#[test]
fn real_custody_survives_a_hostile_token_call() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &5_000);

    let thief = Address::generate(&h.env);
    let hostile = h.env.register(HostileToken, ());
    HostileTokenClient::new(&h.env, &hostile).init(&h.mint, &thief);

    let attempt = client.try_sweep(&hostile, &thief);

    assert!(attempt.is_err());
    assert_eq!(client.total_locked(&h.mint), 5_000);
    assert_eq!(TokenClient::new(&h.env, &h.mint).balance(&h.escrow), 5_000);
    assert_eq!(TokenClient::new(&h.env, &h.mint).balance(&thief), 0);
}

/// A hostile token that merely lies about the balance costs the admin a failed
/// transaction and nothing else. Nothing is credited, because sweep never
/// writes to TotalLocked.
#[test]
fn a_lying_balance_does_not_move_the_record() {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &0);

    let user = Address::generate(&h.env);
    h.fund(&user, 10_000);
    client.deposit(&user, &h.mint, &5_000);

    let before = client.total_locked(&h.mint);

    let liar = h.env.register(HostileToken, ());
    HostileTokenClient::new(&h.env, &liar).init(&h.mint, &Address::generate(&h.env));
    let _ = client.try_sweep(&liar, &Address::generate(&h.env));

    assert_eq!(client.total_locked(&h.mint), before);
}

/// Deposit and release only ever call an asset the admin opened, so the
/// hostile-token surface is limited to sweep.
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn deposit_refuses_an_asset_nobody_opened() {
    let h = Harness::new();
    let hostile = h.env.register(HostileToken, ());
    HostileTokenClient::new(&h.env, &hostile).init(&h.mint, &Address::generate(&h.env));

    let user = Address::generate(&h.env);
    StellarAssetClient::new(&h.env, &h.mint).mint(&user, &1_000);

    h.client().deposit(&user, &hostile, &100);
}

/// Takes a tenth of every transfer, the way a fee-on-transfer asset does.
#[contract]
pub struct FeeToken;

#[contractimpl]
impl FeeToken {
    pub fn init(e: Env, inner: Address) {
        e.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("inner"), &inner);
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        let inner: Address = e
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("inner"))
            .unwrap();
        TokenClient::new(&e, &inner).balance(&id)
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        let inner: Address = e
            .storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("inner"))
            .unwrap();
        TokenClient::new(&e, &inner).transfer(&from, &to, &(amount - amount / 10));
    }
}

/// The contract credits what it asked for, not what arrived, so an asset that
/// deducts a fee leaves the record permanently above the real balance and the
/// gap grows with every deposit.
///
/// This is why `deploy.md` says not to open such an asset. Pinned as a test so
/// the incompatibility is a fact about the code rather than a line in a runbook,
/// and so it fails loudly if anyone later changes `deposit` to measure the
/// delta instead.
#[test]
fn a_fee_taking_asset_drifts_the_record_above_the_balance() {
    let h = Harness::new();
    let client = h.client();

    let fee_token = h.env.register(FeeToken, ());
    FeeTokenClient::new(&h.env, &fee_token).init(&h.mint);
    client.allow_mint(&fee_token, &0);

    let user = Address::generate(&h.env);
    StellarAssetClient::new(&h.env, &h.mint).mint(&user, &10_000);

    client.deposit(&user, &fee_token, &1_000);

    // Asked for 1000, 900 arrived.
    assert_eq!(client.total_locked(&fee_token), 1_000);
    assert_eq!(TokenClient::new(&h.env, &h.mint).balance(&h.escrow), 900);

    // And it compounds.
    client.deposit(&user, &fee_token, &1_000);
    assert_eq!(client.total_locked(&fee_token), 2_000);
    assert_eq!(TokenClient::new(&h.env, &h.mint).balance(&h.escrow), 1_800);
}

/// `sweep` cannot paper over the shortfall: there is no surplus to move.
#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn sweep_cannot_repair_a_fee_taking_asset() {
    let h = Harness::new();
    let client = h.client();

    let fee_token = h.env.register(FeeToken, ());
    FeeTokenClient::new(&h.env, &fee_token).init(&h.mint);
    client.allow_mint(&fee_token, &0);

    let user = Address::generate(&h.env);
    StellarAssetClient::new(&h.env, &h.mint).mint(&user, &10_000);
    client.deposit(&user, &fee_token, &1_000);

    client.sweep(&fee_token, &Address::generate(&h.env));
}
