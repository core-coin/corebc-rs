use corebc::{
    blockindex::Client,
    core::types::Network,
    middleware::energy_oracle::{EnergyOracle, ProviderOracle, EnergyCategory},
    providers::{Http, Provider},
};

#[tokio::main]
async fn main() {
    provider_oracle().await;
}


async fn provider_oracle() {
    const RPC_URL: &str = "https://xcbapi-arch-devin.coreblockchain.net/";
    let provider = Provider::<Http>::try_from(RPC_URL).unwrap();
    let oracle = ProviderOracle::new(provider);
    match oracle.fetch().await {
        Ok(energy_price) => println!("[Provider oracle]: Energy price is {energy_price:?}"),
        Err(e) => panic!("[Provider oracle]: Cannot estimate energy: {e:?}"),
    }
}

