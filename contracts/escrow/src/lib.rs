#![no_std]
use soroban_sdk::contractmeta;

// Baked into the wasm, so the revision an instance is running can be read off
// chain instead of trusted from a deployment table. With `upgrade` live that is
// the only way to tell one instance from another.
contractmeta!(key = "name", val = "cell-protocol-escrow");
contractmeta!(key = "storage_version", val = "1");
contractmeta!(key = "repo", val = "github.com/cellprivacy/contract");

mod contract;
mod error;
mod event;
mod smt;
mod storage;
mod storage_types;

pub use contract::{EscrowContract, EscrowContractClient};
pub use error::EscrowError;
pub use event::{AdminChanged, Deposit, MintSet, OperatorSet, Release, Rotate, Swept, Upgraded};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test;
