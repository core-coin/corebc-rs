#![cfg(not(target_arch = "wasm32"))]

use corebc_core::utils::{Shuttle, ShuttleInstance};
use corebc_providers::{Http, Provider};
use std::time::Duration;

#[cfg(feature = "ipc")]
use corebc_providers::Ipc;
#[cfg(feature = "ipc")]
use tempfile::NamedTempFile;

#[cfg(feature = "ws")]
use corebc_providers::Ws;

mod provider;

mod txpool;

mod ws_errors;

/// Spawns Shuttle and instantiates an Http provider.
pub fn spawn_shuttle() -> (Provider<Http>, ShuttleInstance) {
    let shuttle = Shuttle::new().block_time(1u64).spawn();
    let provider = Provider::<Http>::try_from(shuttle.endpoint())
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, shuttle)
}

/// Spawns Shuttle and instantiates a Ws provider.
#[cfg(feature = "ws")]
pub async fn spawn_shuttle_ws() -> (Provider<Ws>, ShuttleInstance) {
    let shuttle = Shuttle::new().block_time(1u64).spawn();
    let provider = Provider::<Ws>::connect(shuttle.ws_endpoint())
        .await
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, shuttle)
}

/// Spawns Shuttle and instantiates a Ipc provider.
#[cfg(feature = "ipc")]
pub async fn spawn_shuttle_ipc() -> (Provider<Ipc>, ShuttleInstance, NamedTempFile) {
    let ipc = NamedTempFile::new().unwrap();
    let shuttle =
        Shuttle::new().block_time(1u64).arg("--ipc").arg(ipc.path().display().to_string()).spawn();
    let provider = Provider::<Ipc>::connect_ipc(ipc.path())
        .await
        .unwrap()
        .interval(Duration::from_millis(50u64));
    (provider, shuttle, ipc)
}
