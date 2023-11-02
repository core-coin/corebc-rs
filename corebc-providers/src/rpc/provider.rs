use corebc_core::types::SyncingStatus;

use crate::{
    call_raw::CallBuilder,
    errors::ProviderError,
    ext::{ens, erc},
    rpc::pubsub::{PubsubClient, SubscriptionStream},
    stream::{FilterWatcher, DEFAULT_LOCAL_POLL_INTERVAL, DEFAULT_POLL_INTERVAL},
    utils::maybe,
    Http as HttpProvider, JsonRpcClient, JsonRpcClientWrapper, LogQuery, MiddlewareError,
    MockProvider, NodeInfo, PeerInfo, PendingTransaction, QuorumProvider, RwClient,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::{HttpRateLimitRetryPolicy, RetryClient};

pub use crate::Middleware;

use async_trait::async_trait;

use corebc_core::{
    abi::{self, Detokenize, ParamType},
    types::{
        transaction::eip2718::TypedTransaction, Address, Block, BlockId, BlockNumber, BlockTrace,
        Bytes, EIP1186ProofResponse, Filter, FilterBlockOption, GoCoreDebugTracingCallOptions,
        GoCoreDebugTracingOptions, GoCoreTrace, Log, NameOrAddress, Network, Selector, Signature,
        Trace, TraceFilter, TraceType, Transaction, TransactionReceipt, TransactionRequest, TxHash,
        TxpoolContent, TxpoolInspect, TxpoolStatus, H256, U256, U64,
    },
    utils,
};
use futures_util::{lock::Mutex, try_join};
use hex::FromHex;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::VecDeque, convert::TryFrom, fmt::Debug, str::FromStr, sync::Arc, time::Duration,
};
use tracing::trace;
use tracing_futures::Instrument;
use url::{ParseError, Url};

/// Node Clients
#[derive(Copy, Clone)]
pub enum NodeClient {
    /// GoCore
    GoCore,
}

impl FromStr for NodeClient {
    type Err = ProviderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split('/').next().unwrap().to_lowercase().as_str() {
            "gocore" => Ok(NodeClient::GoCore),
            _ => Err(ProviderError::UnsupportedNodeClient),
        }
    }
}

/// An abstract provider for interacting with the [Ethereum JSON RPC
/// API](https://github.com/ethereum/wiki/wiki/JSON-RPC). Must be instantiated
/// with a data transport which implements the [`JsonRpcClient`](trait@crate::JsonRpcClient) trait
/// (e.g. [HTTP](crate::Http), Websockets etc.)
///
/// # Example
///
/// ```no_run
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// use corebc_providers::{Middleware, Provider, Http};
/// use std::convert::TryFrom;
///
/// let provider = Provider::<Http>::try_from(
///     "https://eth.llamarpc.com"
/// ).expect("could not instantiate HTTP Provider");
///
/// let block = provider.get_block(100u64).await?;
/// println!("Got block: {}", serde_json::to_string(&block)?);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Provider<P> {
    inner: P,
    ens: Option<Address>,
    interval: Option<Duration>,
    from: Option<Address>,
    /// Node client hasn't been checked yet = `None`
    /// Unsupported node client = `Some(None)`
    /// Supported node client = `Some(Some(NodeClient))`
    _node_client: Arc<Mutex<Option<NodeClient>>>,
}

impl<P> AsRef<P> for Provider<P> {
    fn as_ref(&self) -> &P {
        &self.inner
    }
}

/// Types of filters supported by the JSON-RPC.
#[derive(Clone, Debug)]
pub enum FilterKind<'a> {
    /// `xcb_newBlockFilter`
    Logs(&'a Filter),

    /// `xcb_newBlockFilter` filter
    NewBlocks,

    /// `xcb_newPendingTransactionFilter` filter
    PendingTransactions,
}

