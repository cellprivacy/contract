mod admin;
mod deposit;
mod hostile_token;
mod props;
mod release;
mod rotation;
mod smt_proofs;
mod sweep;
mod ttl;
mod upgrade;
mod vectors;
#[cfg(feature = "wasm-tests")]
mod wasm;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env};

use crate::contract::EscrowContractArgs;
use crate::{EscrowContract, EscrowContractClient};

/// A deployed, initialized escrow plus a Stellar asset to move through it.
///
/// All authorization is mocked; the tests that actually exercise the
/// authorization rules opt out per call with `mock_auths`.
pub struct Harness {
    pub env: Env,
    pub escrow: Address,
    pub admin: Address,
    pub mint: Address,
}

impl Harness {
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let escrow = env.register(EscrowContract, EscrowContractArgs::__constructor(&admin));
        let mint = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();

        Self {
            env,
            escrow,
            admin,
            mint,
        }
    }

    pub fn client(&self) -> EscrowContractClient<'_> {
        EscrowContractClient::new(&self.env, &self.escrow)
    }

    /// Issue `amount` of the primary test asset to `to`.
    pub fn fund(&self, to: &Address, amount: i128) {
        StellarAssetClient::new(&self.env, &self.mint).mint(to, &amount);
    }

    pub fn balance_of(&self, who: &Address) -> i128 {
        soroban_sdk::token::Client::new(&self.env, &self.mint).balance(who)
    }

    /// Register a second, independent asset under the same issuer.
    pub fn other_mint(&self) -> Address {
        self.env
            .register_stellar_asset_contract_v2(self.admin.clone())
            .address()
    }

    /// Fund a fresh depositor and deposit `amount` from them. The mint must
    /// already be allowed. Returns the depositor.
    pub fn deposit_from_new_user(&self, amount: i128) -> Address {
        let user = Address::generate(&self.env);
        self.fund(&user, amount);
        self.client().deposit(&user, &self.mint, &amount);
        user
    }
}

pub use crate::fuzzing::RefTree;
