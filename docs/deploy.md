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
`cargo test` before deploying — the pre-commit hook covers the first two but not
a release build.

## Deploy

```sh
stellar keys generate <identity> --network testnet --fund
stellar contract deploy \
  --wasm target/wasm32v1-none/release/escrow.wasm \
  --source <identity> \
  --network testnet
```

Prints the contract id (`C...`). Record it, the wasm hash and both transaction
hashes — the upload and the deploy are separate transactions.

## Wire up

Order matters: nothing can be deposited until the asset is allowed, and nothing
can be released until an operator is registered.

```sh
C=<contract id>
ADMIN=$(stellar keys address <identity>)
NATIVE=$(stellar contract id asset --asset native --network testnet)

# 1. Claim the instance. Requires the incoming admin's own signature.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  initialize --admin $ADMIN

# 2. Open an asset for deposits. Assets are blocked by default.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  allow_mint --mint $NATIVE

# 3. Register the operator that will settle withdrawals.
stellar contract invoke --id $C --source <identity> --network testnet -- \
  add_operator --operator <operator G...>
```

Check the wiring:

```sh
stellar contract invoke --id $C --source <identity> --network testnet -- admin
stellar contract invoke --id $C --source <identity> --network testnet -- root
stellar contract invoke --id $C --source <identity> --network testnet -- tree_index
```

A freshly initialized instance reports tree index `0` and root
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
`escrow-design.md` §5 and the worked cases in `../contracts/escrow/vectors/smt_vectors.json`.

```sh
stellar contract invoke --id $C --source <operator> --network testnet -- \
  release_funds --operator <operator G...> --mint $NATIVE --to <recipient G...> \
  --amount <stroops> --nonce <n> --new_root <hex32> --siblings '["<hex32>", ...]'
```

The nonce must satisfy `nonce / 65536 == tree_index`, so nonces are allocated in
blocks of 65 536 per generation and never reused.

**Rotate** once a generation's nonces are used up. `expected_tree_index` guards
against a replay landing twice and stranding a generation:

```sh
stellar contract invoke --id $C --source <operator> --network testnet -- \
  reset_smt_root --operator <operator G...> --expected_tree_index <current>
```

## Deployments

### Testnet

| | |
|---|---|
| Contract | `CDORRV4DXCI73L23PG5IA7WAO4XH4WMGX3KOCCSOYAIMWW3A5C3DTJ36` |
| Wasm hash | `f1de47d4f3503274cb98d23be91f3b139609b6149e5d945200b85e55b053ebf4` |
| Network | Test SDF Network ; September 2015 |
| Admin | `GCGSY4IOU7PG2QN2Z744ZVWMSZD5MYINLPKB5XSGQQECEU7NJBWUWO4Q` |
| Operator | `GA4LOTZNKXSNACOM56YWMIUXEER3NRD7ABJFSPHZP5VOUNROGJZIST7G` |
| Asset | native XLM SAC, `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Transactions, in order:

| Step | Hash |
|---|---|
| Upload wasm | `4bdeeb0be56365cb8b8e930c9950dd9ed6f1756d9f0d230027ab5bdb78083cd6` |
| Deploy | `1bd8dca12c57c62b3cf20929c4c1a2c774eb96914943a31c657eaf5bb032c1cf` |
| `initialize` | `71602a1386d44d185619ecca6c37b36365ba03cab683d03d7e3b55d6ed48e392` |
| `allow_mint` | `4f77f0209e1eb69e64cfe56142d4a1bad54e20afe0e7debe75bdf6a5592d4047` |
| `add_operator` | `bd511f602060620464c64748754f5c3149828eed17e4eeb5345ae82159afe80e` |
| `deposit` 100 XLM | `4f9b1a4d86a17b307dfa889db6c2f70358d2a4a6fca95844f4763a8c593c8451` |
| `release_funds` nonce 0, 30 XLM | `ef92b782cd3cccb464fa124d47af2b59e2bc4dc225c01f346b3e281e363ee34d` |
| `reset_smt_root` to generation 1 | `9adcf092400cafcdfa140c683ca17565f1305aec06db54df2970682ab419832e` |
| `release_funds` nonce 65536, 10 XLM | `920346e3f841a89143aaacb32c9d2b17da930525f6285eb9bcaa84f5d1344d18` |

Explorer: `https://stellar.expert/explorer/testnet/contract/CDORRV4DXCI73L23PG5IA7WAO4XH4WMGX3KOCCSOYAIMWW3A5C3DTJ36`

`total_locked` reads back `600000000` stroops, and `root` on a fresh instance
reads `8fe6b168…2beb`. The release proofs were taken verbatim from
`smt_vectors.json`, so that file is confirmed usable by an off-chain prover
against a live network.

Rejections observed on the same instance, each refused at simulation before
submission:

| Attempt | Error |
|---|---|
| Replay `release_funds` with a spent nonce | `#6` `InvalidSmtProof` |
| Replay `reset_smt_root` with a stale index | `#9` `UnexpectedTreeIndex` |
| `release_funds` with a generation-0 nonce after rotating to 1 | `#8` `WrongTreeGeneration` |

### Mainnet

Not deployed.
