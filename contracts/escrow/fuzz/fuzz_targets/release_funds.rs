#![no_main]
//! Drives the entrypoint that moves money, with the operator's arguments under
//! the fuzzer's control.
//!
//! The properties: a release never moves more than the record holds, the record
//! never goes negative, and the record and the real balance agree afterwards
//! whether the call succeeded or was refused.

use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, Env};

use escrow::fuzzing::RefTree;
use escrow::{EscrowContract, EscrowContractClient};

fuzz_target!(|input: (i128, i128, u64, i128)| {
    let (deposit, amount, nonce, cap) = input;

    // Keep the generated values inside what a token can actually hold.
    if !(1..=1_000_000_000).contains(&deposit) || !(0..=1_000_000_000).contains(&cap) {
        return;
    }

    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let escrow = env.register(
        EscrowContract,
        escrow::contract::EscrowContractArgs::__constructor(&admin),
    );
    let client = EscrowContractClient::new(&env, &escrow);
    let mint = env.register_stellar_asset_contract_v2(admin.clone()).address();

    client.allow_mint(&mint, &cap);
    let operator = Address::generate(&env);
    client.add_operator(&operator);

    let user = Address::generate(&env);
    StellarAssetClient::new(&env, &mint).mint(&user, &deposit);
    client.deposit(&user, &mint, &deposit);

    let mut tree = RefTree::new(&env);
    let siblings = tree.proof(nonce);
    tree.mark_spent(nonce);
    let new_root = tree.root();
    let to = Address::generate(&env);

    let result = client.try_release_funds(&operator, &mint, &to, &amount, &nonce, &new_root, &siblings);

    let locked = client.total_locked(&mint);
    let balance = TokenClient::new(&env, &mint).balance(&escrow);

    assert!(locked >= 0, "the record went negative");
    assert_eq!(locked, balance, "record and balance disagree");

    if result.is_ok() {
        assert!(amount > 0, "a non-positive amount was released");
        assert!(amount <= deposit, "released more than was deposited");
        assert!(cap == 0 || amount <= cap, "released above the ceiling");
        assert_eq!(nonce / 65_536, 0, "released against the wrong generation");
        assert_eq!(locked, deposit - amount);
    } else {
        assert_eq!(locked, deposit, "a refused release still moved the record");
    }
});
