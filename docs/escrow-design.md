# Escrow Contract, Storage Layout & Authorization Flow

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
| `TreeIndex` | instance | `u64` | Generation counter, bumped on rotation |
| `TotalLocked(mint)` | persistent | `i128` | Assets under custody, **per asset** |
| `Operator(address)` | persistent | `bool` | Membership in the operator set |
| `AllowedMint(mint)` | persistent | `bool` | Whether the asset may be deposited |

### Why `TotalLocked` is keyed by mint

The contract can hold several assets at once. A single global total would let a
release of asset B be backed by deposits of asset A, which is a direct path to
draining custody of an asset nobody deposited. Keying by mint makes the backing invariant
hold per asset:

> for every mint `m`, no release of `m` may exceed `TotalLocked(m)`.

The contract enforces that bound. It does **not** reconcile `TotalLocked`
against the token's real balance, and cannot: it never reads
`token::Client::balance`. `TotalLocked(m)` is a record of what the escrow
accepted through `deposit`, and the real balance can sit either side of it.

Above, if someone transfers an allowed asset straight to the contract address
without calling `deposit`. That surplus is unrecoverable, since no entrypoint
can move an asset except `release_funds` and that is capped by `TotalLocked`.

Below, if the asset is a Stellar Asset Contract whose issuer set
`AUTH_CLAWBACK_ENABLED_FLAG` before the balance existed, or who revokes
authorization. Releases then fail at the token rather than at the escrow's own
check. Native XLM has no issuer and neither applies.

Covered by `locked_totals_are_tracked_per_mint` and
`release_cannot_be_backed_by_a_different_assets_deposits`.

### TTL policy

`extend_ttl` is a no-op while the remaining lifetime is still above its
threshold, so both policies below are "top back up when it gets low", not
"extend on every call".

Persistent entries are topped back up to 90 days once fewer than 89 days
remain. They go through the single `get_persistent` / `set_persistent` pair in
`storage.rs` rather than through call sites, so no persistent entry can be read
or written without its lifetime being considered.

Instance state is topped back up to 30 days by every entrypoint that changes
state, `deposit` included. That last one matters: `deposit` is the only
user-facing entrypoint, and an escrow can take deposits for a long time without
a release or a configuration change. Instance storage shares its lifetime with
the contract code, so letting it lapse takes the contract offline until someone
restores it. Covered by `deposit_keeps_the_instance_alive`.

Nothing extends a lifetime automatically. The host never bumps on access, and
`ExtendFootprintTTLOp` has no access control, so anyone may extend any entry.
Expiry is an availability and rent concern, never a security boundary.

One consequence worth knowing: because reads extend, the `total_locked`,
`is_operator` and `is_allowed_mint` views can write a lifetime extension and
charge rent. A monitoring loop polling them needs a read-write footprint.
`root` and `tree_index` read instance storage without extending and are
genuinely read-only.

## 3. Authorization flow

Three roles, each enforced by `require_auth` on a specific address:

| Role | Established by | May call |
|------|----------------|----------|
| **Admin** | `initialize`, then `set_new_admin` | `set_new_admin`, `add_operator`, `remove_operator`, `allow_mint`, `block_mint`, `upgrade` |
| **Operator** | `add_operator` | `release_funds`, `reset_smt_root` |
| **User** |, | `deposit` (authorizing the transfer of their own funds) |

