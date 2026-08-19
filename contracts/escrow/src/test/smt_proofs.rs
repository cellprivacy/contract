//! The SHA256 sparse Merkle tree itself: empty-tree construction, inclusion and
//! exclusion proofs, and the bit ordering an off-chain prover must match.

use soroban_sdk::{BytesN, Env, Vec};

use super::RefTree;
use crate::smt;
use crate::storage_types::{MAX_TREE_LEAVES, TREE_HEIGHT};

/// The contract's empty root and the reference tree's empty root are the same
/// construction: `H(n, n)` folded `TREE_HEIGHT` times from a 32-byte zero leaf.
#[test]
fn the_empty_root_matches_the_reference_tree() {
    let env = Env::default();
    assert_eq!(smt::empty_tree_root(&env), RefTree::new(&env).root());
}

#[test]
fn the_empty_root_is_the_expected_fold() {
    let env = Env::default();

    let mut node = smt::empty_leaf(&env);
    for _ in 0..TREE_HEIGHT {
        node = smt::hash_combine(&env, &node, &node);
    }

    assert_eq!(smt::empty_tree_root(&env), node);
}

#[test]
fn an_unspent_nonce_proves_exclusion_and_its_successor_root_proves_inclusion() {
    let env = Env::default();

    for nonce in [0u64, 1, 7, 12_345, MAX_TREE_LEAVES - 1] {
        let mut tree = RefTree::new(&env);
        let siblings = tree.proof(nonce);
        let current = tree.root();

        assert!(smt::verify_exclusion(&env, &current, nonce, &siblings).is_ok());

        tree.mark_spent(nonce);
        assert!(smt::verify_inclusion(&env, &tree.root(), nonce, &siblings).is_ok());
    }
}

/// A path drawn from a populated tree still verifies against that tree's root.
#[test]
fn proofs_verify_against_a_populated_tree() {
    let env = Env::default();
    let mut tree = RefTree::new(&env);
    tree.mark_spent(3);
    tree.mark_spent(1_000);

    let siblings = tree.proof(8);
    assert!(smt::verify_exclusion(&env, &tree.root(), 8, &siblings).is_ok());
}

#[test]
fn a_spent_nonce_no_longer_proves_exclusion() {
    let env = Env::default();
    let mut tree = RefTree::new(&env);

    let siblings = tree.proof(7);
    tree.mark_spent(7);

    assert_eq!(
        smt::verify_exclusion(&env, &tree.root(), 7, &siblings),
        Err(crate::EscrowError::InvalidSmtProof)
    );
}

/// A proof is bound to its position: a sibling path from one subtree does not
/// open the root at a leaf in another.
#[test]
fn a_proof_does_not_transfer_to_another_subtree() {
    let env = Env::default();
    let mut tree = RefTree::new(&env);
    tree.mark_spent(3);

    let siblings = tree.proof(8);

    assert!(smt::verify_exclusion(&env, &tree.root(), 8, &siblings).is_ok());
    assert_eq!(
        smt::verify_exclusion(&env, &tree.root(), 24, &siblings),
        Err(crate::EscrowError::InvalidSmtProof)
    );
}

/// Two empty sibling leaves share a sibling path, and `H(empty, empty)` is
/// order-independent, so a single exclusion proof covers both. That is sound —
/// both leaves really are unspent — but it means exclusion alone does not pin
/// down *which* leaf a withdrawal refers to.
///
/// The inclusion half is what binds the nonce: spending either leaf with the
/// same path yields a different `new_root`, so `release_funds` cannot be made
/// to mark one leaf while claiming another.
#[test]
fn exclusion_is_shared_by_empty_siblings_but_inclusion_pins_the_leaf() {
    let env = Env::default();
    let tree = RefTree::new(&env);
    let root = tree.root();
    let siblings = tree.proof(8);

    assert!(smt::verify_exclusion(&env, &root, 8, &siblings).is_ok());
    assert!(smt::verify_exclusion(&env, &root, 9, &siblings).is_ok());

    let mut spend_eight = RefTree::new(&env);
    spend_eight.mark_spent(8);
    let mut spend_nine = RefTree::new(&env);
    spend_nine.mark_spent(9);

    assert_ne!(spend_eight.root(), spend_nine.root());
    assert!(smt::verify_inclusion(&env, &spend_eight.root(), 8, &siblings).is_ok());
    assert_eq!(
        smt::verify_inclusion(&env, &spend_eight.root(), 9, &siblings),
        Err(crate::EscrowError::InvalidSmtProof)
    );
}

#[test]
fn a_proof_of_the_wrong_length_is_refused() {
    let env = Env::default();
    let tree = RefTree::new(&env);
    let root = tree.root();

    let short: Vec<BytesN<32>> = Vec::new(&env);
    assert_eq!(
        smt::verify_exclusion(&env, &root, 7, &short),
        Err(crate::EscrowError::InvalidProofLength)
    );

    let mut long = tree.proof(7);
    long.push_back(smt::empty_leaf(&env));
    assert_eq!(
        smt::verify_exclusion(&env, &root, 7, &long),
        Err(crate::EscrowError::InvalidProofLength)
    );
}

/// Leaf position is `nonce mod 2^TREE_HEIGHT`, so a nonce and its wrap-around
/// share a leaf. Capacity per tree is therefore `MAX_TREE_LEAVES` nonces.
#[test]
fn nonces_wrap_onto_the_same_leaf() {
    let env = Env::default();
    let mut tree = RefTree::new(&env);

    let siblings = tree.proof(7);
    tree.mark_spent(7);

    assert_eq!(RefTree::position(7 + MAX_TREE_LEAVES), 7);
    assert_eq!(
        smt::verify_exclusion(&env, &tree.root(), 7 + MAX_TREE_LEAVES, &siblings),
        Err(crate::EscrowError::InvalidSmtProof)
    );
}

/// The traversal reads the position least-significant bit first. Flipping the
/// lowest bit swaps the leaf with its immediate sibling, which a
/// most-significant-first prover would place at the opposite end of the tree.
#[test]
fn traversal_is_least_significant_bit_first() {
    let env = Env::default();
    let mut tree = RefTree::new(&env);
    tree.mark_spent(0);

    // Position 1 is the sibling of position 0, so its level-0 sibling is the
    // spent leaf rather than an empty one.
    let siblings = tree.proof(1);
    assert_eq!(siblings.get(0).unwrap(), smt::non_empty_leaf(&env));
    assert!(smt::verify_exclusion(&env, &tree.root(), 1, &siblings).is_ok());
}