// JSON RPC bindings
impl<P: JsonRpcClient> Provider<P> {
    /// Instantiate a new provider with a backend.
    pub fn new(provider: P) -> Self {
        Self {
            inner: provider,
            ens: None,
            interval: None,
            from: None,
            _node_client: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns the type of node we're connected to, while also caching the value for use
    /// in other node-specific API calls, such as the get_block_receipts call.
    pub async fn node_client(&self) -> Result<NodeClient, ProviderError> {
        let mut node_client = self._node_client.lock().await;

        if let Some(node_client) = *node_client {
            Ok(node_client)
        } else {
            let client_version = self.client_version().await?;
            let client_version = match client_version.parse::<NodeClient>() {
                Ok(res) => res,
                Err(_) => return Err(ProviderError::UnsupportedNodeClient),
            };
            *node_client = Some(client_version);
            Ok(client_version)
        }
    }

    #[must_use]
    /// Set the default sender on the provider
    pub fn with_sender(mut self, address: impl Into<Address>) -> Self {
        self.from = Some(address.into());
        self
    }

    /// Make an RPC request via the internal connection, and return the result.
    pub async fn request<T, R>(&self, method: &str, params: T) -> Result<R, ProviderError>
    where
        T: Debug + Serialize + Send + Sync,
        R: Serialize + DeserializeOwned + Debug + Send,
    {
        let span =
            tracing::trace_span!("rpc", method = method, params = ?serde_json::to_string(&params)?);
        // https://docs.rs/tracing/0.1.22/tracing/span/struct.Span.html#in-asynchronous-code
        let res = async move {
            trace!("tx");
            let res: R = self.inner.request(method, params).await.map_err(Into::into)?;
            trace!(rx = ?serde_json::to_string(&res)?);
            Ok::<_, ProviderError>(res)
        }
        .instrument(span)
        .await?;
        Ok(res)
    }

    async fn get_block_gen<Tx: Default + Serialize + DeserializeOwned + Debug + Send>(
        &self,
        id: BlockId,
        include_txs: bool,
    ) -> Result<Option<Block<Tx>>, ProviderError> {
        let include_txs = utils::serialize(&include_txs);

        Ok(match id {
            BlockId::Hash(hash) => {
                let hash = utils::serialize(&hash);
                self.request("xcb_getBlockByHash", [hash, include_txs]).await?
            }
            BlockId::Number(num) => {
                let num = utils::serialize(&num);
                self.request("xcb_getBlockByNumber", [num, include_txs]).await?
            }
        })
    }

    /// Analogous to [`Middleware::call`], but returns a [`CallBuilder`] that can either be
    /// `.await`d or used to override the parameters sent to `xcb_call`.
    ///
    /// See the [`call_raw::spoof`] for functions to construct state override parameters.
    ///
    /// Note: this method _does not_ send a transaction from your account
    ///
    /// [`call_raw::spoof`]: crate::call_raw::spoof
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use corebc_core::{
    /// #     types::{Address, TransactionRequest, H256},
    /// #     utils::{parse_ether, GoCore},
    /// # };
    /// # use corebc_providers::{Provider, Http, Middleware, call_raw::{RawCall, spoof}};
    /// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
    /// let gocore = GoCore::new().spawn();
    /// let provider = Provider::<Http>::try_from(gocore.endpoint()).unwrap();
    ///
    /// let adr1: Address = "0x00006fC21092DA55B392b045eD78F4732bff3C580e2c".parse()?;
    /// let adr2: Address = "0x0000295a70b2de5e3953354a6a8344e616ed314d7251".parse()?;
    /// let pay_amt = parse_ether(1u64)?;
    ///
    /// // Not enough ether to pay for the transaction
    /// let tx = TransactionRequest::pay(adr2, pay_amt).from(adr1).into();
    ///
    /// // override the sender's balance for the call
    /// let mut state = spoof::balance(adr1, pay_amt * 2);
    /// provider.call_raw(&tx).state(&state).await?;
    /// # Ok(()) }
    /// ```
    pub fn call_raw<'a>(&'a self, tx: &'a TypedTransaction) -> CallBuilder<'a, P> {
        CallBuilder::new(self, tx)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<P: JsonRpcClient> Middleware for Provider<P> {
    type Error = ProviderError;
    type Provider = P;
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        unreachable!("There is no inner provider here")
    }

    fn provider(&self) -> &Provider<Self::Provider> {
        self
    }

    fn convert_err(p: ProviderError) -> Self::Error {
        // no conversion necessary
        p
    }

    fn default_sender(&self) -> Option<Address> {
        self.from
    }

    async fn client_version(&self) -> Result<String, Self::Error> {
        self.request("web3_clientVersion", ()).await
    }

    async fn fill_transaction(
        &self,
        tx: &mut TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<(), Self::Error> {
        if let Some(default_sender) = self.default_sender() {
            if tx.from().is_none() {
                tx.set_from(default_sender);
            }
        }

        // TODO: Join the name resolution and energy price future

        // set the ENS name
        if let Some(NameOrAddress::Name(ref ens_name)) = tx.to() {
            let addr = self.resolve_name(ens_name).await?;
            tx.set_to(addr);
        }

        // fill energy price
        match tx {
            TypedTransaction::Legacy(_) => {
                let energy_price = maybe(tx.energy_price(), self.get_energy_price()).await?;
                tx.set_energy_price(energy_price);
            }
        }

        // Set energy to estimated value only if it was not set by the caller,
        // even if the access list has been populated and saves energy
        if tx.energy().is_none() {
            let energy_estimate = self.estimate_energy(tx, block).await?;
            tx.set_energy(energy_estimate);
        }

        Ok(())
    }

    async fn get_block_number(&self) -> Result<U64, ProviderError> {
        self.request("xcb_blockNumber", ()).await
    }

    async fn get_block<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
    ) -> Result<Option<Block<TxHash>>, Self::Error> {
        self.get_block_gen(block_hash_or_number.into(), false).await
    }

    async fn get_block_with_txs<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
    ) -> Result<Option<Block<Transaction>>, ProviderError> {
        self.get_block_gen(block_hash_or_number.into(), true).await
    }

    async fn get_uncle_count<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
    ) -> Result<U256, Self::Error> {
        let id = block_hash_or_number.into();
        Ok(match id {
            BlockId::Hash(hash) => {
                let hash = utils::serialize(&hash);
                self.request("xcb_getUncleCountByBlockHash", [hash]).await?
            }
            BlockId::Number(num) => {
                let num = utils::serialize(&num);
                self.request("xcb_getUncleCountByBlockNumber", [num]).await?
            }
        })
    }

    async fn get_uncle<T: Into<BlockId> + Send + Sync>(
        &self,
        block_hash_or_number: T,
        idx: U64,
    ) -> Result<Option<Block<H256>>, ProviderError> {
        let blk_id = block_hash_or_number.into();
        let idx = utils::serialize(&idx);
        Ok(match blk_id {
            BlockId::Hash(hash) => {
                let hash = utils::serialize(&hash);
                self.request("xcb_getUncleByBlockHashAndIndex", [hash, idx]).await?
            }
            BlockId::Number(num) => {
                let num = utils::serialize(&num);
                self.request("xcb_getUncleByBlockNumberAndIndex", [num, idx]).await?
            }
        })
    }

    async fn get_transaction<T: Send + Sync + Into<TxHash>>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<Transaction>, ProviderError> {
        let hash = transaction_hash.into();
        self.request("xcb_getTransactionByHash", [hash]).await
    }

    async fn get_transaction_receipt<T: Send + Sync + Into<TxHash>>(
        &self,
        transaction_hash: T,
    ) -> Result<Option<TransactionReceipt>, ProviderError> {
        let hash = transaction_hash.into();
        self.request("xcb_getTransactionReceipt", [hash]).await
    }

    async fn get_block_receipts<T: Into<BlockNumber> + Send + Sync>(
        &self,
        block: T,
    ) -> Result<Vec<TransactionReceipt>, Self::Error> {
        self.request("xcb_getBlockReceipts", [block.into()]).await
    }

    async fn parity_block_receipts<T: Into<BlockNumber> + Send + Sync>(
        &self,
        block: T,
    ) -> Result<Vec<TransactionReceipt>, Self::Error> {
        self.request("parity_getBlockReceipts", vec![block.into()]).await
    }

    async fn get_energy_price(&self) -> Result<U256, ProviderError> {
        self.request("xcb_energyPrice", ()).await
    }

    async fn get_accounts(&self) -> Result<Vec<Address>, ProviderError> {
        self.request("xcb_accounts", ()).await
    }

    async fn get_transaction_count<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        from: T,
        block: Option<BlockId>,
    ) -> Result<U256, ProviderError> {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self.resolve_name(&ens_name).await?,
            NameOrAddress::Address(addr) => addr,
        };

        let from = utils::serialize(&from);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));
        self.request("xcb_getTransactionCount", [from, block]).await
    }

    async fn get_balance<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        from: T,
        block: Option<BlockId>,
    ) -> Result<U256, ProviderError> {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self.resolve_name(&ens_name).await?,
            NameOrAddress::Address(addr) => addr,
        };

        let from = utils::serialize(&from);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));
        self.request("xcb_getBalance", [from, block]).await
    }

    async fn get_networkid(&self) -> Result<U256, ProviderError> {
        self.request("xcb_networkId", ()).await
    }

    async fn syncing(&self) -> Result<SyncingStatus, Self::Error> {
        self.request("xcb_syncing", ()).await
    }

    async fn get_net_version(&self) -> Result<String, ProviderError> {
        self.request("net_version", ()).await
    }

    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<Bytes, ProviderError> {
        let tx = utils::serialize(tx);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));
        self.request("xcb_call", [tx, block]).await
    }

    async fn estimate_energy(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<U256, ProviderError> {
        let tx = utils::serialize(tx);
        // Some nodes (e.g. old Optimism clients) don't support a block ID being passed as a param,
        // so refrain from defaulting to BlockNumber::Latest.
        let params = if let Some(block_id) = block {
            vec![tx, utils::serialize(&block_id)]
        } else {
            vec![tx]
        };
        self.request("xcb_estimateEnergy", params).await
    }

    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, P>, ProviderError> {
        let mut tx = tx.into();
        self.fill_transaction(&mut tx, block).await?;
        let tx_hash = self.request("xcb_sendTransaction", [tx]).await?;

        Ok(PendingTransaction::new(tx_hash, self))
    }

    async fn send_raw_transaction<'a>(
        &'a self,
        tx: Bytes,
    ) -> Result<PendingTransaction<'a, P>, ProviderError> {
        let rlp = utils::serialize(&tx);
        let tx_hash = self.request("xcb_sendRawTransaction", [rlp]).await?;
        Ok(PendingTransaction::new(tx_hash, self))
    }

    async fn is_signer(&self) -> bool {
        match self.from {
            Some(sender) => self.sign(vec![], &sender).await.is_ok(),
            None => false,
        }
    }

    async fn sign<T: Into<Bytes> + Send + Sync>(
        &self,
        data: T,
        from: &Address,
    ) -> Result<Signature, ProviderError> {
        let data = utils::serialize(&data.into());
        let from = utils::serialize(from);

        // get the response from `xcb_sign` call and trim the 0x-prefix if present.
        let sig: String = self.request("xcb_sign", [from, data]).await?;
        let sig = sig.strip_prefix("0x").unwrap_or(&sig);

        // decode the signature.
        let sig = hex::decode(sig)?;
        Ok(Signature::try_from(sig.as_slice())
            .map_err(|e| ProviderError::CustomError(e.to_string()))?)
    }

    /// Sign a transaction via RPC call
    async fn sign_transaction(
        &self,
        _tx: &TypedTransaction,
        _from: Address,
    ) -> Result<Signature, Self::Error> {
        Err(MiddlewareError::from_err(ProviderError::SignerUnavailable))
    }

    ////// Contract state

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>, ProviderError> {
        self.request("xcb_getLogs", [filter]).await
    }

    fn get_logs_paginated<'a>(&'a self, filter: &Filter, page_size: u64) -> LogQuery<'a, P> {
        LogQuery::new(self, filter).with_page_size(page_size)
    }

    async fn watch<'a>(
        &'a self,
        filter: &Filter,
    ) -> Result<FilterWatcher<'a, P, Log>, ProviderError> {
        let id = self.new_filter(FilterKind::Logs(filter)).await?;
        let filter = FilterWatcher::new(id, self).interval(self.get_interval());
        Ok(filter)
    }

    async fn watch_blocks(&self) -> Result<FilterWatcher<'_, P, H256>, ProviderError> {
        let id = self.new_filter(FilterKind::NewBlocks).await?;
        let filter = FilterWatcher::new(id, self).interval(self.get_interval());
        Ok(filter)
    }

    /// Streams pending transactions
    async fn watch_pending_transactions(
        &self,
    ) -> Result<FilterWatcher<'_, P, H256>, ProviderError> {
        let id = self.new_filter(FilterKind::PendingTransactions).await?;
        let filter = FilterWatcher::new(id, self).interval(self.get_interval());
        Ok(filter)
    }

    async fn new_filter(&self, filter: FilterKind<'_>) -> Result<U256, ProviderError> {
        let (method, args) = match filter {
            FilterKind::NewBlocks => ("xcb_newBlockFilter", vec![]),
            FilterKind::PendingTransactions => ("xcb_newPendingTransactionFilter", vec![]),
            FilterKind::Logs(filter) => ("xcb_newFilter", vec![utils::serialize(&filter)]),
        };

        self.request(method, args).await
    }

    async fn uninstall_filter<T: Into<U256> + Send + Sync>(
        &self,
        id: T,
    ) -> Result<bool, ProviderError> {
        let id = utils::serialize(&id.into());
        self.request("xcb_uninstallFilter", [id]).await
    }

    async fn get_filter_changes<T, R>(&self, id: T) -> Result<Vec<R>, ProviderError>
    where
        T: Into<U256> + Send + Sync,
        R: Serialize + DeserializeOwned + Send + Sync + Debug,
    {
        let id = utils::serialize(&id.into());
        self.request("xcb_getFilterChanges", [id]).await
    }

    async fn get_storage_at<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        from: T,
        location: H256,
        block: Option<BlockId>,
    ) -> Result<H256, ProviderError> {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self.resolve_name(&ens_name).await?,
            NameOrAddress::Address(addr) => addr,
        };

        // position is a QUANTITY according to the [spec](https://eth.wiki/json-rpc/API#xcb_getstorageat): integer of the position in the storage, converting this to a U256
        // will make sure the number is formatted correctly as [quantity](https://eips.ethereum.org/EIPS/eip-1474#quantity)
        let position = U256::from_big_endian(location.as_bytes());
        let position = utils::serialize(&position);
        let from = utils::serialize(&from);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));

        // get the hex encoded value.
        let value: String = self.request("xcb_getStorageAt", [from, position, block]).await?;
        // get rid of the 0x prefix and left pad it with zeroes.
        let value = format!("{:0>64}", value.replace("0x", ""));
        Ok(H256::from_slice(&Vec::from_hex(value)?))
    }

    async fn get_code<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        at: T,
        block: Option<BlockId>,
    ) -> Result<Bytes, ProviderError> {
        let at = match at.into() {
            NameOrAddress::Name(ens_name) => self.resolve_name(&ens_name).await?,
            NameOrAddress::Address(addr) => addr,
        };

        let at = utils::serialize(&at);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));
        self.request("xcb_getCode", [at, block]).await
    }

    async fn get_proof<T: Into<NameOrAddress> + Send + Sync>(
        &self,
        from: T,
        locations: Vec<H256>,
        block: Option<BlockId>,
    ) -> Result<EIP1186ProofResponse, ProviderError> {
        let from = match from.into() {
            NameOrAddress::Name(ens_name) => self.resolve_name(&ens_name).await?,
            NameOrAddress::Address(addr) => addr,
        };

        let from = utils::serialize(&from);
        let locations = locations.iter().map(|location| utils::serialize(&location)).collect();
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));

        self.request("xcb_getProof", [from, locations, block]).await
    }

    /// Returns an indication if this node is currently mining.
    async fn mining(&self) -> Result<bool, Self::Error> {
        self.request("xcb_mining", ()).await
    }

    async fn import_raw_key(
        &self,
        private_key: Bytes,
        passphrase: String,
    ) -> Result<Address, ProviderError> {
        // private key should not be prefixed with 0x - it is also up to the user to pass in a key
        // of the correct length

        // the private key argument is supposed to be a string
        let private_key_hex = hex::encode(private_key);
        let private_key = utils::serialize(&private_key_hex);
        let passphrase = utils::serialize(&passphrase);
        self.request("personal_importRawKey", [private_key, passphrase]).await
    }

    async fn unlock_account<T: Into<Address> + Send + Sync>(
        &self,
        account: T,
        passphrase: String,
        duration: Option<u64>,
    ) -> Result<bool, ProviderError> {
        let account = utils::serialize(&account.into());
        let duration = utils::serialize(&duration.unwrap_or(0));
        let passphrase = utils::serialize(&passphrase);
        self.request("personal_unlockAccount", [account, passphrase, duration]).await
    }

    async fn add_peer(&self, enode_url: String) -> Result<bool, Self::Error> {
        let enode_url = utils::serialize(&enode_url);
        self.request("admin_addPeer", [enode_url]).await
    }

    async fn add_trusted_peer(&self, enode_url: String) -> Result<bool, Self::Error> {
        let enode_url = utils::serialize(&enode_url);
        self.request("admin_addTrustedPeer", [enode_url]).await
    }

    async fn node_info(&self) -> Result<NodeInfo, Self::Error> {
        self.request("admin_nodeInfo", ()).await
    }

    async fn peers(&self) -> Result<Vec<PeerInfo>, Self::Error> {
        self.request("admin_peers", ()).await
    }

    async fn remove_peer(&self, enode_url: String) -> Result<bool, Self::Error> {
        let enode_url = utils::serialize(&enode_url);
        self.request("admin_removePeer", [enode_url]).await
    }

    async fn remove_trusted_peer(&self, enode_url: String) -> Result<bool, Self::Error> {
        let enode_url = utils::serialize(&enode_url);
        self.request("admin_removeTrustedPeer", [enode_url]).await
    }

    async fn start_mining(&self, threads: Option<usize>) -> Result<(), Self::Error> {
        let threads = utils::serialize(&threads);
        self.request("miner_start", [threads]).await
    }

    async fn stop_mining(&self) -> Result<(), Self::Error> {
        self.request("miner_stop", ()).await
    }

    async fn resolve_name(&self, ens_name: &str) -> Result<Address, ProviderError> {
        self.query_resolver(ParamType::Address, ens_name, ens::ADDR_SELECTOR).await
    }

    async fn lookup_address(&self, address: Address) -> Result<String, ProviderError> {
        let ens_name = ens::reverse_address(address);
        let domain: String =
            self.query_resolver(ParamType::String, &ens_name, ens::NAME_SELECTOR).await?;
        let reverse_address = self.resolve_name(&domain).await?;
        if address != reverse_address {
            Err(ProviderError::EnsNotOwned(domain))
        } else {
            Ok(domain)
        }
    }

    async fn resolve_avatar(&self, ens_name: &str) -> Result<Url, ProviderError> {
        let (field, owner) =
            try_join!(self.resolve_field(ens_name, "avatar"), self.resolve_name(ens_name))?;
        let url = Url::from_str(&field).map_err(|e| ProviderError::CustomError(e.to_string()))?;
        match url.scheme() {
            "https" | "data" => Ok(url),
            "ipfs" => erc::http_link_ipfs(url).map_err(ProviderError::CustomError),
            "eip155" => {
                let token =
                    erc::ERCNFT::from_str(url.path()).map_err(ProviderError::CustomError)?;
                match token.type_ {
                    erc::ERCNFTType::ERC721 => {
                        let tx = TransactionRequest {
                            data: Some(
                                [&erc::ERC721_OWNER_SELECTOR[..], &token.id].concat().into(),
                            ),
                            to: Some(NameOrAddress::Address(token.contract)),
                            ..Default::default()
                        };
                        let data = self.call(&tx.into(), None).await?;
                        if decode_bytes::<Address>(ParamType::Address, data) != owner {
                            return Err(ProviderError::CustomError("Incorrect owner.".to_string()))
                        }
                    }
                    erc::ERCNFTType::ERC1155 => {
                        let tx = TransactionRequest {
                            data: Some(
                                [
                                    &erc::ERC1155_BALANCE_SELECTOR[..],
                                    &[0x0; 12],
                                    &owner.0,
                                    &token.id,
                                ]
                                .concat()
                                .into(),
                            ),
                            to: Some(NameOrAddress::Address(token.contract)),
                            ..Default::default()
                        };
                        let data = self.call(&tx.into(), None).await?;
                        if decode_bytes::<u64>(ParamType::Uint(64), data) == 0 {
                            return Err(ProviderError::CustomError("Incorrect balance.".to_string()))
                        }
                    }
                }

                let image_url = self.resolve_nft(token).await?;
                match image_url.scheme() {
                    "https" | "data" => Ok(image_url),
                    "ipfs" => erc::http_link_ipfs(image_url).map_err(ProviderError::CustomError),
                    _ => Err(ProviderError::CustomError(
                        "Unsupported scheme for the image".to_string(),
                    )),
                }
            }
            _ => Err(ProviderError::CustomError("Unsupported scheme".to_string())),
        }
    }

    async fn resolve_nft(&self, token: erc::ERCNFT) -> Result<Url, ProviderError> {
        let selector = token.type_.resolution_selector();
        let tx = TransactionRequest {
            data: Some([&selector[..], &token.id].concat().into()),
            to: Some(NameOrAddress::Address(token.contract)),
            ..Default::default()
        };
        let data = self.call(&tx.into(), None).await?;
        let mut metadata_url = Url::parse(&decode_bytes::<String>(ParamType::String, data))
            .map_err(|e| ProviderError::CustomError(format!("Invalid metadata url: {e}")))?;

        if token.type_ == erc::ERCNFTType::ERC1155 {
            metadata_url.set_path(&metadata_url.path().replace("%7Bid%7D", &hex::encode(token.id)));
        }
        if metadata_url.scheme() == "ipfs" {
            metadata_url = erc::http_link_ipfs(metadata_url).map_err(ProviderError::CustomError)?;
        }
        let metadata: erc::Metadata = reqwest::get(metadata_url).await?.json().await?;
        Url::parse(&metadata.image).map_err(|e| ProviderError::CustomError(e.to_string()))
    }

    async fn resolve_field(&self, ens_name: &str, field: &str) -> Result<String, ProviderError> {
        let field: String = self
            .query_resolver_parameters(
                ParamType::String,
                ens_name,
                ens::FIELD_SELECTOR,
                Some(&ens::parameterhash(field)),
            )
            .await?;
        Ok(field)
    }

    async fn txpool_content(&self) -> Result<TxpoolContent, ProviderError> {
        self.request("txpool_content", ()).await
    }

    async fn txpool_inspect(&self) -> Result<TxpoolInspect, ProviderError> {
        self.request("txpool_inspect", ()).await
    }

    async fn txpool_status(&self) -> Result<TxpoolStatus, ProviderError> {
        self.request("txpool_status", ()).await
    }

    async fn debug_trace_transaction(
        &self,
        tx_hash: TxHash,
        trace_options: GoCoreDebugTracingOptions,
    ) -> Result<GoCoreTrace, ProviderError> {
        let tx_hash = utils::serialize(&tx_hash);
        let trace_options = utils::serialize(&trace_options);
        self.request("debug_traceTransaction", [tx_hash, trace_options]).await
    }

    async fn debug_trace_call<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        req: T,
        block: Option<BlockId>,
        trace_options: GoCoreDebugTracingCallOptions,
    ) -> Result<GoCoreTrace, ProviderError> {
        let req = req.into();
        let req = utils::serialize(&req);
        let block = utils::serialize(&block.unwrap_or_else(|| BlockNumber::Latest.into()));
        let trace_options = utils::serialize(&trace_options);
        self.request("debug_traceCall", [req, block, trace_options]).await
    }

    async fn trace_call<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        req: T,
        trace_type: Vec<TraceType>,
        block: Option<BlockNumber>,
    ) -> Result<BlockTrace, ProviderError> {
        let req = req.into();
        let req = utils::serialize(&req);
        let block = utils::serialize(&block.unwrap_or(BlockNumber::Latest));
        let trace_type = utils::serialize(&trace_type);
        self.request("trace_call", [req, trace_type, block]).await
    }

    async fn trace_call_many<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        req: Vec<(T, Vec<TraceType>)>,
        block: Option<BlockNumber>,
    ) -> Result<Vec<BlockTrace>, ProviderError> {
        let req: Vec<(TypedTransaction, Vec<TraceType>)> =
            req.into_iter().map(|(tx, trace_type)| (tx.into(), trace_type)).collect();
        let req = utils::serialize(&req);
        let block = utils::serialize(&block.unwrap_or(BlockNumber::Latest));
        self.request("trace_callMany", [req, block]).await
    }

    async fn trace_raw_transaction(
        &self,
        data: Bytes,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, ProviderError> {
        let data = utils::serialize(&data);
        let trace_type = utils::serialize(&trace_type);
        self.request("trace_rawTransaction", [data, trace_type]).await
    }

    async fn trace_replay_transaction(
        &self,
        hash: H256,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, ProviderError> {
        let hash = utils::serialize(&hash);
        let trace_type = utils::serialize(&trace_type);
        self.request("trace_replayTransaction", [hash, trace_type]).await
    }

    async fn trace_replay_block_transactions(
        &self,
        block: BlockNumber,
        trace_type: Vec<TraceType>,
    ) -> Result<Vec<BlockTrace>, ProviderError> {
        let block = utils::serialize(&block);
        let trace_type = utils::serialize(&trace_type);
        self.request("trace_replayBlockTransactions", [block, trace_type]).await
    }

    async fn trace_block(&self, block: BlockNumber) -> Result<Vec<Trace>, ProviderError> {
        let block = utils::serialize(&block);
        self.request("trace_block", [block]).await
    }

    async fn trace_filter(&self, filter: TraceFilter) -> Result<Vec<Trace>, ProviderError> {
        let filter = utils::serialize(&filter);
        self.request("trace_filter", vec![filter]).await
    }

    async fn trace_get<T: Into<U64> + Send + Sync>(
        &self,
        hash: H256,
        index: Vec<T>,
    ) -> Result<Trace, ProviderError> {
        let hash = utils::serialize(&hash);
        let index: Vec<U64> = index.into_iter().map(|i| i.into()).collect();
        let index = utils::serialize(&index);
        self.request("trace_get", vec![hash, index]).await
    }

    async fn trace_transaction(&self, hash: H256) -> Result<Vec<Trace>, ProviderError> {
        let hash = utils::serialize(&hash);
        self.request("trace_transaction", vec![hash]).await
    }

    async fn subscribe<T, R>(
        &self,
        params: T,
    ) -> Result<SubscriptionStream<'_, P, R>, ProviderError>
    where
        T: Debug + Serialize + Send + Sync,
        R: DeserializeOwned + Send + Sync,
        P: PubsubClient,
    {
        let id: U256 = self.request("xcb_subscribe", params).await?;
        SubscriptionStream::new(id, self).map_err(Into::into)
    }

    async fn unsubscribe<T>(&self, id: T) -> Result<bool, ProviderError>
    where
        T: Into<U256> + Send + Sync,
        P: PubsubClient,
    {
        self.request("xcb_unsubscribe", [id.into()]).await
    }

    async fn subscribe_blocks(
        &self,
    ) -> Result<SubscriptionStream<'_, P, Block<TxHash>>, ProviderError>
    where
        P: PubsubClient,
    {
        self.subscribe(["newHeads"]).await
    }

    async fn subscribe_pending_txs(
        &self,
    ) -> Result<SubscriptionStream<'_, P, TxHash>, ProviderError>
    where
        P: PubsubClient,
    {
        self.subscribe(["newPendingTransactions"]).await
    }

    async fn subscribe_logs<'a>(
        &'a self,
        filter: &Filter,
    ) -> Result<SubscriptionStream<'a, P, Log>, ProviderError>
    where
        P: PubsubClient,
    {
        let loaded_logs = match filter.block_option {
            FilterBlockOption::Range { from_block, to_block: _ } => {
                if from_block.is_none() {
                    vec![]
                } else {
                    self.get_logs(filter).await?
                }
            }
            FilterBlockOption::AtBlockHash(_block_hash) => self.get_logs(filter).await?,
        };
        let loaded_logs = VecDeque::from(loaded_logs);

        let logs = utils::serialize(&"logs"); // TODO: Make this a static
        let filter = utils::serialize(filter);
        self.subscribe([logs, filter]).await.map(|mut stream| {
            stream.set_loaded_elements(loaded_logs);
            stream
        })
    }
}

