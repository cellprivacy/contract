use soroban_sdk::{contract, contractimpl, panic_with_error, token, Address, BytesN, Env, Vec};

use crate::error::EscrowError;
use crate::{event, smt, storage};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    pub fn initialize(e: Env, admin: Address) {
        if storage::has_admin(&e) {
            panic_with_error!(&e, EscrowError::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_admin(&e, admin);
        storage::set_root(&e, &smt::empty_tree_root(&e));
        storage::set_tree_index(&e, 0);
        storage::extend_instance(&e);
    }

    // ---------- admin-gated config ----------
    pub fn set_new_admin(e: Env, new_admin: Address) {
        Self::require_admin(&e);
        storage::set_admin(&e, new_admin);
        storage::extend_instance(&e);
    }

    pub fn add_operator(e: Env, operator: Address) {
        Self::require_admin(&e);
        storage::set_operator(&e, &operator, true);
    }

    pub fn remove_operator(e: Env, operator: Address) {
        Self::require_admin(&e);
        storage::set_operator(&e, &operator, false);
    }

    pub fn allow_mint(e: Env, mint: Address) {
        Self::require_admin(&e);
        storage::set_allowed_mint(&e, &mint, true);
    }

    pub fn block_mint(e: Env, mint: Address) {
        Self::require_admin(&e);
        storage::set_allowed_mint(&e, &mint, false);
    }

    // ---------- deposit ----------
    pub fn deposit(e: Env, from: Address, mint: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(&e, EscrowError::InvalidAmount);
        }
        if !storage::is_allowed_mint(&e, &mint) {
            panic_with_error!(&e, EscrowError::MintNotAllowed);
        }

        let escrow = e.current_contract_address();
        token::Client::new(&e, &mint).transfer(&from, &escrow, &amount);

        let total = storage::get_total_locked(&e, &mint) + amount;
        storage::set_total_locked(&e, &mint, total);

        event::deposit(&e, &from, &mint, amount, total);
    }

    // ---------- release_funds (SMT-gated) ----------
    //
    // The argument list is flat rather than bundled into a struct so the
    // invocation encoding stays a plain positional list for the off-chain
    // operator and the reference implementation.
    #[allow(clippy::too_many_arguments)]
    pub fn release_funds(
        e: Env,
        operator: Address,
        mint: Address,
        to: Address,
        amount: i128,
        nonce: u64,
        new_root: BytesN<32>,
        siblings: Vec<BytesN<32>>,
    ) {
        operator.require_auth();
        if !storage::is_operator(&e, &operator) {
            panic_with_error!(&e, EscrowError::NotAuthorized);
        }
        if amount <= 0 {
            panic_with_error!(&e, EscrowError::InvalidAmount);
        }
        if !storage::is_allowed_mint(&e, &mint) {
            panic_with_error!(&e, EscrowError::MintNotAllowed);
        }

        let total = storage::get_total_locked(&e, &mint);
        if amount > total {
            panic_with_error!(&e, EscrowError::InsufficientLocked);
        }

        // nonce must not spent in the current tree
        let current_root = storage::get_root(&e);
        if let Err(err) = smt::verify_exclusion(&e, &current_root, nonce, &siblings) {
            panic_with_error!(&e, err);
        }

        // nonce must be included in new_root
        if let Err(err) = smt::verify_inclusion(&e, &new_root, nonce, &siblings) {
            panic_with_error!(&e, err);
        }

        let escrow = e.current_contract_address();
        token::Client::new(&e, &mint).transfer(&escrow, &to, &amount);

        storage::set_root(&e, &new_root);
        storage::set_total_locked(&e, &mint, total - amount);
        storage::extend_instance(&e);

        event::release(&e, &to, &mint, amount, nonce, new_root);
    }

    // ---------- tree rotation ----------
    pub fn reset_smt_root(e: Env) {
        Self::require_admin(&e);
        let idx = storage::get_tree_index(&e) + 1;
        let root = smt::empty_tree_root(&e);
        storage::set_tree_index(&e, idx);
        storage::set_root(&e, &root);
        storage::extend_instance(&e);
        event::rotate(&e, idx, root);
    }

    // ---------- views ----------
    pub fn admin(e: Env) -> Address {
        storage::get_admin(&e)
    }

    pub fn root(e: Env) -> BytesN<32> {
        storage::get_root(&e)
    }

    pub fn tree_index(e: Env) -> u32 {
        storage::get_tree_index(&e)
    }

    pub fn total_locked(e: Env, mint: Address) -> i128 {
        storage::get_total_locked(&e, &mint)
    }

    pub fn is_operator(e: Env, who: Address) -> bool {
        storage::is_operator(&e, &who)
    }

    pub fn is_allowed_mint(e: Env, mint: Address) -> bool {
        storage::is_allowed_mint(&e, &mint)
    }

    // ---------- internal ----------
    fn require_admin(e: &Env) {
        storage::get_admin(e).require_auth();
    }
}
