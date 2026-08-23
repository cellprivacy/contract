# Deploy Guide

Build, deploy and wire up the escrow contract. Commands are shown against
testnet; mainnet differs only in `--network` and the identity used.

## Prerequisites

- `stellar` CLI 27 or later
- Rust with the `wasm32v1-none` target: `rustup target add wasm32v1-none`

## Build

```sh
cd contracts/escrow
stellar contract build
```

Produces `target/wasm32v1-none/release/escrow.wasm`. Run `make check` and
`cargo test` before deploying; the pre-commit hook covers the first two but not
a release build.

## Deploy

The admin is set by the constructor, inside the deployment transaction. Pass it
after the `--` separator.

**The admin must be the deploying identity.** The constructor calls
`admin.require_auth()`, and `stellar contract deploy` signs only with
`--source`. Naming a different address traps inside the constructor and rolls
the whole deployment back:

```
[log] VM call trapped with HostError, __constructor, Error(Auth, InvalidAction)
```

Deploying on behalf of a separate admin means building the transaction, adding
that address's authorization entry, collecting its signature and submitting by
hand. If that is what you need, deploy with the operator identity as admin and
hand over afterwards with `set_new_admin`, which is designed for it and takes
both signatures.

```sh
stellar keys generate <identity> --network testnet --fund
ADMIN=$(stellar keys address <identity>)

stellar contract deploy \
  --wasm target/wasm32v1-none/release/escrow.wasm \
  --source <identity> \
  --network testnet \
  -- --admin $ADMIN
```

Prints the contract id (`C...`). Record it, the wasm hash and both transaction
hashes: the upload and the deploy are separate transactions.

There is no separate initialization step, and no window in which the contract
exists without an admin.

## Wire up

Order matters: nothing can be deposited until the asset is allowed, and nothing
can be released until an operator is registered.

```sh
C=<contract id>
NATIVE=$(stellar contract id asset --asset native --network testnet)

# Open an asset for deposits, with the ceiling on a single release.
# 0 means uncapped, and has to be typed rather than fallen into.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  allow_mint --mint $NATIVE --release_cap <stroops or 0>

# Register the operator that will settle withdrawals.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  add_operator --operator <operator G...>
```

Both emit an event, so the control surface is visible off-chain.

Check the wiring:

```sh
stellar contract invoke --id $C --source <identity> --network testnet -- admin
stellar contract invoke --id $C --source <identity> --network testnet -- root
stellar contract invoke --id $C --source <identity> --network testnet -- tree_index
```

A freshly deployed instance reports tree index `0` and root
`8fe6b1689256c0d385f42f5bbe2027a22c1996e110ba97c171d3e5948de92beb`, the empty
tree root. If `root` differs, something has already been settled against it.

## Operating

**Deposit** is called by the user, authorized by their own signature:

```sh
stellar contract invoke --id $C --source <user> --network testnet -- \
  deposit --from <user G...> --mint $NATIVE --amount <stroops>
```

**Release** is called by a registered operator and needs a proof. `siblings` is
a JSON array of 16 hex-encoded 32-byte hashes, least-significant-bit first; see
`escrow-design.md` §5 and the worked cases in
`../contracts/escrow/vectors/smt_vectors.json`.

```sh
stellar contract invoke --id $C --source <operator> --network testnet -- \
  release_funds --operator <operator G...> --mint $NATIVE --to <recipient G...> \
  --amount <stroops> --nonce <n> --new_root <hex32> --siblings '["<hex32>", ...]'
```

The nonce must satisfy `nonce / 65536 == tree_index`, so nonces are allocated in
blocks of 65 536 per generation and never reused.

**Sweep** recovers balance the contract holds beyond what it recorded as
custody, which is what a transfer straight to the contract address leaves
behind. It moves that difference and nothing else, so backed custody is out of
reach whatever arguments it is given. It works for assets that were never
opened, which is the usual case for strays.

```sh
stellar contract invoke --id $C --source <identity> --network testnet -- \
  sweep --mint <asset C...> --to <recipient G...>
```

Fails with `#11` when there is no surplus, including when the real balance has
fallen *below* the record, which a clawback or a fee-on-transfer asset can do.

**Rotate** once a generation's nonces are used up. `expected_tree_index` guards
against a replay landing twice and stranding a generation:

```sh
stellar contract invoke --id $C --source <operator> --network testnet -- \
  reset_smt_root --operator <operator G...> --expected_tree_index <current>
```

## Before allowing an asset

`allow_mint` is the only check the contract makes on a token. Everything else
about that token is assumed, so look at it first.

- **Clawback.** If the issuer set `AUTH_CLAWBACK_ENABLED_FLAG` before the
  escrow's balance existed, the issuer can take it back. `TotalLocked` would
  then sit above the real balance and releases fail at the token.
- **Revocable authorization.** `AUTH_REVOCABLE_FLAG` lets the issuer deauthorize
  the escrow's balance, with the same effect.
- **Fee on transfer.** The contract credits `TotalLocked` with the amount it was
  asked for, not the amount that arrived. A token that deducts a fee on transfer
  leaves the recorded custody permanently above the real balance, the shortfall
  grows with every deposit, and `sweep` cannot fix it because there is no
  surplus to move. Do not allow such a token.
- **Release ceiling.** `allow_mint` takes one. Pick a figure that a settlement
  batch will not normally exceed, so a compromised operator key has to make
  several visible transactions rather than one. It does not stop a drain; it
  slows one down enough to notice.
