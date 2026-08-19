# contract

Soroban contracts for **Cell Protocol**, a private payment channel system for
Stellar. This is the on-chain plane: it holds custody of deposited assets and
gates their release, so the balances mirrored inside the off-chain channel are
real and redeemable.

Individual in-channel transfers never touch these contracts — those live in the
backend.

## Layout

```
contracts/escrow    Custody and mint gating, the admin/operator control
                    surface, and the SHA256 sparse Merkle tree that stops a
                    withdrawal being settled twice.
        vectors/    Test vectors pinning the tree, for any off-chain prover.
```

## How it works

An operator locks an asset in escrow and mirrors the balance inside the channel.
Withdrawals are settled by an operator calling `release_funds` with a proof over
a sparse Merkle tree of spent withdrawal nonces — the tree is what makes a
withdrawal payable exactly once. Nonces carry their own tree generation, so a
nonce settled under an earlier tree can never be replayed after a rotation.

The contract verifies proofs. Generating them is the backend's job; see the
integration guide.

## Build and test

```sh
make check              # cargo fmt --check + clippy -D warnings
cargo test              # unit tests
stellar contract build  # compile to WASM
```

Requires the `wasm32v1-none` target: `rustup target add wasm32v1-none`.

## Docs

- [`docs/escrow-design.md`](docs/escrow-design.md) — storage layout and storage
  classes, the authorization flow, the tree parameters, and the open issues
- [`docs/deploy.md`](docs/deploy.md) — build, deploy and wiring, plus the current
  testnet deployment
- [`docs/integration.md`](docs/integration.md) — what the off-chain backend has
  to build to drive this contract

## Status

Deployed on testnet, not on mainnet. The off-chain prover does not exist yet, so
`release_funds` has no caller outside manual CLI invocations.
