//! `release_funds`: operator gating, custody accounting, SMT-proof enforcement,
//! and replay (double-withdrawal) rejection.

use soroban_sdk::events::Event as _;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Vec};

use super::{Harness, RefTree};
use crate::storage_types::MAX_TREE_LEAVES;
use crate::Release;

/// An escrow holding `locked` of the primary asset, with one registered
/// operator and an empty withdrawal tree.
struct Funded {
    h: Harness,
    operator: Address,
    tree: RefTree,
}

impl Funded {
    fn new(locked: i128) -> Self {
        let h = Harness::new();
        let client = h.client();
        client.allow_mint(&h.mint);

        let operator = Address::generate(&h.env);
        client.add_operator(&operator);

        h.deposit_from_new_user(locked);
        let tree = RefTree::new(&h.env);

        Self { h, operator, tree }
    }
}

#[test]
fn release_pays_out_updates_custody_and_advances_the_root() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    assert_eq!(f.h.balance_of(&to), 300);
    assert_eq!(f.h.balance_of(&f.h.escrow), 700);
    assert_eq!(client.total_locked(&f.h.mint), 700);
    assert_eq!(client.root(), new_root);
}

#[test]
fn successive_releases_chain_onto_the_updated_root() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);

    let (siblings, new_root) = f.tree.spend(7);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    let (siblings, new_root) = f.tree.spend(9);
    client.release_funds(&f.operator, &f.h.mint, &to, &200, &9, &new_root, &siblings);

    assert_eq!(f.h.balance_of(&to), 500);
    assert_eq!(client.total_locked(&f.h.mint), 500);
    assert_eq!(client.root(), new_root);
}

/// Replaying a settled withdrawal must fail: the nonce's leaf is no longer
/// empty, so the exclusion proof against the current root cannot verify.
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn a_spent_nonce_cannot_be_withdrawn_twice() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);
}

/// The replay is rejected before any funds move.
#[test]
fn a_rejected_replay_leaves_custody_untouched() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    let replay =
        client.try_release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    assert!(replay.is_err());
    assert_eq!(f.h.balance_of(&to), 300);
    assert_eq!(client.total_locked(&f.h.mint), 700);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn release_is_rejected_for_an_unregistered_operator() {
    let mut f = Funded::new(1_000);
    let stranger = Address::generate(&f.h.env);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    f.h.client()
        .release_funds(&stranger, &f.h.mint, &to, &300, &7, &new_root, &siblings);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn release_is_rejected_after_the_operator_is_removed() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.remove_operator(&f.operator);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);
}

#[test]
#[should_panic]
fn release_requires_the_operator_auth() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    f.h.client().mock_auths(&[]).release_funds(
        &f.operator,
        &f.h.mint,
        &to,
        &300,
        &7,
        &new_root,
        &siblings,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn release_cannot_exceed_the_locked_total() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    f.h.client().release_funds(
        &f.operator,
        &f.h.mint,
        &to,
        &1_001,
        &7,
        &new_root,
        &siblings,
    );
}

/// Custody of one asset must not back a release of another.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn release_cannot_be_backed_by_a_different_assets_deposits() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let other = f.h.other_mint();
    client.allow_mint(&other);

    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.release_funds(&f.operator, &other, &to, &300, &7, &new_root, &siblings);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn release_is_rejected_for_a_blocked_mint() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.block_mint(&f.h.mint);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn release_rejects_a_non_positive_amount() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    f.h.client()
        .release_funds(&f.operator, &f.h.mint, &to, &0, &7, &new_root, &siblings);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn release_rejects_a_proof_of_the_wrong_length() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    let mut truncated = Vec::new(&f.h.env);
    for i in 0..3 {
        truncated.push_back(siblings.get(i).unwrap());
    }

    f.h.client()
        .release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &truncated);
}

