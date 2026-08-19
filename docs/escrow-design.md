# Escrow Contract — Storage Layout & Authorization Flow

Design note for `contracts/escrow`. Behavioural spec lives in the workflow hub
(`cell-protocol-workflow/docs/contract.md`); this note records what the code
actually does and why, and is the authority for the on-chain interface.

## 1. What the contract is for

The escrow holds custody of real Stellar assets so that balances mirrored inside
the off-chain channel are redeemable. It does four things:

1. **Locks** assets a user deposits, and emits the event the indexer credits from.
2. **Caps** the channel: nothing can be released that was not first deposited.
3. **Gates** release behind an operator signature *and* a sparse-Merkle-tree
   proof that the withdrawal has not already been settled.
4. **Records** an admin/operator control surface the operator can rotate.

Individual in-channel transfers never touch this contract.

## 2. Storage layout

Two storage classes are used. Instance storage holds the singleton control
state, which is read on nearly every call and should live and die with the
contract. Persistent storage holds the per-key maps, which grow with usage and
must not bloat the instance footprint.

| Key | Class | Type | Meaning |
|-----|-------|------|---------|
| `Admin` | instance | `Address` | Sole holder of privileged configuration rights |
| `Root` | instance | `BytesN<32>` | Root of the active withdrawal SMT |
| `TreeIndex` | instance | `u32` | Generation counter, bumped on rotation |
| `TotalLocked(mint)` | persistent | `i128` | Assets under custody, **per asset** |
| `Operator(address)` | persistent | `bool` | Membership in the operator set |
| `AllowedMint(mint)` | persistent | `bool` | Whether the asset may be deposited |

### Why `TotalLocked` is keyed by mint

The contract can hold several assets at once. A single global total would let a
release of asset B be backed by deposits of asset A — a direct path to draining
custody of an asset nobody deposited. Keying by mint makes the backing invariant
hold per asset:

> for every mint `m`: `TotalLocked(m)` == the contract's balance of `m`,
> and no release of `m` may exceed it.

Covered by `locked_totals_are_tracked_per_mint` and
`release_cannot_be_backed_by_a_different_assets_deposits`.

### TTL policy

Instance state is extended to 30 days whenever touched. Persistent entries are
extended to 90 days on every read and write, so an operator or mint that is
actively in use never expires. Both are refreshed through the single
`get_persistent` / `set_persistent` pair in `storage.rs` rather than at call
sites, so no entry can be written without its TTL being renewed.

## 3. Authorization flow

Three roles, each enforced by `require_auth` on a specific address:

| Role | Established by | May call |
|------|----------------|----------|
| **Admin** | `initialize`, then `set_new_admin` | `set_new_admin`, `add_operator`, `remove_operator`, `allow_mint`, `block_mint`, `reset_smt_root` |
| **Operator** | `add_operator` | `release_funds` |
| **User** | — | `deposit` (authorizing the transfer of their own funds) |

```
initialize(admin) ──> Admin
     │
     ├─ allow_mint(mint) ─────────> deposits of `mint` accepted
     ├─ add_operator(op) ─────────> `op` may release
     ├─ set_new_admin(next) ──────> privilege moves, atomically
     └─ reset_smt_root() ─────────> new empty tree, TreeIndex + 1

user ── deposit(from, mint, amount) ── require_auth(from)
          └─ transfer(from -> escrow), TotalLocked(mint) += amount, emit Deposit

operator ── release_funds(..) ── require_auth(operator) + operator-set check
          ├─ exclusion proof: nonce's leaf is empty under the current Root
          ├─ inclusion proof: same path with that leaf spent yields new_root
          └─ transfer(escrow -> to), TotalLocked(mint) -= amount, Root = new_root
```

Two properties worth stating explicitly, because both are covered by tests
rather than left to inspection:

- **Authorization is bound to the stored admin, not to any signature.** A
  well-formed auth entry from a different address does not open the gate
  (`add_operator_rejects_a_non_admin_signer`).
- **Handover is complete.** After `set_new_admin` the previous admin has no
  residual rights (`the_previous_admin_loses_privilege_after_handover`).

The operator check is deliberately two-part: `operator.require_auth()` proves the
caller controls the key, and the `Operator(address)` lookup proves that key is
still authorized. Removing an operator revokes it immediately, mid-flight
signatures included (`release_is_rejected_after_the_operator_is_removed`).

`initialize` requires the incoming admin's own authorization, so the contract
cannot be initialized on someone's behalf.

## 4. Mint gating

`AllowedMint` is a per-asset switch, checked on both `deposit` and
`release_funds`. Assets default to **blocked**: a freshly initialized escrow
accepts nothing until the admin explicitly opens an asset, so the operator can
finish wiring an instance before user funds can arrive.

Blocking an asset stops new deposits *and* pauses releases of it — the intended
lever for freezing an asset during an incident.

## 5. The withdrawal SMT