impl<P: JsonRpcClient> Provider<P> {
    async fn query_resolver<T: Detokenize>(
        &self,
        param: ParamType,
        ens_name: &str,
        selector: Selector,
    ) -> Result<T, ProviderError> {
        self.query_resolver_parameters(param, ens_name, selector, None).await
    }

    async fn query_resolver_parameters<T: Detokenize>(
        &self,
        param: ParamType,
        ens_name: &str,
        selector: Selector,
        parameters: Option<&[u8]>,
    ) -> Result<T, ProviderError> {
        // Get the ENS address, prioritize the local override variable
        let ens_addr = self.ens.unwrap_or(ens::CNS_ADDRESS);

        // first get the resolver responsible for this name
        // the call will return a Bytes array which we convert to an address
        let data = self.call(&ens::get_resolver(ens_addr, ens_name).into(), None).await?;

        // otherwise, decode_bytes panics
        if data.0.is_empty() {
            return Err(ProviderError::EnsError(ens_name.to_string()))
        }

        let resolver_address: Address = decode_bytes(ParamType::Address, data);
        if resolver_address == Address::zero() {
            return Err(ProviderError::EnsError(ens_name.to_string()))
        }

        if let ParamType::Address = param {
            // Reverse resolver reverts when calling `supportsInterface(bytes4)`
            self.validate_resolver(resolver_address, selector, ens_name).await?;
        }

        // resolve
        let data = self
            .call(&ens::resolve(resolver_address, selector, ens_name, parameters).into(), None)
            .await?;

        Ok(decode_bytes(param, data))
    }