/// A path that does not open to the current root is not a proof.
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn release_rejects_a_forged_proof() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    let mut forged = Vec::new(&f.h.env);
    forged.push_back(new_root.clone());
    for i in 1..siblings.len() {
        forged.push_back(siblings.get(i).unwrap());
    }

    f.h.client()
        .release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &forged);
}

/// `new_root` must be the current tree with exactly this nonce flipped; a root
/// that does not follow from the submitted path is refused.
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn release_rejects_a_new_root_that_does_not_follow_from_the_proof() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let (siblings, _) = f.tree.spend(7);
    let unrelated = RefTree::new(&f.h.env).root();

    f.h.client()
        .release_funds(&f.operator, &f.h.mint, &to, &300, &7, &unrelated, &siblings);
}

#[test]
fn release_publishes_the_settlement_event() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);
    let (siblings, new_root) = f.tree.spend(7);

    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    let expected = Release {
        to: to.clone(),
        mint: f.h.mint.clone(),
        amount: 300,
        nonce: 7,
        new_root: new_root.clone(),
        ledger: f.h.env.ledger().sequence(),
    };

    let events = f.h.env.events().all();
    let published = events.filter_by_contract(&f.h.escrow);
    assert_eq!(
        published.events(),
        &[expected.to_xdr(&f.h.env, &f.h.escrow)][..]
    );
}

/// A nonce belongs to the generation `nonce / MAX_TREE_LEAVES`, so one settled
/// under an earlier tree cannot be replayed once the tree has rotated.
#[test]
fn rotation_does_not_reopen_a_spent_nonce() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);

    let (siblings, new_root) = f.tree.spend(7);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    client.reset_smt_root(&f.operator, &0);

    // The nonce's leaf is empty again in the fresh tree, so the proof itself is
    // sound — only the generation check stands in the way.
    let mut fresh = RefTree::new(&f.h.env);
    let (siblings, new_root) = fresh.spend(7);
    let replay =
        client.try_release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    assert!(
        replay.is_err(),
        "nonce 7 was spendable again after rotation"
    );
    assert_eq!(f.h.balance_of(&to), 300);
}

/// After rotating, withdrawals draw on the next block of nonces.
#[test]
fn the_next_generation_of_nonces_is_spendable_after_rotation() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);

    let (siblings, new_root) = f.tree.spend(7);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    client.reset_smt_root(&f.operator, &0);

    let mut next = RefTree::new(&f.h.env);
    let nonce = MAX_TREE_LEAVES + 7;
    let (siblings, new_root) = next.spend(nonce);
    client.release_funds(
        &f.operator,
        &f.h.mint,
        &to,
        &200,
        &nonce,
        &new_root,
        &siblings,
    );

    assert_eq!(f.h.balance_of(&to), 500);
    assert_eq!(client.total_locked(&f.h.mint), 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn release_rejects_a_nonce_from_a_later_generation() {
    let mut f = Funded::new(1_000);
    let to = Address::generate(&f.h.env);
    let nonce = MAX_TREE_LEAVES + 7;
    let (siblings, new_root) = f.tree.spend(nonce);

    f.h.client().release_funds(
        &f.operator,
        &f.h.mint,
        &to,
        &300,
        &nonce,
        &new_root,
        &siblings,
    );
}

/// Two nonces sharing a leaf always sit in different generations, so the
/// wrap-around can never collide inside one tree.
#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn a_wrapped_nonce_cannot_reuse_a_leaf_in_the_same_tree() {
    let mut f = Funded::new(1_000);
    let client = f.h.client();
    let to = Address::generate(&f.h.env);

    let (siblings, new_root) = f.tree.spend(7);
    client.release_funds(&f.operator, &f.h.mint, &to, &300, &7, &new_root, &siblings);

    let wrapped = MAX_TREE_LEAVES + 7;
    let (siblings, new_root) = f.tree.spend(wrapped);
    client.release_funds(
        &f.operator,
        &f.h.mint,
        &to,
        &300,
        &wrapped,
        &new_root,
        &siblings,
    );
}
