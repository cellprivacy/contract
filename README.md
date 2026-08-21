# contract

Soroban contracts for Cell Protocol, a private payment channel system for
Stellar.

This is the on-chain side. It holds custody of deposited assets and controls
when they can be released, so the balances mirrored inside the off-chain channel
are backed by something real. Individual in-channel transfers never touch it;
those run in the backend.

## Layout

```
contracts/escrow          The escrow contract
contracts/escrow/vectors  Test vectors pinning the merkle tree
docs                      Design, deploy and integration notes
```

## How it works

An operator locks an asset in escrow and mirrors the balance inside the channel.
To settle a withdrawal, the operator calls `release_funds` with a proof against a
sparse merkle tree of spent withdrawal nonces. The tree is what makes each
withdrawal payable exactly once.

Nonces carry their own tree generation (`nonce / 65536`), so a nonce settled
under an earlier tree cannot be replayed after the tree rotates.

The contract only verifies proofs. Generating them is the backend's job, which
`docs/integration.md` describes.

## Entrypoints

| | |
|---|---|
| `initialize` | Claim the instance, set the admin |
| `set_new_admin` | Hand over admin rights, signed by both parties |
| `add_operator` / `remove_operator` | Manage who may settle withdrawals |
| `allow_mint` / `block_mint` | Open or freeze an asset |
| `deposit` | Lock an asset, called by the depositor |
| `release_funds` | Pay out against a proof, operator only |
| `reset_smt_root` | Start a new tree generation, operator only |
| `upgrade` | Replace the contract executable, admin only |

Views: `admin`, `root`, `tree_index`, `total_locked`, `is_operator`,
`is_allowed_mint`.

## Build and test

```sh
make check              # cargo fmt --check + clippy -D warnings
cargo test              # 74 unit tests
stellar contract build  # compile to WASM
```

The WASM build needs `rustup target add wasm32v1-none`. A pre-commit hook runs
fmt and clippy.

## Docs

| | |
|---|---|
| [`docs/escrow-design.md`](docs/escrow-design.md) | Storage layout, authorization, tree parameters, open issues |
| [`docs/deploy.md`](docs/deploy.md) | Build, deploy, wiring, and the current testnet deployment |
| [`docs/integration.md`](docs/integration.md) | What the off-chain backend has to build |

## Status

Deployed on testnet, not on mainnet. The off-chain prover does not exist yet, so
`release_funds` has no caller outside manual CLI invocations.