    /// Validates that the resolver supports `selector`.
    async fn validate_resolver(
        &self,
        resolver_address: Address,
        selector: Selector,
        ens_name: &str,
    ) -> Result<(), ProviderError> {
        let data =
            self.call(&ens::supports_interface(resolver_address, selector).into(), None).await?;

        if data.is_empty() {
            return Err(ProviderError::EnsError(format!(
                "`{ens_name}` resolver ({resolver_address:?}) is invalid."
            )))
        }

        let supports_selector = abi::decode(&[ParamType::Bool], data.as_ref())
            .map(|token| token[0].clone().into_bool().unwrap_or_default())
            .unwrap_or_default();

        if !supports_selector {
            return Err(ProviderError::EnsError(format!(
                "`{}` resolver ({:?}) does not support selector {}.",
                ens_name,
                resolver_address,
                hex::encode(selector)
            )))
        }

        Ok(())
    }

    #[cfg(test)]
    /// Anvil and Ganache-only function for mining empty blocks
    pub async fn mine(&self, num_blocks: usize) -> Result<(), ProviderError> {
        for _ in 0..num_blocks {
            self.inner.request::<_, U256>("evm_mine", None::<()>).await.map_err(Into::into)?;
        }
        Ok(())
    }

    /// Sets the ENS Address (default: mainnet)
    #[must_use]
    pub fn ens<T: Into<Address>>(mut self, ens: T) -> Self {
        self.ens = Some(ens.into());
        self
    }

