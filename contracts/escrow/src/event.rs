use soroban_sdk::{contractevent, Address, BytesN, Env};

/// Emitted when a user locks assets in escrow.
///
/// Topics: `("deposit", from, mint)` — both addresses are indexed so the
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