```
initialize(admin) ──> Admin
     │
     ├─ allow_mint(mint) ─────────> deposits of `mint` accepted
     ├─ add_operator(op) ─────────> `op` may release
     └─ set_new_admin(next) ──────> privilege moves, atomically

user ── deposit(from, mint, amount) ── require_auth(from)
          └─ transfer(from -> escrow), TotalLocked(mint) += amount, emit Deposit

operator ── reset_smt_root(expected) ── require_auth + operator-set check
          └─ new empty tree, TreeIndex + 1

operator ── release_funds(..) ── require_auth(operator) + operator-set check
          ├─ generation check: nonce / MAX_TREE_LEAVES == TreeIndex
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

`initialize` requires the incoming admin's own authorization, so nobody can
name a third party as admin. That is narrower than it sounds. Deploy and
initialize are separate transactions in separate ledgers, and between them the
contract sits on chain with no admin. Anyone watching closed ledgers can call
`initialize` nominating themselves, satisfy the authorization check with their
own key, and the `AlreadyInitialized` guard then makes it permanent.

Soroban's answer to this is `__constructor` (CAP-0058), which runs inside the
deployment transaction with arguments supplied at deploy time. Moving
initialization there closes the window. This contract has not done so yet; see
§8.

## 4. Mint gating

`AllowedMint` is a per-asset switch, checked on both `deposit` and
`release_funds`. Assets default to **blocked**: a freshly initialized escrow
accepts nothing until the admin explicitly opens an asset, so the operator can
finish wiring an instance before user funds can arrive.

Blocking an asset stops new deposits *and* pauses releases of it, which is the
intended lever for freezing an asset during an incident. `MintSet` is emitted so
the freeze is visible off-chain.

The gate does a second job. `deposit` and `release_funds` call into a token
contract, which is the one address in those invocations this contract does not
control, and a hostile token can insert authorization entries of its own into
the tree. Because both paths refuse anything the admin has not explicitly
opened, the contract never calls a token nobody vetted.

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
| Empty root | `8fe6b168...2beb`, the empty leaf folded 16 times through `H(n, n)`, stored as a constant |
| Leaf position | `nonce mod 2^16` |
| Path order | **least-significant bit first** |

Path order is the detail most likely to break an off-chain prover: at level `l`,
bit `l` of the leaf position decides whether the running node is the left (`0`)
or right (`1`) child. A most-significant-first generator produces paths that
verify against nothing.

### Generations

A nonce carries its own generation: `nonce / MAX_TREE_LEAVES` must equal the
installed `TreeIndex`. Nonces are therefore allocated in blocks of 65 536, one
block per tree, and the rule does three things at once:

- a nonce settled under an earlier tree cannot be replayed after a rotation,
  even though its leaf is empty again in the fresh tree;
- two nonces that share a leaf (`n` and `n + 65 536`) always sit in different
  generations, so the wrap-around can never collide inside one tree;
- an operator cannot spend ahead into a generation that has not been installed.

Rotation is correspondingly guarded: `reset_smt_root` takes the tree index it
expects to be replacing and refuses a stale one, because a second landing of the
same rotation would advance the counter again and strand a whole generation of
nonces.

### Proof structure

`release_funds` submits **one** sibling path and uses it twice:

- against the current root, starting from the *empty* leaf, proving the nonce
  is unspent;
- against `new_root`, starting from the *spent* leaf, proving `new_root` is the
  current tree with exactly that one leaf flipped.

Sharing the path is what makes the pair sound. Exclusion alone does not identify
a leaf (two empty siblings share a path, and `H(empty, empty)` is
order-independent), but the inclusion half yields a different `new_root` for
each position, so the nonce cannot be swapped
(`exclusion_is_shared_by_empty_siblings_but_inclusion_pins_the_leaf`).

Verification compares only the final computed root. An intermediate node that
happens to equal the target does not short-circuit the climb.

This is a deliberate divergence from the reference implementation, which returns
early on an intermediate match and has a test asserting that behaviour. Reaching
the stored root before the last level requires a collision, so the practical
difference is nil, but a proof that has not been walked to the top has not been
verified. Every legitimate proof is treated identically by both.

### Cross-checking

`vectors/smt_vectors.json` holds parameters and eight worked cases: empty tree,
LSB-set nonces, the last leaf, a wrapped nonce, and paths with occupied
siblings. The vectors are generated independently of this crate and the suite
reproduces every value, so they pin down hash construction, empty-node
convention and bit ordering at once.

`empty_tree_root` matches the reference implementation's hard-coded constant
byte for byte, which is the strongest single check available without running
that code.

An off-chain prover should be held to the same file.

## 5b. Upgrades

`upgrade(new_wasm_hash)` replaces the contract's own executable. The wasm must
already be on the ledger; only its hash is passed. Admin-gated rather than
operator-gated, because it can rewrite every rule in this document, including
who the admin is.

Storage is untouched by the swap, so the new executable inherits the admin, the
operator set, the mint permissions, the locked totals and the tree. A release
that changes the storage layout has to migrate it, in the same invocation or in
a follow-up call.

Two consequences worth stating plainly:

- The admin key is as powerful as the contract. Anyone holding it can install
  code that releases custody without a proof. Key custody is the control here,
  not the contract.
- A deployed instance without this entrypoint cannot be upgraded at all. The
  first testnet deployment predates it and is stuck on its original code.

## 6. Events (indexer interface)

Published via `#[contractevent]`, so the first topic is the struct name in lower
snake case and the data body is a `Map<Symbol, Val>` keyed by field name.

