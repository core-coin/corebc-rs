use super::{EneryOracle, EneryOracleError, Result};
use async_trait::async_trait;
use corebc_core::types::U256;
use corebc_providers::Middleware;
use std::fmt::Debug;

/// Gas oracle from a [`Middleware`] implementation such as an
/// Ethereum RPC provider.
#[derive(Clone, Debug)]
#[must_use]
pub struct ProviderOracle<M: Middleware> {
    provider: M,
}

impl<M: Middleware> ProviderOracle<M> {
    pub fn new(provider: M) -> Self {
        Self { provider }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M: Middleware> EneryOracle for ProviderOracle<M>
where
    M::Error: 'static,
{
    async fn fetch(&self) -> Result<U256> {
        self.provider
            .get_gas_price()
            .await
            .map_err(|err| EneryOracleError::ProviderError(Box::new(err)))
    }
}
