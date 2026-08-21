use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    AlreadyInitialized = 1,
    NotAuthorized = 2,
    MintNotAllowed = 3,
    InvalidAmount = 4,
    InvalidProofLength = 5,
    InvalidSmtProof = 6,
    InsufficientLocked = 7,
    /// The nonce does not belong to the tree generation currently installed.
    WrongTreeGeneration = 8,
    /// The rotation was submitted against a stale tree index.
    UnexpectedTreeIndex = 9,
    /// The payout target is the escrow itself, which would debit custody
    /// without moving anything.
    InvalidRecipient = 10,
}