| Event | Topics | Data |
|-------|--------|------|
| `Deposit` | `("deposit", from, mint)` | `amount`, `total_locked`, `ledger` |
| `Release` | `("release", to, mint)` | `amount`, `nonce`, `new_root`, `ledger` |
| `Rotate` | `("rotate",)` | `tree_index`, `new_root` |
| `Upgraded` | `("upgraded",)` | `new_wasm_hash` |
| `AdminChanged` | `("admin_changed", previous, next)` | `ledger` |
| `OperatorSet` | `("operator_set", operator)` | `enabled`, `ledger` |
| `MintSet` | `("mint_set", mint)` | `allowed`, `ledger` |

The whole control surface emits, not only the money movements. Freezing an
asset is an incident lever and has to be visible to whatever is watching.

On upgrade the host also emits its own system event, topics
`("executable_update", old_executable, new_executable)` with no data. The
contract's `Upgraded` event is additive rather than necessary; an indexer
already subscribed to this contract sees the latter without subscribing to
system events.

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

`initialize` is a singleton rather than a multi-instance factory: one deployed
contract is one escrow. `cell-protocol-workflow/docs/contract.md` specifies the
same shape.

## 8. Open issues

1. **`initialize` is front-runnable in the window before it lands.** Soroban has
   `__constructor` (CAP-0058), which runs inside the deployment transaction with
   arguments passed at deploy time and closes the window entirely. Moving to it
   changes the deployment flow and the contract interface, so it is a decision
   rather than a fix. The already-deployed testnet instances are unaffected,
   having been initialized by their deployer.
2. **Admin handover cannot be undone by the incoming admin alone.** Both parties
   now sign, so a mistyped address is caught. Nothing recovers the instance if
   the new admin later loses their key; the escrow keeps working but can never
   be reconfigured or upgraded again.
3. **The leaf commits to nothing but "spent".** `SHA256([0x01; 32])` is a
   constant, so a proof does not bind the recipient or the amount; both rest
   entirely on the operator's signature. If the tree is meant to carry
   cryptographic weight, the leaf should be `H(nonce ‖ to ‖ amount ‖ mint)`.
4. **No off-chain prover yet.** `vectors/smt_vectors.json` fixes the tree's
   behaviour and `empty_tree_root` matches the reference constant, so the
   algorithm is pinned. What does not exist anywhere is the component that
   *generates* proofs, so nothing can currently call `release_funds`.
5. **`deposit` keeps no per-deposit record.** `contract.md` specifies a
   `Deposit(user, id)` entry and a returned deposit id; the contract emits an
   event and tracks only the aggregate. Fine if the indexer is the system of
   record, but the two specs should be reconciled.
6. **The backend's event decoding does not match.** `cell-protocol`'s indexer
   routes on `"Deposit"`/`"Settlement"` and reads `from`/`amount` from the data
   map; this contract emits `"deposit"`/`"release"` with `from` as a topic. The
   dispatch and handlers need updating against §6 above.
7. **`settle()` does not exist here.** `cell_core::stellar::soroban::settle_args`
   encodes a provisional `settle(batch_id, total)` against the withdraw
   contract, which is not yet written.