    /// Sets the default polling interval for event filters and pending transactions
    /// (default: 7 seconds)
    pub fn set_interval<T: Into<Duration>>(&mut self, interval: T) -> &mut Self {
        self.interval = Some(interval.into());
        self
    }

    /// Sets the default polling interval for event filters and pending transactions
    /// (default: 7 seconds)
    #[must_use]
    pub fn interval<T: Into<Duration>>(mut self, interval: T) -> Self {
        self.set_interval(interval);
        self
    }

    /// Gets the polling interval which the provider currently uses for event filters
    /// and pending transactions (default: 7 seconds)
    pub fn get_interval(&self) -> Duration {
        self.interval.unwrap_or(DEFAULT_POLL_INTERVAL)
    }
}

#[cfg(all(feature = "ipc", any(unix, windows)))]
impl Provider<crate::Ipc> {
    #[cfg_attr(unix, doc = "Connects to the Unix socket at the provided path.")]
    #[cfg_attr(windows, doc = "Connects to the named pipe at the provided path.\n")]
    #[cfg_attr(
        windows,
        doc = r"Note: the path must be the fully qualified, like: `\\.\pipe\<name>`."
    )]
    pub async fn connect_ipc(path: impl AsRef<std::path::Path>) -> Result<Self, ProviderError> {
        let ipc = crate::Ipc::connect(path).await?;
        Ok(Self::new(ipc))
    }
}

