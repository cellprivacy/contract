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

```sh
stellar keys generate <identity> --network testnet --fund
stellar contract deploy \
  --wasm target/wasm32v1-none/release/escrow.wasm \
  --source <identity> \
  --network testnet \
  -- --admin <admin G...>
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

# Open an asset for deposits. Assets are blocked by default.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  allow_mint --mint $NATIVE

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

### Superseded

`CAOWXO6MVNRP26XHPCK5KRQ44GUKCRYYCOLQ5PBHKACIAOIXKC6L7ZHR` and
`CDORRV4DXCI73L23PG5IA7WAO4XH4WMGX3KOCCSOYAIMWW3A5C3DTJ36` were earlier records.
Both predate the constructor and the security review fixes, and both were
initialized in a follow-up transaction. Left in place only as history.

### Mainnet

Not deployed.
