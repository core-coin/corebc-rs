use corebc_core::{rand::thread_rng, utils::NetworkType};
use corebc_middleware::{
    gas_escalator::{Frequency, GasEscalatorMiddleware, GeometricGasPrice},
    gas_oracle::{GasCategory, GasNow, GasOracleMiddleware},
    nonce_manager::NonceManagerMiddleware,
    signer::SignerMiddleware,
};
use corebc_providers::{Middleware, Provider};
use corebc_signers::{LocalWallet, Signer};

#[tokio::test]
async fn mock_with_middleware() {
    let (provider, mock) = Provider::mocked();

    // add a bunch of middlewares
    let gas_oracle = GasNow::new().category(GasCategory::SafeLow);
    let signer = LocalWallet::new(&mut thread_rng(), NetworkType::Mainnet);
    let address = signer.address();
    let escalator = GeometricGasPrice::new(1.125, 60u64, None::<u64>);
    let provider = GasEscalatorMiddleware::new(provider, escalator, Frequency::PerBlock);
    let provider = GasOracleMiddleware::new(provider, gas_oracle);
    let provider = SignerMiddleware::new(provider, signer);
    let provider = NonceManagerMiddleware::new(provider, address);

    // push a response
    use corebc_core::types::U64;
    mock.push(U64::from(12u64)).unwrap();
    let blk = provider.get_block_number().await.unwrap();
    assert_eq!(blk.as_u64(), 12);

    // now that the response is gone, there's nothing left
    // TODO: This returns:
    // MiddlewareError(
    // MiddlewareError(
    // MiddlewareError(
    // MiddlewareError(
    // JsonRpcClientError(EmptyResponses)
    // ))))
    // Can we flatten it in any way? Maybe inherent to the middleware
    // infrastructure
    provider.get_block_number().await.unwrap_err();

    // 2 calls were made
    mock.assert_request("eth_blockNumber", ()).unwrap();
    mock.assert_request("eth_blockNumber", ()).unwrap();
    mock.assert_request("eth_blockNumber", ()).unwrap_err();
}

// CORETODO: Needs Anvil
// #[tokio::test]
// async fn can_stack_middlewares() {
//     let anvil = Anvil::new().block_time(5u64).spawn();
//     let gas_oracle = GasNow::new().category(GasCategory::SafeLow);
//     let signer: LocalWallet = anvil.keys()[0].clone().into();
//     let address = signer.address();

//     // the base provider
//     let provider = Arc::new(Provider::<Http>::try_from(anvil.endpoint()).unwrap());
//     let network_id = provider.get_networkid().await.unwrap().as_u64();
//     let signer = signer.with_network_id(network_id);

//     // the Gas Price escalator middleware is the first middleware above the provider,
//     // so that it receives the transaction last, after all the other middleware
//     // have modified it accordingly
//     let escalator = GeometricGasPrice::new(1.125, 60u64, None::<u64>);
//     let provider = GasEscalatorMiddleware::new(provider, escalator, Frequency::PerBlock);

//     // The gas price middleware MUST be below the signing middleware for things to work
//     let provider = GasOracleMiddleware::new(provider, gas_oracle);

//     // The signing middleware signs txs
//     use std::sync::Arc;
//     let provider = Arc::new(SignerMiddleware::new(provider, signer));

//     // The nonce manager middleware MUST be above the signing middleware so that it overrides
//     // the nonce and the signer does not make any eth_getTransaction count calls
//     let provider = NonceManagerMiddleware::new(provider, address);

//     let tx = TransactionRequest::new();
//     let mut pending_txs = Vec::new();
//     for _ in 0..10 {
//         let pending = provider.send_transaction(tx.clone(), None).await.unwrap();
//         let hash = *pending;
//         let gas_price = provider.get_transaction(hash).await.unwrap().unwrap().gas_price;
//         dbg!(gas_price);
//         pending_txs.push(pending);
//     }

//     let receipts = futures_util::future::join_all(pending_txs);
//     dbg!(receipts.await);
// }
