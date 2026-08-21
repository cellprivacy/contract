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

```sh
stellar keys generate <identity> --network testnet --fund
stellar contract deploy \
  --wasm target/wasm32v1-none/release/escrow.wasm \
  --source <identity> \
  --network testnet
```

Prints the contract id (`C...`). Record it, the wasm hash and both transaction
hashes: the upload and the deploy are separate transactions.

## Wire up

Order matters: nothing can be deposited until the asset is allowed, and nothing
can be released until an operator is registered.

```sh
C=<contract id>
ADMIN=$(stellar keys address <identity>)
NATIVE=$(stellar contract id asset --asset native --network testnet)

# 1. Claim the instance. Requires the incoming admin's own signature.
#    Do this immediately after deploying: until it lands, anyone can claim the
#    instance as their own admin. See escrow-design.md §8.
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

Built with soroban-sdk 27.

| | |
|---|---|
| Contract | `CAOWXO6MVNRP26XHPCK5KRQ44GUKCRYYCOLQ5PBHKACIAOIXKC6L7ZHR` |
| Wasm hash | `74b6c9326f04655d9bc0f71e856375bc5d2f682ecdce72dfab352f06e28315eb` |
| Network | Test SDF Network ; September 2015 |
| Admin | `GCGSY4IOU7PG2QN2Z744ZVWMSZD5MYINLPKB5XSGQQECEU7NJBWUWO4Q` |
| Operator | `GA4LOTZNKXSNACOM56YWMIUXEER3NRD7ABJFSPHZP5VOUNROGJZIST7G` |
| Asset | native XLM SAC, `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

| Step | Hash |
|---|---|
| Deploy | `435dbc376496922b2fc8181daceae591b40dfca197e41cdf475cb065074d2800` |
| `initialize` | `cba61f3a91311f8db1285d8277ce7ba56adc89bc2676aee298fc76799c555278` |
| `allow_mint` | `3e9b69877a66fb881889da9657a63ea9b3b96d1af5a15be10b63386533da8335` |
| `add_operator` | `f1b1dcc1122a60bbbffb0763bfd805ac2173f1800e67f5e18b80aad1593b02e3` |
| `deposit` 100 XLM | `c448e4634755b959297cd21a605c3f45de5e891fb6d12508f26f5d53e0a782e4` |
| `release_funds` nonce 0, 30 XLM | `50c52e332441bce0b88d6394a95265b1f5fa0b563ce856ead250d1c9868d9c20` |
| `reset_smt_root` to generation 1 | `1640e074cce75d53698e53429b2fdb260cae9cb94e2994accc8b4d1b11212f01` |
| `release_funds` nonce 65536, 10 XLM | `3d060aa54563f379f1abcada6a1e8e9b6e9bf1181daa83374d2e6143de9405ed` |

`total_locked` reads back `600000000` stroops. The release proofs were taken
verbatim from `smt_vectors.json`, so that file is confirmed usable by an
off-chain prover against a live network.

### Upgrade verified

A second instance, `CA3ZXQF2BKFH2KNNGQAYJAOJZ7N5XPUTQU5BS2PL5TCJT6XKKFPHI75E`,
was deployed, loaded with state, then upgraded from wasm
`74b6c932...15eb` to `0995fce6...2df4` in transaction
`69f6f7f1686c4242ca861d0476d0b68b75909e77a895f12b1b0ce0cbb7580594`.

Admin, operator set, mint permission, locked total and tree index all read back
unchanged afterwards, and a further deposit succeeded on the new executable.
An `upgrade` submitted from the operator was refused: simulation demanded the
admin key.

### Superseded

`CDORRV4DXCI73L23PG5IA7WAO4XH4WMGX3KOCCSOYAIMWW3A5C3DTJ36` was the previous
record, built with soroban-sdk 26 before the `upgrade` entrypoint existed. It
cannot be upgraded and is left in place only as history.

### Mainnet

Not deployed.
