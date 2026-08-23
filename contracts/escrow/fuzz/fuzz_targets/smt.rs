#![no_main]
//! The tree is a pure function of its inputs, which makes it the one part of
//! this contract worth fuzzing directly rather than through an entrypoint.
//!
//! The property: a proof generated for an unspent leaf verifies against the
//! root it was generated from, and stops verifying the moment that leaf is
//! spent. Anything else is a hole in the replay guard.

use libfuzzer_sys::fuzz_target;
use soroban_sdk::Env;

use escrow::fuzzing::{verify_exclusion, verify_inclusion, RefTree};

fuzz_target!(|input: (u64, Vec<u64>)| {
    let (nonce, spent) = input;
    if spent.len() > 24 {
        return;
    }

    let env = Env::default();
    let mut tree = RefTree::new(&env);
    for n in &spent {
        tree.mark_spent(*n);
    }

    let collides = spent.iter().any(|n| RefTree::position(*n) == RefTree::position(nonce));
    let siblings = tree.proof(nonce);
    let current = tree.root();

    if collides {
        // Leaf already spent: exclusion must fail.
        assert!(verify_exclusion(&env, &current, nonce, &siblings).is_err());
        return;
    }

    assert!(
        verify_exclusion(&env, &current, nonce, &siblings).is_ok(),
        "unspent leaf failed to prove exclusion"
    );

    tree.mark_spent(nonce);
    assert!(
        verify_inclusion(&env, &tree.root(), nonce, &siblings).is_ok(),
        "spending a leaf did not produce the proved root"
    );
    assert!(
        verify_exclusion(&env, &tree.root(), nonce, &siblings).is_err(),
        "a spent leaf still proved exclusion"
    );
});
