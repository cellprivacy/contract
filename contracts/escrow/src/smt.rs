use soroban_sdk::{Bytes, BytesN, Env, Vec};

use crate::error::EscrowError;
use crate::storage_types::{MAX_TREE_LEAVES, TREE_HEIGHT};

// SHA256(left || right)
pub fn hash_combine(e: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    let mut buf = [0u8; 64];
    buf[0..32].copy_from_slice(&left.to_array());
    buf[32..64].copy_from_slice(&right.to_array());

    e.crypto().sha256(&Bytes::from_array(e, &buf)).to_bytes()
}

pub fn empty_leaf(e: &Env) -> BytesN<32> {
    BytesN::from_array(e, &[0u8; 32])
}

pub fn non_empty_leaf(e: &Env) -> BytesN<32> {
    e.crypto()
        .sha256(&Bytes::from_array(e, &[1u8; 32]))
        .to_bytes()
}

/// Root of an all-empty tree: the empty leaf folded through `H(n, n)`
/// `TREE_HEIGHT` times.
///
/// Precomputed so `initialize` and `reset_smt_root` do not pay sixteen hashes
/// each. `the_empty_root_is_the_expected_fold` recomputes the fold and fails if
/// this constant ever drifts from `TREE_HEIGHT`.
pub const EMPTY_TREE_ROOT: [u8; 32] = [
    0x8f, 0xe6, 0xb1, 0x68, 0x92, 0x56, 0xc0, 0xd3, 0x85, 0xf4, 0x2f, 0x5b, 0xbe, 0x20, 0x27, 0xa2,
    0x2c, 0x19, 0x96, 0xe1, 0x10, 0xba, 0x97, 0xc1, 0x71, 0xd3, 0xe5, 0x94, 0x8d, 0xe9, 0x2b, 0xeb,
];

pub fn empty_tree_root(e: &Env) -> BytesN<32> {
    BytesN::from_array(e, &EMPTY_TREE_ROOT)
}

/// Walk `start` up `TREE_HEIGHT` levels using `siblings` and check the computed
/// root equals `target`.
///
/// The path is derived from `nonce` **least-significant bit first**: at level
/// `l`, bit `l` of the leaf position selects whether the running node is the
/// left (`0`) or right (`1`) child. Off-chain proof generators must use the same
/// order or every proof fails.
///
/// The comparison happens only after the full climb. An intermediate node that
/// happens to equal `target` does not shortcut verification.
fn climb(
    e: &Env,
    start: BytesN<32>,
    nonce: u64,
    siblings: &Vec<BytesN<32>>,
    target: &BytesN<32>,
) -> Result<(), EscrowError> {
    if siblings.len() != TREE_HEIGHT {
        return Err(EscrowError::InvalidProofLength);
    }

    let leaf_position = nonce % MAX_TREE_LEAVES;
    let mut current = start;
    for (level, sibling) in siblings.iter().enumerate() {
        let bit = (leaf_position >> level) & 1;
        current = if bit == 0 {
            hash_combine(e, &current, &sibling)
        } else {
            hash_combine(e, &sibling, &current)
        };
    }

    if current == *target {
        Ok(())
    } else {
        Err(EscrowError::InvalidSmtProof)
    }
}

/// Prove `nonce`'s leaf is still empty under `current_root`, i.e. the nonce has
/// not been spent in the active tree.
pub fn verify_exclusion(
    e: &Env,
    current_root: &BytesN<32>,
    nonce: u64,
    siblings: &Vec<BytesN<32>>,
) -> Result<(), EscrowError> {
    climb(e, empty_leaf(e), nonce, siblings, current_root)
}

/// Prove the same path, with `nonce`'s leaf now marked spent, produces
/// `new_root`. Sharing `siblings` with the exclusion check is what forces
/// `new_root` to be the current tree with exactly this one leaf flipped.
pub fn verify_inclusion(
    e: &Env,
    new_root: &BytesN<32>,
    nonce: u64,
    siblings: &Vec<BytesN<32>>,
) -> Result<(), EscrowError> {
    climb(e, non_empty_leaf(e), nonce, siblings, new_root)
}
