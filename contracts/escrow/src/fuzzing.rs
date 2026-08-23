//! Test and fuzzing support: an off-chain tree that mirrors [`crate::smt`].
//!
//! Lives outside the test module so the fuzz crate can drive it too. Compiled
//! only under `cfg(test)` or the `fuzzing` feature, never in a real build.

use std::collections::BTreeSet;
use std::vec::Vec as StdVec;

use soroban_sdk::{BytesN, Env, Vec};

use crate::smt::{empty_leaf, hash_combine, non_empty_leaf};
use crate::storage_types::{MAX_TREE_LEAVES, TREE_HEIGHT};

pub use crate::smt::{empty_tree_root, verify_exclusion, verify_inclusion};

/// Off-chain sparse Merkle tree mirroring [`crate::smt`], used to produce the
/// sibling paths a real operator would submit.
///
/// Only spent positions are tracked. Every subtree with no spent leaf collapses
/// to the precomputed empty node for its level, which is what keeps a
/// 2^16-leaf tree cheap to evaluate in a unit test.
pub struct RefTree {
    env: Env,
    /// `empty[l]` is the root of an all-empty subtree of height `l`.
    empty: StdVec<BytesN<32>>,
    spent: BTreeSet<u64>,
}

impl RefTree {
    pub fn new(env: &Env) -> Self {
        let mut empty = StdVec::with_capacity(TREE_HEIGHT as usize + 1);
        empty.push(empty_leaf(env));
        for level in 0..TREE_HEIGHT as usize {
            let below = empty[level].clone();
            empty.push(hash_combine(env, &below, &below));
        }

        Self {
            env: env.clone(),
            empty,
            spent: BTreeSet::new(),
        }
    }

    pub fn root(&self) -> BytesN<32> {
        self.node(TREE_HEIGHT, 0)
    }

    /// Sibling path for `nonce`, least-significant bit first, which is the
    /// order [`crate::smt`] walks.
    pub fn proof(&self, nonce: u64) -> Vec<BytesN<32>> {
        let position = Self::position(nonce);
        let mut out = Vec::new(&self.env);
        for level in 0..TREE_HEIGHT {
            out.push_back(self.node(level, (position >> level) ^ 1));
        }
        out
    }

    pub fn mark_spent(&mut self, nonce: u64) {
        self.spent.insert(Self::position(nonce));
    }

    /// Proof material for spending `nonce`: the sibling path against the
    /// current tree, plus the root that results from marking it spent.
    pub fn spend(&mut self, nonce: u64) -> (Vec<BytesN<32>>, BytesN<32>) {
        let siblings = self.proof(nonce);
        self.mark_spent(nonce);
        (siblings, self.root())
    }

    pub fn position(nonce: u64) -> u64 {
        nonce % MAX_TREE_LEAVES
    }

    /// Hash of the subtree rooted at `index` on `level`.
    fn node(&self, level: u32, index: u64) -> BytesN<32> {
        let width = 1u64 << level;
        let start = index * width;
        if self.spent.range(start..start + width).next().is_none() {
            return self.empty[level as usize].clone();
        }
        if level == 0 {
            return non_empty_leaf(&self.env);
        }

        let left = self.node(level - 1, index * 2);
        let right = self.node(level - 1, index * 2 + 1);
        hash_combine(&self.env, &left, &right)
    }
}
