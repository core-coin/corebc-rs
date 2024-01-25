#![allow(clippy::extra_unused_type_parameters)]
#![cfg(not(target_arch = "wasm32"))]

use corebc_core::utils::{Shuttle, ShuttleInstance};
use corebc_providers::{Http, Provider, Ws};
use corebc_signers::{LocalWallet, Signer};
use std::time::Duration;

mod builder;

mod energy_escalator;

mod energy_oracle;

mod signer;

mod nonce_manager;

mod stack;

mod transformer;

/// Spawns Shuttle and instantiates an Http provider.
pub fn spawn_shuttle() -> (Provider<Http>, ShuttleInstance) {
    let shuttle = Shuttle::new().block_time(1u64).spawn();
    let provider = Provider::<Http>::try_from(shuttle.endpoint())
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, shuttle)
}

/// Spawns Shuttle and instantiates a Ws provider.
pub async fn spawn_shuttle_ws() -> (Provider<Ws>, ShuttleInstance) {
    let shuttle = Shuttle::new().block_time(1u64).spawn();
    let provider = Provider::<Ws>::connect(shuttle.ws_endpoint())
        .await
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, shuttle)
}

/// Gets `idx` wallet from the given shuttle instance.
pub fn get_wallet(shuttle: &ShuttleInstance, idx: usize) -> LocalWallet {
    LocalWallet::from(shuttle.keys()[idx].clone()).with_network_id(shuttle.network_id())
}
