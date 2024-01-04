pub mod etherchain;
pub use etherchain::Etherchain;

pub mod middleware;
pub use middleware::{EnergyOracleMiddleware, MiddlewareError};

pub mod median;
pub use median::Median;

pub mod cache;
pub use cache::Cache;

pub mod provider_oracle;
pub use provider_oracle::ProviderOracle;

use async_trait::async_trait;
use auto_impl::auto_impl;
use corebc_core::types::U256;
use reqwest::Error as ReqwestError;
use std::{error::Error, fmt::Debug};
use thiserror::Error;

pub(crate) const GWEI_TO_WEI: u64 = 1_000_000_000;
pub(crate) const GWEI_TO_WEI_U256: U256 = U256([GWEI_TO_WEI, 0, 0, 0]);

pub type Result<T, E = EnergyOracleError> = std::result::Result<T, E>;

// Generic [`EnergyOracle`] gas price categories.
#[derive(Clone, Copy, Default, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GasCategory {
    SafeLow,
    #[default]
    Standard,
    Fast,
    Fastest,
}

// Error thrown by a [`EnergyOracle`].
#[derive(Debug, Error)]
pub enum EnergyOracleError {
    // An internal error in the HTTP request made from the underlying
    // gas oracle
    #[error(transparent)]
    HttpClientError(#[from] ReqwestError),

    // An error decoding JSON response from gas oracle
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    // An error with oracle response type
    #[error("invalid oracle response")]
    InvalidResponse,

    // An internal error in the Blockindex client request made from the underlying
    // gas oracle
    #[error(transparent)]
    BlockindexError(#[from] corebc_blockindex::errors::BlockindexError),

    // An internal error thrown when the required gas category is not
    // supported by the gas oracle API
    #[error("gas category not supported")]
    GasCategoryNotSupported,

    #[error("None of the oracles returned a value")]
    NoValues,

    #[error("Network is not supported by the oracle")]
    UnsupportedNetwork,

    // Error thrown when the provider failed.
    #[error("Provider error: {0}")]
    ProviderError(#[from] Box<dyn Error + Send + Sync>),
    #[error("Failed to parse gas values: {0}")]
    ConversionError(#[from] corebc_core::utils::ConversionError),
}

// An Ethereum gas price oracle.
//
// # Example
//
// ```no_run
// use corebc_core::types::U256;
// use corebc_middleware::energy_oracle::{GasCategory, GasNow, EnergyOracle};
//
// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
// let oracle = GasNow::default().category(GasCategory::SafeLow);
// let energy_price = oracle.fetch().await?;
// assert!(energy_price > U256::zero());
// # Ok(())
// # }
// ```
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[auto_impl(&, Box, Arc)]
pub trait EnergyOracle: Send + Sync + Debug {
    // Makes an asynchronous HTTP query to the underlying [`EnergyOracle`] to fetch the current gas
    // price estimate.
    //
    // # Example
    //
    // ```no_run
    // use corebc_core::types::U256;
    // use corebc_middleware::energy_oracle::{GasCategory, GasNow, EnergyOracle};
    //
    // # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
    // let oracle = GasNow::default().category(GasCategory::SafeLow);
    // let energy_price = oracle.fetch().await?;
    // assert!(energy_price > U256::zero());
    // # Ok(())
    // # }
    // ```
    async fn fetch(&self) -> Result<U256>;
}

#[inline]
#[doc(hidden)]
pub(crate) fn from_gwei_f64(gwei: f64) -> U256 {
    corebc_core::types::u256_from_f64_saturating(gwei) * GWEI_TO_WEI_U256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gwei_wei_constants() {
        let as_u256: U256 = GWEI_TO_WEI.into();
        assert_eq!(as_u256, GWEI_TO_WEI_U256);
        assert_eq!(GWEI_TO_WEI_U256.as_u64(), GWEI_TO_WEI);
    }
}
