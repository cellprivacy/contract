//! Tests that need the contract's own compiled wasm on disk.
//!
//! Two paths cannot be reached through `Env::register`. It mocks all
//! authorization for the constructor, so `__constructor`'s `require_auth` is
//! never exercised; and `upgrade` needs a wasm hash that actually exists on the
//! ledger, which means uploading real bytes.
//!
//! Run with `make test-wasm`, which builds the wasm first. Gated behind the
//! `wasm-tests` feature so a clean checkout can still `cargo test`.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

use crate::{EscrowContract, EscrowContractClient};

const WASM: &[u8] = include_bytes!("../../../../target/wasm32v1-none/release/escrow.wasm");

/// Deployment has to happen from inside a contract frame, the way a real
/// factory does it. Calling `deploy_v2` straight from the test context leaves
/// the constructor's `require_auth` untied to any root invocation, which is a
/// different failure from the one being tested.
#[contract]
pub struct Factory;

#[contractimpl]
impl Factory {
    pub fn deploy(e: Env, wasm_hash: BytesN<32>, admin: Address, salt: BytesN<32>) -> Address {
        e.deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, (admin,))
    }
}

fn deploy(env: &Env, admin: &Address, salt: u8) -> Address {
    let factory = env.register(Factory, ());
    let wasm_hash = env.deployer().upload_contract_wasm(WASM);
    FactoryClient::new(env, &factory).deploy(
        &wasm_hash,
        admin,
        &BytesN::from_array(env, &[salt; 32]),
    )
}

/// The constructor authorizes the admin it is given. Without that signature the
/// deployment traps and no contract is created.
#[test]
#[should_panic(expected = "InvalidAction")]
fn the_constructor_requires_the_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // No auth mocked at all: the constructor's require_auth has nothing to
    // satisfy it.
    deploy(&env, &admin, 1);
}

#[test]
fn a_deployment_with_admin_auth_comes_up_configured() {
    let env = Env::default();
    // The constructor's require_auth runs inside a sub-invocation of the
    // factory's deploy, so it is not tied to the root invocation.
    env.mock_all_auths_allowing_non_root_auth();
    let admin = Address::generate(&env);

    let escrow = deploy(&env, &admin, 2);
    let client = EscrowContractClient::new(&env, &escrow);

    assert_eq!(client.admin(), admin);
    assert_eq!(client.tree_index(), 0);
    assert_eq!(client.root(), crate::smt::empty_tree_root(&env));
}

/// Storage survives the executable swap. This is the claim the deploy runbook
/// makes, and the reason a release that changes the storage layout has to
/// migrate it.
#[test]
fn state_survives_an_upgrade() {
    let env = Env::default();
    // The constructor's require_auth runs inside a sub-invocation of the
    // factory's deploy, so it is not tied to the root invocation.
    env.mock_all_auths_allowing_non_root_auth();
    let admin = Address::generate(&env);

    let escrow = deploy(&env, &admin, 3);
    let client = EscrowContractClient::new(&env, &escrow);

    let mint = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let operator = Address::generate(&env);
    client.allow_mint(&mint, &0);
    client.add_operator(&operator);

    let user = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &mint).mint(&user, &1_000);
    client.deposit(&user, &mint, &500);
    client.reset_smt_root(&operator, &0);

    let wasm_hash = env.deployer().upload_contract_wasm(WASM);
    client.upgrade(&wasm_hash);

    assert_eq!(client.admin(), admin);
    assert_eq!(client.tree_index(), 1);
    assert_eq!(client.total_locked(&mint), 500);
    assert!(client.is_operator(&operator));
    assert!(client.is_allowed_mint(&mint));

    // The new executable still works.
    let another = Address::generate(&env);
    client.add_operator(&another);
    assert!(client.is_operator(&another));
}

/// Registering the type directly cannot test the constructor's authorization,
/// which is why the tests above go through a real deployment.
#[test]
fn env_register_masks_constructor_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);

    // No mock_all_auths, yet this succeeds.
    let escrow = env.register(
        EscrowContract,
        crate::contract::EscrowContractArgs::__constructor(&admin),
    );

    assert_eq!(EscrowContractClient::new(&env, &escrow).admin(), admin);
}
