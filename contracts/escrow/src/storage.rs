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
pub fn has_admin(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Admin)
}

pub fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(e: &Env, admin: Address) {
    e.storage().instance().set(&DataKey::Admin, &admin);
}

// ----- Root -----
pub fn get_root(e: &Env) -> BytesN<32> {
    e.storage().instance().get(&DataKey::Root).unwrap()
}
pub fn set_root(e: &Env, root: &BytesN<32>) {
    e.storage().instance().set(&DataKey::Root, root);
}

// ----- TreeIndex -----
pub fn get_tree_index(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::TreeIndex).unwrap_or(0)
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
    set_persistent(e, &DataKey::Operator(op.clone()), &enabled);
}

// ----- Allowed mints (persistent) -----
pub fn is_allowed_mint(e: &Env, mint: &Address) -> bool {
    get_persistent(e, &DataKey::AllowedMint(mint.clone())).unwrap_or(false)
}

pub fn set_allowed_mint(e: &Env, mint: &Address, allowed: bool) {
    set_persistent(e, &DataKey::AllowedMint(mint.clone()), &allowed);
}
