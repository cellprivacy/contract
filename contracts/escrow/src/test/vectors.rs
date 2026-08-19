//! Cross-implementation check against `vectors/smt_vectors.json`.
//!
//! The vectors are generated independently of this crate. Reproducing every
//! value here proves the contract's tree agrees with an outside implementation
//! on hash construction, empty-node convention and bit ordering — the three
//! things a proof generator can silently get wrong. Any off-chain prover should
//! be held to the same file.

use soroban_sdk::{BytesN, Env, Vec};

use super::RefTree;
use crate::smt;
use crate::storage_types::{MAX_TREE_LEAVES, TREE_HEIGHT};

const VECTORS: &str = include_str!("../../vectors/smt_vectors.json");

fn doc() -> serde_json::Value {
    serde_json::from_str(VECTORS).expect("smt_vectors.json is not valid JSON")
}

fn hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert_eq!(bytes.len(), 64, "expected 32 bytes of hex, got {s}");
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = (bytes[i * 2] as char).to_digit(16).expect("bad hex") as u8;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16).expect("bad hex") as u8;
        *byte = (hi << 4) | lo;
    }
    out
}

fn bytes32(env: &Env, s: &str) -> BytesN<32> {
    BytesN::from_array(env, &hex32(s))
}

#[test]
fn tree_parameters_match_the_vectors() {
    let env = Env::default();
    let doc = doc();

    assert_eq!(doc["tree_height"].as_u64().unwrap() as u32, TREE_HEIGHT);
    assert_eq!(doc["max_tree_leaves"].as_u64().unwrap(), MAX_TREE_LEAVES);
    assert_eq!(
        smt::empty_leaf(&env),
        bytes32(&env, doc["empty_leaf"].as_str().unwrap())
    );
    assert_eq!(
        smt::non_empty_leaf(&env),
        bytes32(&env, doc["non_empty_leaf"].as_str().unwrap())
    );
    assert_eq!(
        smt::empty_tree_root(&env),
        bytes32(&env, doc["empty_tree_root"].as_str().unwrap())
    );
}

/// Every case: the roots and the sibling path must come out identical, and the
/// contract's own verifier must accept the pair.
#[test]
fn every_vector_reproduces_and_verifies() {
    let env = Env::default();
    let doc = doc();

    for case in doc["cases"].as_array().unwrap() {
        let nonce = case["nonce"].as_u64().unwrap();

        let mut tree = RefTree::new(&env);
        for spent in case["spent_before"].as_array().unwrap() {
            tree.mark_spent(spent.as_u64().unwrap());
        }

        let current_root = bytes32(&env, case["current_root"].as_str().unwrap());
        assert_eq!(tree.root(), current_root, "current_root, nonce {nonce}");

        let mut expected = Vec::new(&env);
        for sibling in case["siblings"].as_array().unwrap() {
            expected.push_back(bytes32(&env, sibling.as_str().unwrap()));
        }
        let siblings = tree.proof(nonce);
        assert_eq!(siblings, expected, "siblings, nonce {nonce}");

        tree.mark_spent(nonce);
        let new_root = bytes32(&env, case["new_root"].as_str().unwrap());
        assert_eq!(tree.root(), new_root, "new_root, nonce {nonce}");

        smt::verify_exclusion(&env, &current_root, nonce, &siblings)
            .unwrap_or_else(|e| panic!("exclusion failed for nonce {nonce}: {e:?}"));
        smt::verify_inclusion(&env, &new_root, nonce, &siblings)
            .unwrap_or_else(|e| panic!("inclusion failed for nonce {nonce}: {e:?}"));
    }
}

/// The constant must stay in step with `TREE_HEIGHT`.
#[test]
fn the_precomputed_empty_root_equals_the_fold() {
    let env = Env::default();

    let mut node = smt::empty_leaf(&env);
    for _ in 0..TREE_HEIGHT {
        node = smt::hash_combine(&env, &node, &node);
    }

    assert_eq!(node, smt::empty_tree_root(&env));
}
