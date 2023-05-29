use async_trait::async_trait;
use corebc_core::{
    types::*,
    utils::{parse_ether, Anvil},
};
use corebc_middleware::gas_oracle::{
    BlockNative, Etherchain, GasCategory, GasNow, GasOracle, GasOracleError, ProviderOracle, Result,
};
use corebc_providers::{Http, Middleware, Provider};

#[derive(Debug)]
struct FakeGasOracle {
    gas_price: U256,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl GasOracle for FakeGasOracle {
    async fn fetch(&self) -> Result<U256> {
        Ok(self.gas_price)
    }

    async fn estimate_eip1559_fees(&self) -> Result<(U256, U256)> {
        Err(GasOracleError::Eip1559EstimationNotSupported)
    }
}

// CORETODO: Needs Anvil
// #[tokio::test]
// async fn provider_using_gas_oracle() {
//     let anvil = Anvil::new().spawn();

//     let from = anvil.addresses()[0];

//     // connect to the network
//     let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();

//     // assign a gas oracle to use
//     let expected_gas_price = U256::from(1234567890_u64);
//     let gas_oracle = FakeGasOracle { gas_price: expected_gas_price };
//     let gas_price = gas_oracle.fetch().await.unwrap();
//     assert_eq!(gas_price, expected_gas_price);

//     let provider = GasOracleMiddleware::new(provider, gas_oracle);

//     // broadcast a transaction
//     let tx = TransactionRequest::new().from(from).to(Address::zero()).value(10000);
//     let tx_hash = provider.send_transaction(tx, None).await.unwrap();

//     let tx = provider.get_transaction(*tx_hash).await.unwrap().unwrap();
//     assert_eq!(tx.gas_price, Some(expected_gas_price));
// }

#[tokio::test]
async fn provider_oracle() {
    // spawn anvil and connect to it
    let anvil = Anvil::new().spawn();
    let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();

    // assert that provider.get_gas_price() and oracle.fetch() return the same value
    let expected_gas_price = provider.get_gas_price().await.unwrap();
    let provider_oracle = ProviderOracle::new(provider);
    let gas = provider_oracle.fetch().await.unwrap();
    assert_eq!(gas, expected_gas_price);
}

#[tokio::test]
async fn blocknative() {
    let gas_now_oracle = BlockNative::default();
    let gas_price = gas_now_oracle.fetch().await.unwrap();
    assert!(gas_price > U256::zero());
}

#[tokio::test]
#[ignore = "ETHGasStation is shutting down: https://twitter.com/ETHGasStation/status/1597341610777317376"]
#[allow(deprecated)]
async fn eth_gas_station() {
    let eth_gas_station_oracle = corebc_middleware::gas_oracle::EthGasStation::default();
    let gas_price = eth_gas_station_oracle.fetch().await.unwrap();
    assert!(gas_price > U256::zero());
}

#[tokio::test]
#[ignore = "Etherchain / beaconcha.in's `gasPriceOracle` API currently returns 404: https://www.etherchain.org/api/gasPriceOracle"]
async fn etherchain() {
    let etherchain_oracle = Etherchain::default();
    let gas_price = etherchain_oracle.fetch().await.unwrap();
    assert!(gas_price > U256::zero());
}

#[tokio::test]
async fn gas_now() {
    let gas_now_oracle = GasNow::default();
    let gas_price = gas_now_oracle.fetch().await.unwrap();
    assert!(gas_price > U256::zero());
}
