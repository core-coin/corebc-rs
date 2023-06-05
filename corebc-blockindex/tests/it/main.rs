//! Etherscan integration tests

#![cfg(not(target_arch = "wasm32"))]

use corebc_blockindex::{errors::BlockindexError, Client};
use corebc_core::types::Network;
use std::{
    future::Future,
    time::{Duration, Instant},
};

mod account;
mod block;
mod transaction;
mod version;

/// Calls the function with a new Etherscan Client.
pub async fn run_with_client<F, Fut, T>(network: Network, f: F) -> T
where
    F: FnOnce(Client) -> Fut,
    Fut: Future<Output = T>,
{
    init_tracing();
    let client = match Client::new(network) {
        Ok(c) => c,
        Err(_) => Client::builder().network(network).unwrap().build().unwrap(),
    };
    run_at_least_duration(Duration::from_millis(250), f(client)).await
}

async fn run_at_least_duration<T>(duration: Duration, block: impl Future<Output = T>) -> T {
    let start = Instant::now();
    let output = block.await;
    let elapsed = start.elapsed();
    if elapsed < duration {
        tokio::time::sleep(duration - elapsed).await;
    }
    output
}

#[track_caller]
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
