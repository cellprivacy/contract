use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env};

use crate::error::EscrowError;
use crate::storage;

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

    // ---------- views ----------
    pub fn admin(e: Env) -> Address {
        storage::get_admin(&e)
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
