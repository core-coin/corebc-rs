// pub mod app;
// pub mod types;

// use crate::Signer;
// use app::TrezorEthereum;
// use async_trait::async_trait;
// use corebc_core::types::{
//     transaction::{eip2718::TypedTransaction, cip712::Cip712},
//     Address, Signature,
// };
// use types::TrezorError;

// #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
// #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
// impl Signer for TrezorEthereum {
//     type Error = TrezorError;

//     /// Signs the hash of the provided message after prefixing it
//     async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
//         &self,
//         message: S,
//     ) -> Result<Signature, Self::Error> {
//         self.sign_message(message).await
//     }

//     /// Signs the transaction
//     async fn sign_transaction(&self, message: &TypedTransaction) -> Result<Signature,
// Self::Error> {         let mut tx_with_network = message.clone();
//         if tx_with_network.network_id().is_none() {
//             // in the case we don't have a network_id, let's use the signer network id instead
//             tx_with_network.set_network_id(self.network_id);
//         }
//         self.sign_tx(&tx_with_network).await
//     }

//     /// Signs a CIP712 derived struct
//     async fn sign_typed_data<T: Cip712 + Send + Sync>(
//         &self,
//         payload: &T,
//     ) -> Result<Signature, Self::Error> {
//         self.sign_typed_struct(payload).await
//     }

//     /// Returns the signer's Ethereum Address
//     fn address(&self) -> Address {
//         self.address
//     }

//     fn with_network_id<T: Into<u64>>(mut self, network_id: T) -> Self {
//         self.network_id = network_id.into();
//         self
//     }

//     fn network_id(&self) -> u64 {
//         self.network_id
//     }
// }