A fixed-height sparse Merkle tree over SHA256 records which withdrawal nonces
have been settled. It is a replay guard, not an authorization scheme: the
operator decides *what* to pay, the tree ensures each nonce is paid *once*.

| Parameter | Value |
|-----------|-------|
| Height | 16 (`TREE_HEIGHT`) |
| Capacity | 65 536 nonces per tree (`MAX_TREE_LEAVES`) |
| Node hash | `SHA256(left ‖ right)`, 64-byte input |
| Empty leaf | 32 zero bytes |
| Spent leaf | `SHA256([0x01; 32])` |
| Empty root | the empty leaf folded 16 times through `H(n, n)` |
| Leaf position | `nonce mod 2^16` |
| Path order | **least-significant bit first** |

Path order is the detail most likely to break an off-chain prover: at level `l`,
bit `l` of the leaf position decides whether the running node is the left (`0`)
or right (`1`) child. A most-significant-first generator produces paths that
verify against nothing.

`release_funds` submits **one** sibling path and uses it twice:

- against the current root, starting from the *empty* leaf — proving the nonce
  is unspent;
- against `new_root`, starting from the *spent* leaf — proving `new_root` is the
  current tree with exactly that one leaf flipped.

Sharing the path is what makes the pair sound. Exclusion alone does not identify
a leaf (two empty siblings share a path, and `H(empty, empty)` is
order-independent), but the inclusion half yields a different `new_root` for
each position, so the nonce cannot be swapped
(`exclusion_is_shared_by_empty_siblings_but_inclusion_pins_the_leaf`).

Verification compares only the final computed root. An intermediate node that
happens to equal the target does not short-circuit the climb.

## 6. Events (indexer interface)

Published via `#[contractevent]`, so the first topic is the struct name in lower
snake case and the data body is a `Map<Symbol, Val>` keyed by field name.

| Event | Topics | Data |
|-------|--------|------|
| `Deposit` | `("deposit", from, mint)` | `amount`, `total_locked`, `ledger` |
| `Release` | `("release", to, mint)` | `amount`, `nonce`, `new_root`, `ledger` |
| `Rotate` | `("rotate",)` | `tree_index`, `new_root` |

Addresses are topics so the indexer can subscribe per user or per asset; scalar
payload is data. The exact XDR each event produces is asserted in
`deposit_publishes_the_indexer_event` and `release_publishes_the_settlement_event`.

## 7. Naming

The delivery plan uses the reference implementation's PascalCase names. The
contract uses Soroban's snake_case convention; the mapping is one-to-one:

| Plan | Contract |
|------|----------|
| `CreateInstance` | `initialize` |
| `AllowMint` / `BlockMint` | `allow_mint` / `block_mint` |
| `AddOperator` / `RemoveOperator` | `add_operator` / `remove_operator` |
| `SetNewAdmin` | `set_new_admin` |
| `Deposit` | `deposit` |
| `ReleaseFunds` | `release_funds` |
| `ResetSmtRoot` | `reset_smt_root` |

`initialize` is a singleton, not a multi-instance factory — one deployed
contract is one escrow. `cell-protocol-workflow/docs/contract.md` specifies the
same shape.

## 8. Open issues

1. **Rotation re-opens spent nonces.** `reset_smt_root` installs a fresh empty
   root, after which every nonce settled in an earlier tree verifies as unspent
   again. Nothing binds a spent marker to the `TreeIndex` it was recorded under.
   Fix by mixing `TreeIndex` into the leaf pre-image, or by requiring nonces to
   be globally monotonic across rotations. Test written and `#[ignore]`d:
   `rotation_must_not_reopen_a_spent_nonce`.
2. **The leaf commits to nothing but "spent".** `SHA256([0x01; 32])` is a
   constant, so a proof does not bind the recipient or the amount — those rest
   entirely on the operator's signature. If the tree is meant to carry
   cryptographic weight, the leaf should be `H(nonce ‖ to ‖ amount ‖ mint)`.
3. **No reference test vectors.** The SMT is tested against a reference tree
   built in `src/test/mod.rs` from the same primitives, which proves internal
   consistency but not agreement with the Solana reference implementation.
   Cross-checking needs the reference's hash construction, empty-node
   convention, and bit order confirmed.
4. **`deposit` keeps no per-deposit record.** `contract.md` specifies a
   `Deposit(user, id)` entry and a returned deposit id; the contract emits an
   event and tracks only the aggregate. Fine if the indexer is the system of
   record, but the two specs should be reconciled.
5. **The backend's event decoding does not match.** `cell-protocol`'s indexer
   routes on `"Deposit"`/`"Settlement"` and reads `from`/`amount` from the data
   map; this contract emits `"deposit"`/`"release"` with `from` as a topic. The
   dispatch and handlers need updating against §6 above.
6. **`settle()` does not exist here.** `cell_core::stellar::soroban::settle_args`
   encodes a provisional `settle(batch_id, total)` against the withdraw
   contract, which is not yet written.
