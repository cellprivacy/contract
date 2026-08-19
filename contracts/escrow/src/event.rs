use soroban_sdk::{contractevent, Address, Env};

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
