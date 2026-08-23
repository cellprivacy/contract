//! Property tests over generated input.
//!
//! The unit tests pin specific cases; these state the invariants those cases
//! are examples of, and let proptest look for a counterexample. Findings from
//! interactive fuzzing belong here too, as regressions that run in CI.

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::Address;

use super::{Harness, RefTree};
use crate::smt;
use crate::storage_types::MAX_TREE_LEAVES;

/// Escrow with an operator, an open asset and `funded` already deposited.
fn funded(cap: i128, deposit: i128) -> (Harness, Address) {
    let h = Harness::new();
    let client = h.client();
    client.allow_mint(&h.mint, &cap);
    let operator = Address::generate(&h.env);
    client.add_operator(&operator);

    let user = Address::generate(&h.env);
    h.fund(&user, deposit);
    client.deposit(&user, &h.mint, &deposit);
    (h, operator)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Whatever goes in and comes out, the record is the difference and the
    /// real balance agrees with it. This is the invariant the whole contract
    /// exists to hold.
    #[test]
    fn custody_is_conserved_across_deposits_and_releases(
        deposits in prop::collection::vec(1i128..1_000_000, 1..6),
        releases in prop::collection::vec(1i128..100_000, 0..5),
    ) {
        let h = Harness::new();
        let client = h.client();
        client.allow_mint(&h.mint, &0);
        let operator = Address::generate(&h.env);
        client.add_operator(&operator);

        let mut expected = 0i128;
        for (i, amount) in deposits.iter().enumerate() {
            let user = Address::generate(&h.env);
            h.fund(&user, *amount);
            client.deposit(&user, &h.mint, amount);
            expected += *amount;
            prop_assert_eq!(client.total_locked(&h.mint), expected, "after deposit {}", i);
        }

        let mut tree = RefTree::new(&h.env);
        let to = Address::generate(&h.env);
        for (nonce, amount) in releases.iter().enumerate() {
            if *amount > expected {
                continue;
            }
            let (siblings, new_root) = tree.spend(nonce as u64);
            client.release_funds(&operator, &h.mint, &to, amount, &(nonce as u64), &new_root, &siblings);
            expected -= *amount;
            prop_assert_eq!(client.total_locked(&h.mint), expected);
        }

        prop_assert_eq!(
            TokenClient::new(&h.env, &h.mint).balance(&h.escrow),
            expected,
            "record and real balance must agree"
        );
    }

    /// No release ever moves more than is recorded, whatever is asked for.
    #[test]
    fn a_release_never_exceeds_the_record(amount in 1i128..10_000_000) {
        let (h, operator) = funded(0, 1_000);
        let client = h.client();
        let mut tree = RefTree::new(&h.env);
        let (siblings, new_root) = tree.spend(0);
        let to = Address::generate(&h.env);

        let result = client.try_release_funds(&operator, &h.mint, &to, &amount, &0, &new_root, &siblings);

        if amount > 1_000 {
            prop_assert!(result.is_err(), "asked {} against 1000 locked", amount);
            prop_assert_eq!(client.total_locked(&h.mint), 1_000);
        } else {
            prop_assert!(result.is_ok());
            prop_assert_eq!(client.total_locked(&h.mint), 1_000 - amount);
        }
    }

    /// A ceiling means exactly what it says for one release.
    #[test]
    fn the_ceiling_holds_for_any_amount(cap in 1i128..1_000, amount in 1i128..2_000) {
        let (h, operator) = funded(cap, 5_000);
        let client = h.client();
        let mut tree = RefTree::new(&h.env);
        let (siblings, new_root) = tree.spend(0);
        let to = Address::generate(&h.env);

        let result = client.try_release_funds(&operator, &h.mint, &to, &amount, &0, &new_root, &siblings);
        prop_assert_eq!(result.is_ok(), amount <= cap);
    }

    /// A nonce belongs to exactly one generation, and only that generation
    /// accepts it.
    #[test]
    fn only_the_installed_generation_accepts_a_nonce(nonce in 0u64..500_000) {
        let (h, operator) = funded(0, 5_000);
        let client = h.client();
        let mut tree = RefTree::new(&h.env);
        let (siblings, new_root) = tree.spend(nonce);
        let to = Address::generate(&h.env);

        let result = client.try_release_funds(&operator, &h.mint, &to, &100, &nonce, &new_root, &siblings);
        prop_assert_eq!(result.is_ok(), nonce / MAX_TREE_LEAVES == 0);
    }

    /// After a sweep the record and the balance agree, and the record itself
    /// never moved.
    #[test]
    fn sweep_closes_the_gap_without_touching_the_record(
        deposit in 1i128..100_000,
        stray in 1i128..100_000,
    ) {
        let (h, _operator) = funded(0, deposit);
        let client = h.client();

        let donor = Address::generate(&h.env);
        h.fund(&donor, stray);
        TokenClient::new(&h.env, &h.mint).transfer(&donor, &h.escrow, &stray);

        let before = client.total_locked(&h.mint);
        let recovered = client.sweep(&h.mint, &Address::generate(&h.env));

        prop_assert_eq!(recovered, stray);
        prop_assert_eq!(client.total_locked(&h.mint), before);
        prop_assert_eq!(
            TokenClient::new(&h.env, &h.mint).balance(&h.escrow),
            before
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// An unspent nonce proves exclusion against the current root, and the same
    /// path with the leaf flipped proves inclusion against the next one.
    #[test]
    fn any_unspent_nonce_proves_exclusion_then_inclusion(
        nonce in 0u64..u64::MAX,
        spent in prop::collection::vec(0u64..MAX_TREE_LEAVES, 0..8),
    ) {
        let h = Harness::new();
        let mut tree = RefTree::new(&h.env);
        for n in &spent {
            tree.mark_spent(*n);
        }
        prop_assume!(!spent.iter().any(|n| n % MAX_TREE_LEAVES == nonce % MAX_TREE_LEAVES));

        let siblings = tree.proof(nonce);
        let current = tree.root();
        prop_assert!(smt::verify_exclusion(&h.env, &current, nonce, &siblings).is_ok());

        tree.mark_spent(nonce);
        prop_assert!(smt::verify_inclusion(&h.env, &tree.root(), nonce, &siblings).is_ok());
    }

    /// Once a leaf is spent it can never prove exclusion again.
    #[test]
    fn a_spent_leaf_never_proves_exclusion_again(nonce in 0u64..u64::MAX) {
        let h = Harness::new();
        let mut tree = RefTree::new(&h.env);

        let siblings = tree.proof(nonce);
        tree.mark_spent(nonce);

        prop_assert!(smt::verify_exclusion(&h.env, &tree.root(), nonce, &siblings).is_err());
    }
}
