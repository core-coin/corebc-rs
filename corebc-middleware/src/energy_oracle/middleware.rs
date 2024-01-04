use super::{EnergyOracle, EnergyOracleError};
use async_trait::async_trait;
use corebc_core::types::{transaction::eip2718::TypedTransaction, *};
use corebc_providers::{Middleware, MiddlewareError as METrait, PendingTransaction};
use thiserror::Error;

/// Middleware used for fetching gas prices over an API instead of `eth_gasPrice`.
#[derive(Debug)]
pub struct EnergyOracleMiddleware<M, G> {
    inner: M,
    energy_oracle: G,
}

impl<M, G> EnergyOracleMiddleware<M, G>
where
    M: Middleware,
    G: EnergyOracle,
{
    pub fn new(inner: M, energy_oracle: G) -> Self {
        Self { inner, energy_oracle }
    }
}

#[derive(Debug, Error)]
pub enum MiddlewareError<M: Middleware> {
    #[error(transparent)]
    EnergyOracleError(#[from] EnergyOracleError),

    #[error("{0}")]
    MiddlewareError(M::Error),
}

impl<M: Middleware> METrait for MiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> MiddlewareError<M> {
        MiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            MiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M, G> Middleware for EnergyOracleMiddleware<M, G>
where
    M: Middleware,
    G: EnergyOracle,
{
    type Error = MiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    // OVERRIDEN METHODS

    fn inner(&self) -> &M {
        &self.inner
    }

    async fn fill_transaction(
        &self,
        tx: &mut TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<(), Self::Error> {
        match tx {
            TypedTransaction::Legacy(ref mut tx) => {
                if tx.energy_price.is_none() {
                    tx.energy_price = Some(self.get_energy_price().await?);
                }
            }
        };

        self.inner().fill_transaction(tx, block).await.map_err(METrait::from_err)
    }

    async fn get_energy_price(&self) -> Result<U256, Self::Error> {
        Ok(self.energy_oracle.fetch().await?)
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        let mut tx = tx.into();
        self.fill_transaction(&mut tx, block).await?;
        self.inner.send_transaction(tx, block).await.map_err(MiddlewareError::MiddlewareError)
    }
}
