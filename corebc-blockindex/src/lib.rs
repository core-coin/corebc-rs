#![doc = include_str!("../README.md")]
#![deny(unsafe_code, rustdoc::broken_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use crate::errors::{is_blocked_by_cloudflare_response, is_cloudflare_security_challenge};
use corebc_core::{
    abi::Address,
    types::{Network, H256},
};
use errors::BlockindexError;
use reqwest::{header, IntoUrl, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{borrow::Cow, fmt::Debug};
use tracing::{error, trace};
pub mod account;
pub mod block;
pub mod errors;
pub mod transaction;
pub mod utils;

pub(crate) type Result<T, E = BlockindexError> = std::result::Result<T, E>;

/// The Blockindex API client.
#[derive(Clone, Debug)]
pub struct Client {
    /// Client that executes HTTP requests
    client: reqwest::Client,
    /// Blockindex API endpoint like <https://blockindex.net/api/>
    blockindex_api_url: Url,
    /// Blockindex base endpoint like <https://blockindex.net/>
    blockindex_url: Url,
}

impl Client {
    /// Creates a `ClientBuilder` to configure a `Client`.
    ///
    /// This is the same as `ClientBuilder::default()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use corebc_core::types::Network;
    /// use corebc_blockindex::Client;
    /// let client = Client::builder().network(Network::Mainnet).unwrap().build().unwrap();
    /// ```
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Create a new client with the correct endpoints based on the network
    pub fn new(network: Network) -> Result<Self> {
        Client::builder().network(network)?.build()
    }

    pub fn blockindex_api_url(&self) -> &Url {
        &self.blockindex_api_url
    }

    pub fn blockindex_url(&self) -> &Url {
        &self.blockindex_url
    }

    /// Return the URL for the given block number
    pub fn block_url(&self, block: u64) -> String {
        format!("{}block/{block}", self.blockindex_url)
    }

    /// Return the URL for the given address
    pub fn address_url(&self, address: Address) -> String {
        format!("{}address/{address:?}", self.blockindex_url)
    }

    /// Return the URL for the given transaction hash
    pub fn transaction_url(&self, tx_hash: H256) -> String {
        format!("{}tx/{tx_hash:?}", self.blockindex_url)
    }

    /// Return the URL for the given token hash
    pub fn token_url(&self, token_hash: Address) -> String {
        format!("{}address/{token_hash:?}", self.blockindex_url)
    }

    /// Execute an GET request with parameters.
    async fn get_json<'a, T: DeserializeOwned, Q: Serialize>(
        &self,
        query: &Query<'a, Q>,
    ) -> Result<T> {
        let res = self.get(query).await?;
        self.sanitize_response(res)
    }

    /// Execute a GET request with parameters, without sanity checking the response.
    async fn get<'a, T: Serialize>(&self, query: &Query<'a, T>) -> Result<String> {
        trace!(target: "blockindex", "GET {}", self.blockindex_api_url);
        let response = self
            .client
            .get(
                String::from(self.blockindex_api_url.as_str()) +
                    query.module() +
                    "/" +
                    query.target(),
            )
            .header(header::ACCEPT, "application/json")
            .query(query.other())
            .send()
            .await?
            .text()
            .await?;
        Ok(response)
    }

    /// Perform sanity checks on a response and deserialize it into a [Result].
    fn sanitize_response<T: DeserializeOwned>(&self, res: impl AsRef<str>) -> Result<T> {
        let res = res.as_ref();
        let res: ResponseData<T> = serde_json::from_str(res).map_err(|err| {
            error!(target: "blockindex", ?res, "Failed to deserialize response: {}", err);
            if res == "Page not found" {
                BlockindexError::PageNotFound
            } else if is_blocked_by_cloudflare_response(res) {
                BlockindexError::BlockedByCloudflare
            } else if is_cloudflare_security_challenge(res) {
                BlockindexError::CloudFlareSecurityChallenge
            } else {
                BlockindexError::Serde(err)
            }
        })?;

        match res {
            ResponseData::Error { error } => Err(BlockindexError::ErrorResponse { error }),
            ResponseData::Success(res) => Ok(res),
        }
    }

    fn create_query<'a, T: Serialize>(
        &self,
        module: &'a str,
        target: &'a str,
        other: T,
    ) -> Query<'a, T> {
        Query { module: Cow::Borrowed(module), target: Cow::Borrowed(target), other }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ClientBuilder {
    /// Client that executes HTTP requests
    client: Option<reqwest::Client>,
    /// Blockindex API endpoint like <https://blockindex.net/api/>
    blockindex_api_url: Option<Url>,
    /// Blockindex base endpoint like <https://blockindex.net/>
    blockindex_url: Option<Url>,
}

// === impl ClientBuilder ===

