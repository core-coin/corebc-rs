#![allow(clippy::extra_unused_type_parameters)]
#![cfg(not(target_arch = "wasm32"))]

use corebc_core::utils::{Anvil, AnvilInstance};
use corebc_providers::{Http, Provider, Ws};
use corebc_signers::{LocalWallet, Signer};
use std::time::Duration;

mod builder;

mod gas_escalator;

mod gas_oracle;

mod signer;

mod nonce_manager;

mod stack;

mod transformer;

/// Spawns Anvil and instantiates an Http provider.
pub fn spawn_anvil() -> (Provider<Http>, AnvilInstance) {
    let anvil = Anvil::new().block_time(1u64).spawn();
    let provider = Provider::<Http>::try_from(anvil.endpoint())
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, anvil)
}

/// Spawns Anvil and instantiates a Ws provider.
pub async fn spawn_anvil_ws() -> (Provider<Ws>, AnvilInstance) {
    let anvil = Anvil::new().block_time(1u64).spawn();
    let provider = Provider::<Ws>::connect(anvil.ws_endpoint())
        .await
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, anvil)
}

/// Gets `idx` wallet from the given anvil instance.
pub fn get_wallet(anvil: &AnvilInstance, idx: usize) -> LocalWallet {
    LocalWallet::from(anvil.keys()[idx].clone()).with_network_id(anvil.network_id())
}
