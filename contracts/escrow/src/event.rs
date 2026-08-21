use soroban_sdk::{contractevent, Address, BytesN, Env};

/// Emitted when a user locks assets in escrow.
///
/// Topics: `("deposit", from, mint)`. Both addresses are indexed so the
/// off-chain indexer can subscribe per user or per asset.
/// Data: a map of `amount`, `total_locked`, `ledger`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposit {
    #[topic]
    pub from: Address,
    #[topic]
    pub mint: Address,
    pub amount: i128,
    pub total_locked: i128,
    pub ledger: u32,
}

/// Emitted when an operator releases custody back to a user.
///
/// Topics: `("release", to, mint)`.
/// Data: a map of `amount`, `nonce`, `new_root`, `ledger`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    #[topic]
    pub to: Address,
    #[topic]
    pub mint: Address,
    pub amount: i128,
    pub nonce: u64,
    pub new_root: BytesN<32>,
    pub ledger: u32,
}

/// Emitted when the admin rotates the withdrawal tree.
///
/// Topics: `("rotate",)`. Data: a map of `tree_index`, `new_root`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rotate {
    pub tree_index: u64,
    pub new_root: BytesN<32>,
}

pub fn deposit(e: &Env, from: &Address, mint: &Address, amount: i128, total_locked: i128) {
    Deposit {
        from: from.clone(),
        mint: mint.clone(),
        amount,
        total_locked,
        ledger: e.ledger().sequence(),
    }
    .publish(e);
}

pub fn release(
    e: &Env,
    to: &Address,
    mint: &Address,
    amount: i128,
    nonce: u64,
    new_root: BytesN<32>,
) {
    Release {
        to: to.clone(),
        mint: mint.clone(),
        amount,
        nonce,
        new_root,
        ledger: e.ledger().sequence(),
    }
    .publish(e);
}

pub fn rotate(e: &Env, tree_index: u64, new_root: BytesN<32>) {
    Rotate {
        tree_index,
        new_root,
    }
    .publish(e);
}

/// Emitted when the admin replaces the contract executable.
///
/// Topics: `("upgraded",)`. Data: a map of `new_wasm_hash`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upgraded {
    pub new_wasm_hash: BytesN<32>,
}

pub fn upgraded(e: &Env, new_wasm_hash: BytesN<32>) {
    Upgraded { new_wasm_hash }.publish(e);
}

/// Emitted when admin rights move to another address.
///
/// Topics: `("admin_changed", previous, next)`. Data: a map of `ledger`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChanged {
    #[topic]
    pub previous: Address,
    #[topic]
    pub next: Address,
    pub ledger: u32,
}

/// Emitted when an address joins or leaves the operator set.
///
/// Topics: `("operator_set", operator)`. Data: a map of `enabled`, `ledger`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorSet {
    #[topic]
    pub operator: Address,
    pub enabled: bool,
    pub ledger: u32,
}

/// Emitted when an asset is opened for deposits or frozen.
///
/// Topics: `("mint_set", mint)`. Data: a map of `allowed`, `ledger`.
///
/// Freezing an asset is an incident lever, so it has to be visible to
/// whatever is watching this contract.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintSet {
    #[topic]
    pub mint: Address,
    pub allowed: bool,
    pub ledger: u32,
}

pub fn admin_changed(e: &Env, previous: &Address, next: &Address) {
    AdminChanged {
        previous: previous.clone(),
        next: next.clone(),
        ledger: e.ledger().sequence(),
    }
    .publish(e);
}

pub fn operator_set(e: &Env, operator: &Address, enabled: bool) {
    OperatorSet {
        operator: operator.clone(),
        enabled,
        ledger: e.ledger().sequence(),
    }
    .publish(e);
}

pub fn mint_set(e: &Env, mint: &Address, allowed: bool) {
    MintSet {
        mint: mint.clone(),
        allowed,
        ledger: e.ledger().sequence(),
    }
    .publish(e);
}
