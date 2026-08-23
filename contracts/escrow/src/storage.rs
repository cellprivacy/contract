use soroban_sdk::{Address, BytesN, Env};

use crate::storage_types::{
    DataKey, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};

pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

/// Read a persistent entry, refreshing its TTL when it exists.
fn get_persistent<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(
    e: &Env,
    key: &DataKey,
) -> Option<V> {
    let value = e.storage().persistent().get::<_, V>(key)?;
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
    Some(value)
}

/// Write a persistent entry and refresh its TTL.
fn set_persistent<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    e: &Env,
    key: &DataKey,
    value: &V,
) {
    e.storage().persistent().set(key, value);
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

// ----- Admin -----
pub fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(e: &Env, admin: Address) {
    e.storage().instance().set(&DataKey::Admin, &admin);
}

// ----- Version -----
//
// Storage-layout revision, written by the constructor. An upgrade that changes
// the layout reads this to know what it is migrating from. Adding the marker
// after the fact is far harder than carrying it from the start.
pub fn get_version(e: &Env) -> u32 {
    e.storage().instance().get(&DataKey::Version).unwrap_or(0)
}

pub fn set_version(e: &Env, v: u32) {
    e.storage().instance().set(&DataKey::Version, &v);
}

// ----- Root -----
pub fn get_root(e: &Env) -> BytesN<32> {
    e.storage().instance().get(&DataKey::Root).unwrap()
}
pub fn set_root(e: &Env, root: &BytesN<32>) {
    e.storage().instance().set(&DataKey::Root, root);
}

// ----- TreeIndex -----
// The constructor writes this alongside the root, so a missing entry means a
// corrupt instance rather than generation zero. Inventing a zero here would
// silently re-open every nonce of the first generation.
pub fn get_tree_index(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::TreeIndex).unwrap()
}
pub fn set_tree_index(e: &Env, i: u64) {
    e.storage().instance().set(&DataKey::TreeIndex, &i);
}

// ----- TotalLocked (persistent, per mint) -----
//
// Custody is tracked per mint: the contract may hold several assets at once and
// a release of one asset must never be backed by deposits of another.
pub fn get_total_locked(e: &Env, mint: &Address) -> i128 {
    get_persistent(e, &DataKey::TotalLocked(mint.clone())).unwrap_or(0)
}

pub fn set_total_locked(e: &Env, mint: &Address, v: i128) {
    set_persistent(e, &DataKey::TotalLocked(mint.clone()), &v);
}

// ----- Operators (persistent) -----
pub fn is_operator(e: &Env, op: &Address) -> bool {
    get_persistent(e, &DataKey::Operator(op.clone())).unwrap_or(false)
}

pub fn set_operator(e: &Env, op: &Address, enabled: bool) {
    let key = DataKey::Operator(op.clone());
    if enabled {
        set_persistent(e, &key, &true);
    } else {
        // Readers fall back to false, so removing is the same answer and stops
        // paying rent on a key that has been revoked.
        e.storage().persistent().remove(&key);
    }
}

// ----- Allowed mints (persistent) -----
pub fn is_allowed_mint(e: &Env, mint: &Address) -> bool {
    get_persistent(e, &DataKey::AllowedMint(mint.clone())).unwrap_or(false)
}

pub fn set_allowed_mint(e: &Env, mint: &Address, allowed: bool) {
    let key = DataKey::AllowedMint(mint.clone());
    if allowed {
        set_persistent(e, &key, &true);
    } else {
        e.storage().persistent().remove(&key);
    }
}
