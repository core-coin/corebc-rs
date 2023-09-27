#![doc = include_str!("../README.md")]
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]
#![deny(unsafe_code, rustdoc::broken_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod ext;
pub use ext::*;

mod rpc;
pub use rpc::*;

mod toolbox;
pub use toolbox::*;

/// Crate utilities and type aliases
mod utils;
pub use utils::{interval, maybe, EscalationPolicy};

/// Errors
mod errors;
pub use errors::{MiddlewareError, ProviderError, RpcError};

mod stream;
pub use futures_util::StreamExt;
pub use stream::{
    tx_stream::TransactionStream, FilterWatcher, DEFAULT_LOCAL_POLL_INTERVAL, DEFAULT_POLL_INTERVAL,
};

mod middleware;
pub use middleware::Middleware;

#[allow(deprecated)]
pub use test_provider::{DEVIN, MAINNET};

#[allow(missing_docs)]
/// Pre-instantiated Infura HTTP clients which rotate through multiple API keys
/// to prevent rate limits
pub mod test_provider {
    use super::*;
    use crate::Http;
    use once_cell::sync::Lazy;
    use std::{convert::TryFrom, iter::Cycle, slice::Iter, sync::Mutex};

    // List of infura keys to rotate through so we don't get rate limited
    const INFURA_KEYS: &[&str] = &["15e8aaed6f894d63a0f6a0206c006cdd"];

    pub static MAINNET: Lazy<TestProvider> = Lazy::new(|| TestProvider::new("mainnet"));
    pub static DEVIN: Lazy<TestProvider> = Lazy::new(|| TestProvider::new("devin"));

    #[derive(Debug)]
    pub struct TestProvider {
        network: String,
    }

    impl TestProvider {
        pub fn new(network: impl Into<String>) -> Self {
            Self { network: network.into() }
        }

        pub fn url(&self) -> String {
            let Self { network } = self;
            match network.as_str() {
                "devin" => String::from("https://xcbapi.corecoin.cc/"),
                "mainnet" => String::from("https://xcbapi.coreblockchain.net/"),
                _ => panic!("Invalid Network. Only devin and mainnet are availible"),
            }
        }

        pub fn provider(&self) -> Provider<Http> {
            Provider::try_from(self.url().as_str()).unwrap()
        }

        #[cfg(feature = "ws")]
        pub async fn ws(&self) -> Provider<crate::Ws> {
            Provider::connect("wss://xcbapi.coreblockchain.net").await.unwrap()
        }
    }
}
