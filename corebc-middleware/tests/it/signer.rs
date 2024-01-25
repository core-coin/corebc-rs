// CORETODO: Needs Shuttle
// use crate::{get_wallet, spawn_shuttle, spawn_shuttle_ws};
// use corebc_core::types::*;
// use corebc_middleware::{signer::SignerMiddleware, MiddlewareBuilder};
// use corebc_providers::{JsonRpcClient, Middleware};
// use corebc_signers::{LocalWallet, Signer};

// #[tokio::test]
// async fn send_eth() {
//     let (provider, shuttle) = spawn_shuttle();
//     let wallet = get_wallet(&shuttle, 0);
//     let address = wallet.address();
//     let provider = provider.with_signer(wallet);

//     let to = shuttle.addresses()[1];

//     // craft the transaction
//     let tx = TransactionRequest::new().to(to).value(10000);

//     let balance_before = provider.get_balance(address, None).await.unwrap();

//     // send it!
//     provider.send_transaction(tx, None).await.unwrap().await.unwrap().unwrap();

//     let balance_after = provider.get_balance(address, None).await.unwrap();

//     assert!(balance_before > balance_after);
// }

// #[tokio::test]
// async fn send_transaction_handles_tx_from_field() {
//     // launch shuttle
//     let (provider, shuttle) = spawn_shuttle_ws().await;

//     // grab 2 wallets
//     let signer: LocalWallet = shuttle.keys()[0].clone().into();
//     let other: LocalWallet = shuttle.keys()[1].clone().into();

//     // connect to the network
//     let provider =
//         SignerMiddleware::new_with_provider_network(provider, signer.clone()).await.unwrap();

//     // sending a TransactionRequest with a from field of None should result
//     // in a transaction from the signer address
//     let request_from_none = TransactionRequest::new();
//     let receipt =
//         provider.send_transaction(request_from_none,
// None).await.unwrap().await.unwrap().unwrap();     let sent_tx =
// provider.get_transaction(receipt.transaction_hash).await.unwrap().unwrap();

//     assert_eq!(sent_tx.from, signer.address());

//     // sending a TransactionRequest with the signer as the from address should
//     // result in a transaction from the signer address
//     let request_from_signer = TransactionRequest::new().from(signer.address());
//     let receipt =
//         provider.send_transaction(request_from_signer,
// None).await.unwrap().await.unwrap().unwrap();     let sent_tx =
// provider.get_transaction(receipt.transaction_hash).await.unwrap().unwrap();

//     assert_eq!(sent_tx.from, signer.address());

//     // sending a TransactionRequest with a from address that is not the signer
//     // should result in a transaction from the specified address
//     let request_from_other = TransactionRequest::new().from(other.address());
//     let receipt =
//         provider.send_transaction(request_from_other,
// None).await.unwrap().await.unwrap().unwrap();     let sent_tx =
// provider.get_transaction(receipt.transaction_hash).await.unwrap().unwrap();

//     assert_eq!(sent_tx.from, other.address());
// }

// async fn check_tx<P: JsonRpcClient + Clone>(
//     pending_tx: corebc_providers::PendingTransaction<'_, P>,
//     expected: u64,
// ) { let provider = pending_tx.provider(); let receipt = pending_tx.await.unwrap().unwrap(); let
//   tx = provider.get_transaction(receipt.transaction_hash).await.unwrap().unwrap();

//     let expected = U64::from(expected);
//     for ty in [receipt.transaction_type, tx.transaction_type] {
//         // legacy can be either None or Some(0)
//         if expected.is_zero() {
//             assert!(ty.is_none() || ty == Some(0.into()));
//         } else {
//             assert_eq!(ty, Some(expected));
//         }
//     }
// }