impl Provider<HttpProvider> {
    /// The Url to which requests are made
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    /// Mutable access to the Url to which requests are made
    pub fn url_mut(&mut self) -> &mut Url {
        self.inner.url_mut()
    }
}

impl<Read, Write> Provider<RwClient<Read, Write>>
where
    Read: JsonRpcClient + 'static,
    <Read as JsonRpcClient>::Error: Sync + Send + 'static,
    Write: JsonRpcClient + 'static,
    <Write as JsonRpcClient>::Error: Sync + Send + 'static,
{
    /// Creates a new [Provider] with a [RwClient]
    pub fn rw(r: Read, w: Write) -> Self {
        Self::new(RwClient::new(r, w))
    }
}

impl<T: JsonRpcClientWrapper> Provider<QuorumProvider<T>> {
    /// Provider that uses a quorum
    pub fn quorum(inner: QuorumProvider<T>) -> Self {
        Self::new(inner)
    }
}

impl Provider<MockProvider> {
    /// Returns a `Provider` instantiated with an internal "mock" transport.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
    /// use corebc_core::types::U64;
    /// use corebc_providers::{Middleware, Provider};
    /// // Instantiate the provider
    /// let (provider, mock) = Provider::mocked();
    /// // Push the mock response
    /// mock.push(U64::from(12))?;
    /// // Make the call
    /// let blk = provider.get_block_number().await.unwrap();
    /// // The response matches
    /// assert_eq!(blk.as_u64(), 12);
    /// // and the request as well!
    /// mock.assert_request("xcb_blockNumber", ()).unwrap();
    /// # Ok(())
    /// # }
    /// ```
    pub fn mocked() -> (Self, MockProvider) {
        let mock = MockProvider::new();
        let mock_clone = mock.clone();
        (Self::new(mock), mock_clone)
    }
}

/// infallible conversion of Bytes to Address/String
///
/// # Panics
///
/// If the provided bytes were not an interpretation of an address
fn decode_bytes<T: Detokenize>(param: ParamType, bytes: Bytes) -> T {
    let tokens = abi::decode(&[param], bytes.as_ref())
        .expect("could not abi-decode bytes to address tokens");
    T::from_tokens(tokens).expect("could not parse tokens as address")
}

impl TryFrom<&str> for Provider<HttpProvider> {
    type Error = ParseError;

    fn try_from(src: &str) -> Result<Self, Self::Error> {
        Ok(Provider::new(HttpProvider::new(Url::parse(src)?)))
    }
}

impl TryFrom<String> for Provider<HttpProvider> {
    type Error = ParseError;

    fn try_from(src: String) -> Result<Self, Self::Error> {
        Provider::try_from(src.as_str())
    }
}

impl<'a> TryFrom<&'a String> for Provider<HttpProvider> {
    type Error = ParseError;

