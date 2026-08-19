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
```

## Build and test

```sh
make check              # cargo fmt --check + clippy -D warnings
cargo test              # unit tests
stellar contract build  # compile to WASM
```

## Design

[`docs/escrow-design.md`](docs/escrow-design.md) — storage layout and storage
classes, the authorization flow, the SMT parameters an off-chain prover has to
match, and the open issues.
