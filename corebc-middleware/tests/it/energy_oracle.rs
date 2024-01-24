use async_trait::async_trait;
use corebc_core::{types::*, utils::Shuttle};
use corebc_middleware::energy_oracle::{
    EnergyOracle, EnergyOracleError, Etherchain, ProviderOracle, Result,
};
use corebc_providers::{Http, Middleware, Provider};

#[derive(Debug)]
struct FakeEnergyOracle {
    energy_price: U256,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl EnergyOracle for FakeEnergyOracle {
    async fn fetch(&self) -> Result<U256> {
        Ok(self.energy_price)
    }
}

// CORETODO: Needs Shuttle
// #[tokio::test]
// async fn provider_using_energy_oracle() {
//     let shuttle = Shuttle::new().spawn();

//     let from = shuttle.addresses()[0];

//     // connect to the network
//     let provider = Provider::<Http>::try_from(shuttle.endpoint()).unwrap();

//     // assign a gas oracle to use
//     let expected_energy_price = U256::from(1234567890_u64);
//     let energy_oracle = FakeEnergyOracle { energy_price: expected_energy_price };
//     let energy_price = energy_oracle.fetch().await.unwrap();
//     assert_eq!(energy_price, expected_energy_price);

//     let provider = EnergyOracleMiddleware::new(provider, energy_oracle);

//     // broadcast a transaction
//     let tx = TransactionRequest::new().from(from).to(Address::zero()).value(10000);
//     let tx_hash = provider.send_transaction(tx, None).await.unwrap();

//     let tx = provider.get_transaction(*tx_hash).await.unwrap().unwrap();
//     assert_eq!(tx.energy_price, Some(expected_energy_price));
// }

#[ignore = "Won't work until shuttle is fixed"]
#[tokio::test]
async fn provider_oracle() {
    // spawn shuttle and connect to it
    let shuttle = Shuttle::new().spawn();
    let provider = Provider::<Http>::try_from(shuttle.endpoint()).unwrap();

    // assert that provider.get_energy_price() and oracle.fetch() return the same value
    let expected_energy_price = provider.get_energy_price().await.unwrap();
    let provider_oracle = ProviderOracle::new(provider);
    let gas = provider_oracle.fetch().await.unwrap();
    assert_eq!(gas, expected_energy_price);
}
