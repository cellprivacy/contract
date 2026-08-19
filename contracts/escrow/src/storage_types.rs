use soroban_sdk::{contracttype, Address};

// ledger, ~5s/ledger
pub(crate) const DAY_IN_LEDGERS: u32 = 17_280;

pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const PERSISTENT_BUMP_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
pub(crate) const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,                // instance: Address
    Operator(Address),    // persistent: bool
    AllowedMint(Address), // persistent: bool
}