    fn try_from(src: &'a String) -> Result<Self, Self::Error> {
        Provider::try_from(src.as_str())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Provider<RetryClient<HttpProvider>> {
    /// Create a new [`RetryClient`] by connecting to the provided URL. Errors
    /// if `src` is not a valid URL
    pub fn new_client(src: &str, max_retry: u32, initial_backoff: u64) -> Result<Self, ParseError> {
        Ok(Provider::new(RetryClient::new(
            HttpProvider::new(Url::parse(src)?),
            Box::new(HttpRateLimitRetryPolicy),
            max_retry,
            initial_backoff,
        )))
    }
}

mod sealed {
    use crate::{Http, Provider};
    /// private trait to ensure extension trait is not implement outside of this crate
    pub trait Sealed {}
    impl Sealed for Provider<Http> {}
}

/// Extension trait for `Provider`
///
/// **Note**: this is currently sealed until <https://github.com/gakonst/ethers-rs/pull/1267> is finalized
///
/// # Example
///
/// Automatically configure poll interval via `xcb_getNetworkId`
///
/// Note that this will send an RPC to retrieve the network id.
///
/// ```no_run
///  # use corebc_providers::{Http, Provider, ProviderExt};
///  # async fn t() {
/// let http_provider = Provider::<Http>::connect("https://eth.llamarpc.com").await;
/// # }
/// ```
///
/// This is essentially short for
///
/// ```no_run
/// use std::convert::TryFrom;
/// use corebc_core::types::Network;
/// use corebc_providers::{Http, Provider, ProviderExt};
/// let http_provider = Provider::<Http>::try_from("https://eth.llamarpc.com").unwrap().set_network(Network::Mainnet);
/// ```
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ProviderExt: sealed::Sealed {
    /// The error type that can occur when creating a provider
    type Error: Debug;

    /// Creates a new instance connected to the given `url`, exit on error
    async fn connect(url: &str) -> Self
    where
        Self: Sized,
    {
        Self::try_connect(url).await.unwrap()
    }

    /// Try to create a new `Provider`
    async fn try_connect(url: &str) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Customize `Provider` settings for network.
    ///
    /// E.g. [`Network::average_blocktime_hint()`] returns the average block time which can be used
    /// to tune the polling interval.
    ///
    /// Returns the customized `Provider`
    fn for_network(mut self, network: impl Into<Network>) -> Self
    where
        Self: Sized,
    {
        self.set_network(network);
        self
    }

    /// Customized `Provider` settings for network
    fn set_network(&mut self, network: impl Into<Network>) -> &mut Self;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ProviderExt for Provider<HttpProvider> {
    type Error = ParseError;

    async fn try_connect(url: &str) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let mut provider = Provider::try_from(url)?;
        if is_local_endpoint(url) {
            provider.set_interval(DEFAULT_LOCAL_POLL_INTERVAL);
        } else if let Some(network) =
            provider.get_networkid().await.ok().and_then(|id| Network::try_from(id).ok())
        {
            provider.set_network(network);
        }

        Ok(provider)
    }

    fn set_network(&mut self, network: impl Into<Network>) -> &mut Self {
        let network = network.into();
        if let Some(blocktime) = network.average_blocktime_hint() {
            // use half of the block time
            self.set_interval(blocktime / 2);
        }
        self
    }
}

/// Returns true if the endpoint is local
///
/// # Example
///
/// ```
/// use corebc_providers::is_local_endpoint;
/// assert!(is_local_endpoint("http://localhost:8545"));
/// assert!(is_local_endpoint("http://127.0.0.1:8545"));
/// ```
#[inline]
pub fn is_local_endpoint(url: &str) -> bool {
    url.contains("127.0.0.1") || url.contains("localhost")
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::Http;
    use corebc_core::{
        types::{TransactionRequest, H256},
        utils::{Genesis, GoCore, GoCoreInstance},
    };
    use std::path::PathBuf;

    // #[tokio::test]
    // async fn test_provider() {
    //     let provider = Provider::<Http>::try_from("https://xcbapi.coreblockchain.net/")
    //     // let provider = Provider::<Http>::try_from("https://xcbapi.corecoin.cc/")
    //         .unwrap()
    //         .interval(Duration::from_millis(1000));

    //     let mut block_number = provider.get_block_number().await.unwrap();
    //     block_number = U64::from(6038332);
    //     // block_number = U64::from(179040);
    //     let current_block = provider.get_block(block_number).await.unwrap().unwrap();
    //     println!("{:?}", current_block.transactions[0]);
    //     let tx = provider.get_transaction(current_block.transactions[0]).await.unwrap().unwrap();
    //     println!("{:?}", tx);
    //     println!("{:?}", tx.from);
    //     println!("{:?}", tx.recover_from().unwrap());
    // }

    #[test]
    fn convert_h256_u256_quantity() {
        let hash: H256 = H256::zero();
        let quantity = U256::from_big_endian(hash.as_bytes());
        assert_eq!(format!("{quantity:#x}"), "0x0");
        assert_eq!(utils::serialize(&quantity).to_string(), "\"0x0\"");

        let address: Address = "0x0000295a70b2de5e3953354a6a8344e616ed314d7251".parse().unwrap();
        let block = BlockNumber::Latest;
        let params =
            [utils::serialize(&address), utils::serialize(&quantity), utils::serialize(&block)];

        let params = serde_json::to_string(&params).unwrap();
        assert_eq!(params, r#"["0x0000295a70b2de5e3953354a6a8344e616ed314d7251","0x0","latest"]"#);
    }

    // CORETODO: This test is impossible without modifying anvil in the first place
    // #[tokio::test]
    // async fn test_new_block_filter() {
    //     let num_blocks = 3;
    //     let gocore = Anvil::new().block_time(2u64).spawn();
    //     let provider = Provider::<Http>::try_from(gocore.endpoint())
    //         .unwrap()
    //         .interval(Duration::from_millis(1000));

    //     let start_block = provider.get_block_number().await.unwrap();

    //     let stream = provider.watch_blocks().await.unwrap().stream();

    //     let hashes: Vec<H256> = stream.take(num_blocks).collect::<Vec<H256>>().await;
    //     for (i, hash) in hashes.iter().enumerate() {
    //         let block = provider.get_block(start_block + i as u64 + 1).await.unwrap().unwrap();
    //         assert_eq!(*hash, block.hash.unwrap());
    //     }
    // }

    // CORETODO: This test is impossible without modifying anvil in the first place
    // #[tokio::test]
    // async fn test_is_signer() {
    //     use corebc_core::utils::Anvil;
    //     use std::str::FromStr;

    //     let anvil = Anvil::new().spawn();
    //     let provider =
    //         Provider::<Http>::try_from(anvil.endpoint()).unwrap().with_sender(anvil.
    // addresses()[0]);     assert!(provider.is_signer().await);

    //     let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();
    //     assert!(!provider.is_signer().await);

    //     let sender = Address::from_str("0000635B4764D1939DfAcD3a8014726159abC277BecC")
    //         .expect("should be able to parse hex address");
    //     let provider = Provider::<Http>::try_from(
    //         "https://ropsten.infura.io/v3/fd8b88b56aa84f6da87b60f5441d6778",
    //     )
    //     .unwrap()
    //     .with_sender(sender);
    //     assert!(!provider.is_signer().await);
    // }

    // CORETODO: This test is impossible without modifying anvil in the first place
    // #[tokio::test]
    // async fn test_new_pending_txs_filter() {
    //     let num_txs = 5;

    //     let gocore = Anvil::new().block_time(2u64).spawn();
    //     let provider = Provider::<Http>::try_from(gocore.endpoint())
    //         .unwrap()
    //         .interval(Duration::from_millis(1000));
    //     let accounts = provider.get_accounts().await.unwrap();

    //     let stream = provider.watch_pending_transactions().await.unwrap().stream();

    //     let mut tx_hashes = Vec::new();
    //     let tx = TransactionRequest::new().from(accounts[0]).to(accounts[0]).value(1e18 as u64);

    //     for _ in 0..num_txs {
    //         tx_hashes.push(provider.send_transaction(tx.clone(), None).await.unwrap());
    //     }

    //     let hashes: Vec<H256> = stream.take(num_txs).collect::<Vec<H256>>().await;
    //     assert_eq!(tx_hashes, hashes);
    // }

    // CORETODO: This test is impossible without modifying anvil in the first place
    // #[tokio::test]
    // async fn receipt_on_unmined_tx() {
    //     use corebc_core::{
    //         types::TransactionRequest,
    //         utils::{parse_ether, Anvil},
    //     };
    //     let anvil = Anvil::new().block_time(2u64).spawn();
    //     let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();

    //     let accounts = provider.get_accounts().await.unwrap();
    //     let tx = TransactionRequest::pay(accounts[0],
    // parse_ether(1u64).unwrap()).from(accounts[0]);     let pending_tx =
    // provider.send_transaction(tx, None).await.unwrap();

    //     assert!(provider.get_transaction_receipt(*pending_tx).await.unwrap().is_none());

    //     let hash = *pending_tx;
    //     let receipt = pending_tx.await.unwrap().unwrap();
    //     assert_eq!(receipt.transaction_hash, hash);
    // }

    #[tokio::test]
    async fn parity_block_receipts() {
        let url = match std::env::var("PARITY") {
            Ok(inner) => inner,
            _ => return,
        };
        let provider = Provider::<Http>::try_from(url.as_str()).unwrap();
        let receipts = provider.parity_block_receipts(10657200).await.unwrap();
        assert!(!receipts.is_empty());
    }

    #[tokio::test]
    #[ignore]
    #[cfg(feature = "ws")]
    async fn test_trace_call_many() {
        use corebc_core::types::H176;

        // TODO: Implement ErigonInstance, so it'd be possible to test this.
        let provider = Provider::new(crate::Ws::connect("ws://127.0.0.1:8545").await.unwrap());
        let traces = provider
            .trace_call_many(
                vec![
                    (
                        TransactionRequest::new()
                            .from(Address::zero())
                            .to("0x00000000000000000000000000000000000000000001"
                                .parse::<H176>()
                                .unwrap())
                            .value(U256::from(10000000000000000u128)),
                        vec![TraceType::StateDiff],
                    ),
                    (
                        TransactionRequest::new()
                            .from(
                                "0x00000000000000000000000000000000000000000001"
                                    .parse::<H176>()
                                    .unwrap(),
                            )
                            .to("0x00000000000000000000000000000000000000000002"
                                .parse::<H176>()
                                .unwrap())
                            .value(U256::from(10000000000000000u128)),
                        vec![TraceType::StateDiff],
                    ),
                ],
                None,
            )
            .await
            .unwrap();
        dbg!(traces);
    }

    #[tokio::test]
    async fn test_fill_transaction_legacy() {
        let (mut provider, mock) = Provider::mocked();
        provider.from = Some("0x00006fC21092DA55B392b045eD78F4732bff3C580e2c".parse().unwrap());

        let energy = U256::from(21000_usize);
        let energy_price = U256::from(50_usize);

        // --- leaves a filled legacy transaction unchanged, making no requests
        let from: Address = "0x00000000000000000000000000000000000000000001".parse().unwrap();
        let to: Address = "0x00000000000000000000000000000000000000000002".parse().unwrap();
        let mut tx = TransactionRequest::new()
            .from(from)
            .to(to)
            .energy(energy)
            .energy_price(energy_price)
            .into();
        provider.fill_transaction(&mut tx, None).await.unwrap();

        assert_eq!(tx.from(), Some(&from));
        assert_eq!(tx.to(), Some(&to.into()));
        assert_eq!(tx.energy(), Some(&energy));
        assert_eq!(tx.energy_price(), Some(energy_price));

        // --- fills an empty legacy transaction
        let mut tx = TransactionRequest::new().into();
        mock.push(energy).unwrap();
        mock.push(energy_price).unwrap();
        provider.fill_transaction(&mut tx, None).await.unwrap();

        assert_eq!(tx.from(), provider.from.as_ref());
        assert!(tx.to().is_none());
        assert_eq!(tx.energy(), Some(&energy));
        assert_eq!(tx.energy_price(), Some(energy_price));
    }

    // CORETODO: Uncomment once signatures are done
    // #[tokio::test]
    // async fn gocore_admin_nodeinfo() {
    //     // we can't use the test provider because infura does not expose admin endpoints
    //     let network = 1337u64;
    //     let dir = tempfile::tempdir().unwrap();

    //     let (gocore, provider) =
    //         spawn_gocore_and_create_provider(network, Some(dir.path().into()), None);

    //     let info = provider.node_info().await.unwrap();
    //     drop(gocore);

    //     // make sure it is running eth
    //     assert!(info.protocols.eth.is_some());

    //     // check that the network id is correct
    //     assert_eq!(info.protocols.eth.unwrap().network, network);

    //     #[cfg(not(windows))]
    //     dir.close().unwrap();
    // }

    /// Spawn a new `GoCoreInstance` without discovery and create a `Provider` for it.
    ///
    /// These will all use the same genesis config.
    fn spawn_gocore_and_create_provider(
        network_id: u64,
        datadir: Option<PathBuf>,
        genesis: Option<Genesis>,
    ) -> (GoCoreInstance, Provider<HttpProvider>) {
        let gocore = GoCore::new().network_id(network_id).disable_discovery();

        let gocore = match genesis {
            Some(genesis) => gocore.genesis(genesis),
            None => gocore,
        };

        let gocore = match datadir {
            Some(dir) => gocore.data_dir(dir),
            None => gocore,
        }
        .spawn();

        let provider = Provider::try_from(gocore.endpoint()).unwrap();
        (gocore, provider)
    }
}
