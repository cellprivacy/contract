use soroban_sdk::{contract, contractimpl, panic_with_error, token, Address, BytesN, Env, Vec};

use crate::error::EscrowError;
use crate::storage_types::{MAX_TREE_LEAVES, STORAGE_VERSION};
use crate::{event, smt, storage};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    // Runs inside the deployment transaction, called by the host and never
    // reachable afterwards. Initializing in a follow-up call would leave the
    // contract on chain with no admin for at least one ledger, long enough for
    // anyone watching to claim it as their own.
    pub fn __constructor(e: Env, admin: Address) {
        admin.require_auth();
        storage::set_version(&e, STORAGE_VERSION);
        storage::set_admin(&e, admin);
        storage::set_root(&e, &smt::empty_tree_root(&e));
        storage::set_tree_index(&e, 0);
        storage::extend_instance(&e);
    }

    // ---------- admin-gated config ----------
    // Both parties sign: the outgoing admin to give the rights up, the
    // incoming one to prove the address is controlled. A one-sided handover to
    // a mistyped address would strand admin rights permanently, and with them
    // the ability to upgrade.
    pub fn set_new_admin(e: Env, new_admin: Address) {
        let previous = storage::get_admin(&e);
        previous.require_auth();
        new_admin.require_auth();

        storage::set_admin(&e, new_admin.clone());
        storage::extend_instance(&e);
        event::admin_changed(&e, &previous, &new_admin);
    }

    pub fn add_operator(e: Env, operator: Address) {
        Self::require_admin(&e);
        storage::set_operator(&e, &operator, true);
        storage::extend_instance(&e);
        event::operator_set(&e, &operator, true);
    }

    pub fn remove_operator(e: Env, operator: Address) {
        Self::require_admin(&e);
        storage::set_operator(&e, &operator, false);
        storage::extend_instance(&e);
        event::operator_set(&e, &operator, false);
    }

    // Opening an asset and deciding how much may leave in one release are the
    // same decision, so they are the same call. A ceiling of zero means
    // uncapped; the admin has to type it rather than fall into it.
    //
    // Call again to change the ceiling on an asset that is already open.
    pub fn allow_mint(e: Env, mint: Address, release_cap: i128) {
        Self::require_admin(&e);
        if release_cap < 0 {
            panic_with_error!(&e, EscrowError::InvalidAmount);
        }
        storage::allow_mint(&e, &mint, release_cap);
        storage::extend_instance(&e);
        event::mint_set(&e, &mint, true, release_cap);
    }

    pub fn block_mint(e: Env, mint: Address) {
        Self::require_admin(&e);
        storage::block_mint(&e, &mint);
        storage::extend_instance(&e);
        event::mint_set(&e, &mint, false, 0);
    }

    // ---------- sweep ----------
    //
    // Moves the balance this contract holds beyond what it recorded as custody,
    // and nothing else. `TotalLocked` is not touched, so the ceiling on every
    // release is unchanged and backed custody is out of reach by construction.
    //
    // Surplus arrives from transfers straight to the contract address, which
    // bypass `deposit` entirely. Assets that were never opened are the common
    // case, so this deliberately does not require the asset to be allowed.
    pub fn sweep(e: Env, mint: Address, to: Address) -> i128 {
        Self::require_admin(&e);

        let escrow = e.current_contract_address();
        if to == escrow {
            panic_with_error!(&e, EscrowError::InvalidRecipient);
        }

        let token = token::Client::new(&e, &mint);
        let balance = token.balance(&escrow);
        let locked = storage::get_total_locked(&e, &mint);

        // Below zero means the real balance has fallen under the record, which
        // a clawback or a fee-on-transfer asset can do. Nothing to recover, and
        // the shortfall is not this function's problem to paper over.
        let surplus = balance - locked;
        if surplus <= 0 {
            panic_with_error!(&e, EscrowError::NoSurplus);
        }

        storage::extend_instance(&e);
        event::swept(&e, &mint, &to, surplus, locked);
        token.transfer(&escrow, &to, &surplus);

        surplus
    }

    // ---------- deposit ----------
    pub fn deposit(e: Env, from: Address, mint: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(&e, EscrowError::InvalidAmount);
        }
        // Mirror of the guard in release_funds. Depositing from the escrow to
        // itself moves nothing but would still credit the recorded custody.
        if from == e.current_contract_address() {
            panic_with_error!(&e, EscrowError::InvalidRecipient);
        }
        if !storage::is_allowed_mint(&e, &mint) {
            panic_with_error!(&e, EscrowError::MintNotAllowed);
        }

        // State first, then the asset moves. Soroban rejects reentry into a
        // contract already on the call stack, so this is defence in depth
        // against a custom token contract rather than a live hole, but the
        // token is the one address here we do not control.
        let total = storage::get_total_locked(&e, &mint) + amount;
        storage::set_total_locked(&e, &mint, total);

        // Deposits are the only user-facing entrypoint. Without this an escrow
        // that takes deposits but has not released or been reconfigured lets
        // its instance, and with it the contract code, fall out of the live
        // state.
        storage::extend_instance(&e);

        let escrow = e.current_contract_address();
        token::Client::new(&e, &mint).transfer(&from, &escrow, &amount);

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

        if to == e.current_contract_address() {
            panic_with_error!(&e, EscrowError::InvalidRecipient);
        }

        // The ceiling does not stop a compromised operator, who can release
        // repeatedly. It turns one transaction into a visible sequence of them,
        // which is the only thing on chain that buys anyone reaction time.
        let cap = storage::get_release_cap(&e, &mint);
        if cap > 0 && amount > cap {
            panic_with_error!(&e, EscrowError::ReleaseCapExceeded);
        }

        let total = storage::get_total_locked(&e, &mint);
        if amount > total {
            panic_with_error!(&e, EscrowError::InsufficientLocked);
        }

        // The nonce's generation is derived from the nonce itself, so a nonce
        // settled under an earlier tree can never be replayed after a rotation,
        // and two nonces sharing a leaf always sit in different generations.
        if nonce / MAX_TREE_LEAVES != storage::get_tree_index(&e) {
            panic_with_error!(&e, EscrowError::WrongTreeGeneration);
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

        // The nonce is spent and the custody is debited before anything is
        // paid out, so the proof cannot be replayed from inside the transfer.
        storage::set_root(&e, &new_root);
        storage::set_total_locked(&e, &mint, total - amount);
        storage::extend_instance(&e);

        let escrow = e.current_contract_address();
        token::Client::new(&e, &mint).transfer(&escrow, &to, &amount);

        event::release(&e, &to, &mint, amount, total - amount, nonce, new_root);
    }

    // ---------- tree rotation ----------
    //
    // Rotation is an operational step: the operator starts a new tree once the
    // current generation's 65 536 nonces are used up. `expected_tree_index`
    // guards against a duplicate landing, which would otherwise advance the
    // counter again and strand a whole generation of nonces.
    pub fn reset_smt_root(e: Env, operator: Address, expected_tree_index: u64) {
        operator.require_auth();
        if !storage::is_operator(&e, &operator) {
            panic_with_error!(&e, EscrowError::NotAuthorized);
        }
        if storage::get_tree_index(&e) != expected_tree_index {
            panic_with_error!(&e, EscrowError::UnexpectedTreeIndex);
        }

        let idx = expected_tree_index + 1;
        let previous = storage::get_root(&e);
        let root = smt::empty_tree_root(&e);
        storage::set_tree_index(&e, idx);
        storage::set_root(&e, &root);
        storage::extend_instance(&e);
        event::rotate(&e, idx, previous, root);
    }

    // ---------- upgrade ----------
    //
    // Replaces the contract's own executable. The wasm must already be uploaded
    // to the ledger; only its hash is passed here. Admin-gated rather than
    // operator-gated because it can change every rule in this file, including
    // who the admin is.
    //
    // Storage is untouched, so the new executable inherits the admin, the
    // operator set, the mint permissions, the locked totals and the tree. A
    // release that changes the storage layout has to migrate it in the same
    // invocation or in a follow-up call.
    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        Self::require_admin(&e);
        storage::extend_instance(&e);
        event::upgraded(&e, new_wasm_hash.clone());
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    // ---------- views ----------
    pub fn admin(e: Env) -> Address {
        storage::get_admin(&e)
    }

    pub fn root(e: Env) -> BytesN<32> {
        storage::get_root(&e)
    }

    pub fn version(e: Env) -> u32 {
        storage::get_version(&e)
    }

    pub fn tree_index(e: Env) -> u64 {
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

    pub fn release_cap(e: Env, mint: Address) -> i128 {
        storage::get_release_cap(&e, &mint)
    }

    // ---------- internal ----------
    fn require_admin(e: &Env) {
        storage::get_admin(e).require_auth();
    }
}
