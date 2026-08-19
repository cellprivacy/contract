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
| Contract | `CAIHS2Q5FJYEHNDNH2RXFPMIKIQXUOKZ7KYPHHM2SEPNPGIQ7RRGYS4D` |
| Wasm hash | `c42ba80ba0e26a0f02ba5466fe44e2a4140da600dbbe73746ec97eaba0db7a8f` |
| Network | Test SDF Network ; September 2015 |
| Admin | `GCGSY4IOU7PG2QN2Z744ZVWMSZD5MYINLPKB5XSGQQECEU7NJBWUWO4Q` |
| Operator | `GA4LOTZNKXSNACOM56YWMIUXEER3NRD7ABJFSPHZP5VOUNROGJZIST7G` |
| Asset | native XLM SAC, `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Transactions, in order:

| Step | Hash |
|---|---|
| Upload wasm | `805882d546241ab08eb99f7d8b09a74a1d71b878bf77f9203d6dd44cc8a77a42` |
| Deploy | `02c6f993d93c1a723c8427e9b385dfaecc110bdda5b01340a02977a012a47b67` |
| `initialize` | `e4749f451c1e3ac16d05445a926b8f1d420320966ac0f85b6710151367b43a59` |
| `allow_mint` | `8386aede05602222ead516adbff1a4575c71ade59aec6ccdcc4997f966bda4a8` |
| `add_operator` | `b500d71e853be0d093d63a3c0ee85580d5d86388b581de633d3a8555a2987c5e` |
| `deposit` 100 XLM | `71823507bf56d8e96bafaf1131a6f330b4d49c0857a9e5f1f213d46221fd07f3` |
| `release_funds` nonce 0, 30 XLM | `693ac5aa72a86eafd1d0b546d71fc765f2ecd85ac34856fdc1be899bc556b2c6` |
| `reset_smt_root` to generation 1 | `72e1f87a1264c4a6dfe42c71c4e8a586b9202482e60bef3ed2281bff1ece3a51` |
| `release_funds` nonce 65536, 10 XLM | `7e77e6afa0e1155ad048534b9a9887e6f4e36e495173b64a5409a5d77552ca8e` |

Explorer: `https://stellar.expert/explorer/testnet/contract/CAIHS2Q5FJYEHNDNH2RXFPMIKIQXUOKZ7KYPHHM2SEPNPGIQ7RRGYS4D`

The release proofs were taken verbatim from `smt_vectors.json`, so that file is
confirmed usable by an off-chain prover against a live network.

Rejections observed on the same instance, each simulated and refused before
submission:

| Attempt | Error |
|---|---|
| Replay `release_funds` with a spent nonce | `#6` `InvalidSmtProof` |
| Replay `reset_smt_root` with a stale index | `#9` `UnexpectedTreeIndex` |
| `release_funds` with a nonce from generation 0 after rotating to 1 | `#8` `WrongTreeGeneration` |

### Mainnet

Not deployed.
