use soroban_sdk::{contracttype, Address};

// ledger, ~5s/ledger
pub(crate) const DAY_IN_LEDGERS: u32 = 17_280;

pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const PERSISTENT_BUMP_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
pub(crate) const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

/// Storage-layout revision written by the constructor.
pub const STORAGE_VERSION: u32 = 1;

pub const TREE_HEIGHT: u32 = 16;
pub const MAX_TREE_LEAVES: u64 = 1 << TREE_HEIGHT; // 65_536

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,                // instance: Address
    Version,              // instance: u32, storage-layout revision
    Root,                 // instance: BytesN<32>  (withdrawal_transactions_root)
    TreeIndex,            // instance: u64
    TotalLocked(Address), // persistent: i128, held per mint
    Operator(Address),    // persistent: bool
    AllowedMint(Address), // persistent: i128 release cap; presence = allowed
}