- **Non-standard decimals or supply hooks.** Anything that makes `transfer` do
  something other than move exactly `amount` breaks the same assumption.

Native XLM has no issuer and none of these apply.

## Upgrading

`upgrade` replaces the contract's executable in place. Storage survives, so the
admin, operators, mint permissions, locked totals and tree all carry over. Only
the admin may call it.

```sh
# 1. Put the new wasm on the ledger. Prints its hash.
stellar contract upload \
  --wasm target/wasm32v1-none/release/escrow.wasm \
  --source <admin identity> --network testnet

# 2. Point the contract at it.
stellar contract invoke --id $C --source <admin identity> --network testnet -- \
  upgrade --new_wasm_hash <hash from step 1>
```

An instance deployed without the `upgrade` entrypoint cannot be upgraded. If a
release changes the storage layout, migrate it in the same invocation or in a
follow-up call, because the swap does not touch storage.

## Deployments

### Testnet

Built with soroban-sdk 27. The admin is set by the constructor, so there is no
separate initialization transaction.

| | |
|---|---|
| Contract | `CBH3J73JTD77DUQ6FAGVOPFTY3CDE6V6DEFW6CH4QQQGCINBB76KLKTY` |
| Wasm hash | `139affa0c2480ec3333b891f4af413da24d09768646d8ee4d8649a1637d73faf` |
| Network | Test SDF Network ; September 2015 |
| Admin | `GCGSY4IOU7PG2QN2Z744ZVWMSZD5MYINLPKB5XSGQQECEU7NJBWUWO4Q` |
| Operator | `GA4LOTZNKXSNACOM56YWMIUXEER3NRD7ABJFSPHZP5VOUNROGJZIST7G` |
| Asset | native XLM SAC, `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

| Step | Hash |
|---|---|
| Upload wasm | `c5ba69644b4cb34b2088fc1abe62a86c74b6bd3c04fb91b083cdb1a145cf2cc0` |
| Deploy, admin set by the constructor | `0612c164a2dba8a449223ffafb844db637170c508c4e5f9a096e75bb074e7c6f` |
| `allow_mint` | `86fdeae5844328fced3a082c43fdf5c7455a31444c446e9ae273846e78b37fdb` |
| `add_operator` | `151ba9019c5bde8d17f6fc1607e3ade75db3c3b9fe7dfc6c7705da9fde4e56c5` |
| `deposit` 100 XLM | `107e3b9def1242107102f0c286193ee8cd43f32f3d61556ce5fcc7b599c23429` |
| `release_funds` nonce 0, 30 XLM | `34b7bc76797fd35d47673bfb5cce2c90e85a7db869e6f252875476b692f9b8be` |
| `release_funds` nonce 1, 10 XLM | `2651afd4971abe5163ea848529ae3d9897e89bf101ab1236994fd36d37c60d16` |
| `release_funds` nonce 2, 10 XLM | `1f57fabc918bfdade5afec78f962c9300c543be66da123dd73c83304cd9505a1` |
| `release_funds` nonce 3, 10 XLM | `fa3d93cd7436e0f9d4b41f96bfb04406f05f8e4b3bdfd8e15c859fc403c73369` |
| Duplicate of nonce 3, **failed on chain** | `0ded56a0a9ec736bf0610df4788c97e305d0c5396a338324581a30bcc1dde2ec` |

`total_locked` reads back `400000000` stroops. The release proofs were taken
verbatim from `smt_vectors.json`, so that file is confirmed usable by an
off-chain prover against a live network.

The duplicate withdrawal is a genuine on-chain failure, not a simulation error.
It was built and simulated against the state before nonce 3 was spent, then
submitted after the original landed, which is exactly what happens when an
operator's submission is beaten to the ledger. It reached ledger 4253601 and
failed there with contract error `#6`. No XLM moved.

### Upgrade verified

Verified twice, in-crate and on the network.

`state_survives_an_upgrade` in `src/test/wasm.rs` uploads the compiled wasm,
deploys through a factory so the constructor runs under real authorization,
loads the instance with a deposit, an operator, a mint permission and a
rotation, then upgrades and reads every piece of state back. Run it with
`make test-wasm`.

On testnet, instance `CA3ZXQF2BKFH2KNNGQAYJAOJZ7N5XPUTQU5BS2PL5TCJT6XKKFPHI75E`
was upgraded in transaction
`69f6f7f1686c4242ca861d0476d0b68b75909e77a895f12b1b0ce0cbb7580594`, and a
second instance was upgraded mid-run with fifteen nonces already spent, in
`9e578fb38e0367a2ecb2894957e507b78dccfb0325a4d0fa96f910fc47aa74e3`. In both
cases admin, operator set, mint permission, locked total and tree index read
back unchanged, and further calls settled normally on the new executable. An
`upgrade` submitted by the operator was refused: simulation demanded the admin
key.

Both instances predate the constructor, so they are also the evidence that a
contract deployed without `upgrade` cannot be upgraded at all.

### Superseded

`CAOWXO6MVNRP26XHPCK5KRQ44GUKCRYYCOLQ5PBHKACIAOIXKC6L7ZHR` and
`CDORRV4DXCI73L23PG5IA7WAO4XH4WMGX3KOCCSOYAIMWW3A5C3DTJ36` were earlier records.
Both predate the constructor and the security review fixes, and both were
initialized in a follow-up transaction. Left in place only as history.

### Mainnet

Not deployed.