impl ClientBuilder {
    /// Configures the blockindex url and api url for the given network
    ///
    /// # Errors
    ///
    /// Fails if the network is not supported by blockindex
    pub fn network(self, network: Network) -> Result<Self> {
        fn urls(
            api: impl IntoUrl,
            url: impl IntoUrl,
        ) -> (reqwest::Result<Url>, reqwest::Result<Url>) {
            (api.into_url(), url.into_url())
        }
        let (blockindex_api_url, blockindex_url) = network
            .blockindex_urls()
            .map(|(api, base)| urls(api, base))
            .ok_or_else(|| BlockindexError::NetworkNotSupported(network))?;
        self.with_api_url(blockindex_api_url?)?.with_url(blockindex_url?)
    }

    /// Configures the blockindex url
    ///
    /// # Errors
    ///
    /// Fails if the `blockindex_url` is not a valid `Url`
    pub fn with_url(mut self, blockindex_url: impl IntoUrl) -> Result<Self> {
        self.blockindex_url = Some(ensure_url(blockindex_url)?);
        Ok(self)
    }

    /// Configures the `reqwest::Client`
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    /// Configures the blockindex api url
    ///
    /// # Errors
    ///
    /// Fails if the `blockindex_api_url` is not a valid `Url`
    pub fn with_api_url(mut self, blockindex_api_url: impl IntoUrl) -> Result<Self> {
        self.blockindex_api_url = Some(ensure_url(blockindex_api_url)?);
        Ok(self)
    }

    /// Returns a Client that uses this ClientBuilder configuration.
    ///
    /// # Errors
    ///
    /// If the following required fields are missing:
    ///   - `blockindex_api_url`
    ///   - `blockindex_url`
    pub fn build(self) -> Result<Client> {
        let ClientBuilder { client, blockindex_api_url, blockindex_url } = self;

        let client = Client {
            client: client.unwrap_or_default(),
            blockindex_api_url: blockindex_api_url
                .ok_or_else(|| BlockindexError::Builder("blockindex api url".to_string()))?,
            blockindex_url: blockindex_url
                .ok_or_else(|| BlockindexError::Builder("blockindex url".to_string()))?,
        };
        Ok(client)
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ResponseData<T> {
    Success(T),
    Error { error: String },
}

/// The type that gets serialized as query
#[derive(Clone, Debug, Serialize)]
struct Query<'a, T: Serialize> {
    module: Cow<'a, str>,
    target: Cow<'a, str>,
    #[serde(flatten)]
    other: T,
}

impl<'a, T: Serialize> Query<'a, T> {
    fn module(&self) -> &str {
        &self.module
    }
    fn target(&self) -> &str {
        &self.target
    }
    fn other(&self) -> &T {
        &self.other
    }
}

/// Ensures that the url is well formatted to be used by the Client's functions that join paths.
fn ensure_url(url: impl IntoUrl) -> std::result::Result<Url, reqwest::Error> {
    let url_str = url.as_str();

    // ensure URL ends with `/`
    if url_str.ends_with('/') {
        url.into_url()
    } else {
        into_url(format!("{url_str}/"))
    }
}

/// This is a hack to work around `IntoUrl`'s sealed private functions, which can't be called
/// normally.
#[inline]
fn into_url(url: impl IntoUrl) -> std::result::Result<Url, reqwest::Error> {
    url.into_url()
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use corebc_core::types::{Address, Network, H256};

    #[test]
    fn test_api_paths() {
        let client = Client::new(Network::Devin).unwrap();
        assert_eq!(client.blockindex_api_url.as_str(), "https://devin.blockindex.net/api/v2/");

        assert_eq!(client.block_url(100), "https://devin.blockindex.net/block/100");
    }

    #[test]
    fn stringifies_block_url() {
        let blockindex = Client::new(Network::Mainnet).unwrap();
        let block: u64 = 1;
        let block_url: String = blockindex.block_url(block);
        assert_eq!(block_url, format!("https://blockindex.net/block/{block}"));
    }

    #[test]
    fn stringifies_address_url() {
        let blockindex = Client::new(Network::Mainnet).unwrap();
        let addr: Address = Address::zero();
        let address_url: String = blockindex.address_url(addr);
        assert_eq!(address_url, format!("https://blockindex.net/address/{addr:?}"));
    }

    #[test]
    fn stringifies_transaction_url() {
        let blockindex = Client::new(Network::Mainnet).unwrap();
        let tx_hash = H256::zero();
        let tx_url: String = blockindex.transaction_url(tx_hash);
        assert_eq!(tx_url, format!("https://blockindex.net/tx/{tx_hash:?}"));
    }

    #[test]
    fn stringifies_token_url() {
        let blockindex = Client::new(Network::Mainnet).unwrap();
        let token_hash = Address::zero();
        let token_url: String = blockindex.token_url(token_hash);
        assert_eq!(token_url, format!("https://blockindex.net/address/{token_hash:?}"));
    }
}
