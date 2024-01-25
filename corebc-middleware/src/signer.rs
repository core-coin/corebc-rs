use corebc_core::types::{
    transaction::eip2718::TypedTransaction, Address, BlockId, Bytes, Signature, U256,
};
use corebc_providers::{maybe, Middleware, MiddlewareError, PendingTransaction};
use corebc_signers::Signer;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Debug)]
/// Middleware used for locally signing transactions, compatible with any implementer
/// of the [`Signer`] trait.
///
/// # Example
///
/// ```no_run
/// use corebc_providers::{Middleware, Provider, Http};
/// use corebc_signers::LocalWallet;
/// use corebc_middleware::SignerMiddleware;
/// use corebc_core::types::{Address, TransactionRequest};
/// use std::convert::TryFrom;
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = Provider::<Http>::try_from("http://localhost:8545")
///     .expect("could not instantiate HTTP Provider");
///
/// // Transactions will be signed with the private key below and will be broadcast
/// // via the eth_sendRawTransaction API)
/// let wallet: LocalWallet = "ffffffffff8c88cb64309d5bb5f0b5ac13156a48bd297ef08a060f4b43468947b8922c0a89b8c5c16c95215206d2abef34a73ed96b1aaffff7"
///     .parse()?;
///
/// let mut client = SignerMiddleware::new(provider, wallet);
///
/// // You can sign messages with the key
/// let signed_msg = client.sign(b"hello".to_vec(), &client.address()).await?;
///
/// // ...and sign transactions
/// let tx = TransactionRequest::pay("vitalik.eth", 100);
/// let pending_tx = client.send_transaction(tx, None).await?;
///
/// // You can `await` on the pending transaction to get the receipt with a pre-specified
/// // number of confirmations
/// let receipt = pending_tx.confirmations(6).await?;
///
/// // You can connect with other wallets at runtime via the `with_signer` function
/// let wallet2: LocalWallet = "cd8c407233c0560f6de24bb2dc60a8b02335c959a1a17f749ce6c1ccf63d74a7"
///     .parse()?;
///
/// let signed_msg2 = client.with_signer(wallet2).sign(b"hello".to_vec(), &client.address()).await?;
///
/// // This call will be made with `wallet2` since `with_signer` takes a mutable reference.
/// let tx2 = TransactionRequest::new()
///     .to("0xd8da6bf26964af9d7eed9e03e53415d37aa96045".parse::<Address>()?)
///     .value(200);
/// let tx_hash2 = client.send_transaction(tx2, None).await?;
///
/// # Ok(())
/// # }
/// ```
///
/// [`Signer`]: corebc_signers::Signer
pub struct SignerMiddleware<M, S> {
    pub(crate) inner: M,
    pub(crate) signer: S,
    pub(crate) address: Address,
}

#[derive(Error, Debug)]
/// Error thrown when the client interacts with the blockchain
pub enum SignerMiddlewareError<M: Middleware, S: Signer> {
    #[error("{0}")]
    /// Thrown when the internal call to the signer fails
    SignerError(S::Error),

    #[error("{0}")]
    /// Thrown when an internal middleware errors
    MiddlewareError(M::Error),

    /// Thrown if the `nonce` field is missing
    #[error("no nonce was specified")]
    NonceMissing,
    /// Thrown if the `energy_price` field is missing
    #[error("no gas price was specified")]
    GasPriceMissing,
    /// Thrown if the `gas` field is missing
    #[error("no gas was specified")]
    GasMissing,
    /// Thrown if a signature is requested from a different address
    #[error("specified from address is not signer")]
    WrongSigner,
    /// Thrown if the signer's network_id is different than the network_id of the transaction
    #[error("specified network_id is different than the signer's network_id")]
    DifferentNetworkID,
}

impl<M: Middleware, S: Signer> MiddlewareError for SignerMiddlewareError<M, S> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> Self {
        SignerMiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            SignerMiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

// Helper functions for locally signing transactions
impl<M, S> SignerMiddleware<M, S>
where
    M: Middleware,
    S: Signer,
{
    /// Creates a new client from the provider and signer.
    /// Sets the address of this middleware to the address of the signer.
    /// The network_id of the signer will not be set to the network id of the provider. If the
    /// signer passed here is initialized with a different network id, then the client may throw
    /// errors, or methods like `sign_transaction` may error.
    /// To automatically set the signer's network id, see `new_with_provider_network`.
    ///
    /// [`Middleware`] corebc_providers::Middleware
    /// [`Signer`] corebc_signers::Signer
    pub fn new(inner: M, signer: S) -> Self {
        let address = signer.address();
        SignerMiddleware { inner, signer, address }
    }

    /// Signs and returns the RLP encoding of the signed transaction.
    /// If the transaction does not have a network id set, it sets it to the signer's network id.
    /// Returns an error if the transaction's existing network id does not match the signer's
    /// network id.
    async fn sign_transaction(
        &self,
        mut tx: TypedTransaction,
    ) -> Result<Bytes, SignerMiddlewareError<M, S>> {
        // compare network_id and use signer's network_id if the tranasaction's network_id is None,
        // return an error if they are not consistent
        let network_id = self.signer.network_id();
        match tx.network_id() {
            Some(id) if id.as_u64() != network_id => {
                return Err(SignerMiddlewareError::DifferentNetworkID)
            }
            None => {
                tx.set_network_id(network_id);
            }
            _ => {}
        }

        let signature =
            self.signer.sign_transaction(&tx).await.map_err(SignerMiddlewareError::SignerError)?;

        // Return the raw rlp-encoded signed transaction
        Ok(tx.rlp_signed(&signature))
    }

    /// Returns the client's address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Returns a reference to the client's signer
    pub fn signer(&self) -> &S {
        &self.signer
    }

    /// Builds a SignerMiddleware with the given Signer.
    #[must_use]
    pub fn with_signer(&self, signer: S) -> Self
    where
        S: Clone,
        M: Clone,
    {
        let mut this = self.clone();
        this.address = signer.address();
        this.signer = signer;
        this
    }

    /// Creates a new client from the provider and signer.
    /// Sets the address of this middleware to the address of the signer.
    /// Sets the network id of the signer to the network id of the inner [`Middleware`] passed in,
    /// using the [`Signer`]'s implementation of with_network_id.
    ///
    /// [`Middleware`] corebc_providers::Middleware
    /// [`Signer`] corebc_signers::Signer
    pub async fn new_with_provider_network(
        inner: M,
        signer: S,
    ) -> Result<Self, SignerMiddlewareError<M, S>> {
        let address = signer.address();
        let network_id =
            inner.get_networkid().await.map_err(|e| SignerMiddlewareError::MiddlewareError(e))?;
        let signer = signer.with_network_id(network_id.as_u64());
        Ok(SignerMiddleware { inner, signer, address })
    }

    fn set_tx_from_if_none(&self, tx: &TypedTransaction) -> TypedTransaction {
        let mut tx = tx.clone();
        if tx.from().is_none() {
            tx.set_from(self.address);
        }
        tx
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<M, S> Middleware for SignerMiddleware<M, S>
where
    M: Middleware,
    S: Signer,
{
    type Error = SignerMiddlewareError<M, S>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }

    /// Returns the client's address
    fn default_sender(&self) -> Option<Address> {
        Some(self.address)
    }

    /// `SignerMiddleware` is instantiated with a signer.
    async fn is_signer(&self) -> bool {
        true
    }

    async fn sign_transaction(
        &self,
        tx: &TypedTransaction,
        _: Address,
    ) -> Result<Signature, Self::Error> {
        Ok(self.signer.sign_transaction(tx).await.map_err(SignerMiddlewareError::SignerError)?)
    }

    /// Helper for filling a transaction's nonce using the wallet
    async fn fill_transaction(
        &self,
        tx: &mut TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<(), Self::Error> {
        // get the `from` field's nonce if it's set, else get the signer's nonce
        let from = if tx.from().is_some() && tx.from() != Some(&self.address()) {
            *tx.from().unwrap()
        } else {
            self.address
        };
        tx.set_from(from);

        // get the signer's network_id if the transaction does not set it
        let network_id = self.signer.network_id();
        if tx.network_id().is_none() {
            tx.set_network_id(network_id);
        }

        let nonce = maybe(tx.nonce().cloned(), self.get_transaction_count(from, block)).await?;
        tx.set_nonce(nonce);
        self.inner()
            .fill_transaction(tx, block)
            .await
            .map_err(SignerMiddlewareError::MiddlewareError)?;
        Ok(())
    }

    /// Signs and broadcasts the transaction. The optional parameter `block` can be passed so that
    /// gas cost and nonce calculations take it into account. For simple transactions this can be
    /// left to `None`.
    async fn send_transaction<T: Into<TypedTransaction> + Send + Sync>(
        &self,
        tx: T,
        block: Option<BlockId>,
    ) -> Result<PendingTransaction<'_, Self::Provider>, Self::Error> {
        let mut tx = tx.into();

        // fill any missing fields
        self.fill_transaction(&mut tx, block).await?;

        // If the from address is set and is not our signer, delegate to inner
        if tx.from().is_some() && tx.from() != Some(&self.address()) {
            return self
                .inner
                .send_transaction(tx, block)
                .await
                .map_err(SignerMiddlewareError::MiddlewareError);
        }

        // if we have a nonce manager set, we should try handling the result in
        // case there was a nonce mismatch
        let signed_tx = self.sign_transaction(tx).await?;

        // Submit the raw transaction
        self.inner
            .send_raw_transaction(signed_tx)
            .await
            .map_err(SignerMiddlewareError::MiddlewareError)
    }

    /// Signs a message with the internal signer, or if none is present it will make a call to
    /// the connected node's `eth_call` API.
    async fn sign<T: Into<Bytes> + Send + Sync>(
        &self,
        data: T,
        _: &Address,
    ) -> Result<Signature, Self::Error> {
        self.signer.sign_message(data.into()).await.map_err(SignerMiddlewareError::SignerError)
    }

    async fn estimate_energy(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<U256, Self::Error> {
        let tx = self.set_tx_from_if_none(tx);
        self.inner.estimate_energy(&tx, block).await.map_err(SignerMiddlewareError::MiddlewareError)
    }

    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<Bytes, Self::Error> {
        let tx = self.set_tx_from_if_none(tx);
        self.inner().call(&tx, block).await.map_err(SignerMiddlewareError::MiddlewareError)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use corebc_core::{
        abi::Address,
        types::{Bytes, Network, TransactionRequest, U256},
        utils::{self, sha3, Shuttle},
    };
    use corebc_providers::{Middleware, Provider};
    use corebc_signers::{LocalWallet, Signer};
    use std::convert::TryFrom;

    use crate::SignerMiddleware;

    #[tokio::test]
    async fn test_signer() -> Result<(), Box<dyn std::error::Error>> {
        // let provider = Provider::try_from("http://127.0.0.1:8545/").unwrap();
        let provider = Provider::try_from("https://xcbapi.corecoin.cc/").unwrap();
        let network_id = 3u64;

        let key =
    "9d8230420100cee4caee63d7385e6a784baa228efcceabdfed8d04f9705cdbe1f3fabf8295d89e8a79c235ab11b5aaff830b0569936afd254c"
            .parse::<LocalWallet>()
            .unwrap()
            .with_network_id(network_id);

        let client = SignerMiddleware::new(provider, key);

        let address_to = "cb81a111da2c7ec8cee4baa791504379a6230fd1c7af".parse::<Address>()?;
        let address_to = "ab36393ecaa2d3209cee16ce9b2360e327ed3c923346".parse::<Address>()?;

        let tx = TransactionRequest::new()
            .to(address_to)
            .value(U256::from(utils::parse_units("1", "wei")?));

        let receipt = client.send_transaction(tx, None).await?.await?;
        Ok(())
    }

    #[tokio::test]
    async fn signs_tx() {
        // retrieved test vector from:
        // https://web3js.readthedocs.io/en/v1.2.0/web3-eth-accounts.html#eth-accounts-signtransaction
        let tx = TransactionRequest {
            from: None,
            to: Some(
                "0000F0109fC8DF283027b6285cc889F5aA624EaC1F55".parse::<Address>().unwrap().into(),
            ),
            value: Some(1_000_000_000.into()),
            energy: Some(2_000_000.into()),
            nonce: Some(0.into()),
            energy_price: Some(21_000_000_000u128.into()),
            data: None,
            network_id: None,
        }
        .into();
        let network_id = 1u64;

        // Signer middlewares now rely on a working provider which it can query the network idfrom,
        // so we make sure Shuttle is started with the network id that the expected txwas signed
        // with
        let shuttle =
            Shuttle::new().args(vec!["--chain-id".to_string(), network_id.to_string()]).spawn();
        let provider = Provider::try_from(shuttle.endpoint()).unwrap();
        let key = "ffffffffff8c88cb64309d5bb5f0b5ac13156a48bd297ef08a060f4b43468947b8922c0a89b8c5c16c95215206d2abef34a73ed96b1aaffff7"
            .parse::<LocalWallet>()
            .unwrap()
            .with_network_id(network_id);
        let client = SignerMiddleware::new(provider, key);

        let tx = client.sign_transaction(tx).await.unwrap();

        assert_eq!(
            sha3(&tx)[..],
            hex::decode("9db3116c3e853e72084d754ad14ee3e3829ce69aaf1db95b1d073e5cb214dd4f")
                .unwrap()
        );

        let expected_rlp =
    Bytes::from(hex::decode("
    f86b808504e3b29200831e8480960000f0109fc8df283027b6285cc889f5aa624eac1f55843b9aca008025a0f9fa41caed9b9b79fb6c34e54d43a9da5725d92c1d53f5b0a9078eddb63118aea01da56f076c9fe3f40488b7a3627829760f86a1fc931396bea1e7284d6d61315c"
    ).unwrap());
        assert_eq!(tx, expected_rlp);
    }

    #[tokio::test]
    async fn signs_tx_none_networkid() {
        // retrieved test vector from:
        // https://web3js.readthedocs.io/en/v1.2.0/web3-eth-accounts.html#eth-accounts-signtransaction
        // the signature is different because we're testing signer middleware handling the None
        // case for a non-mainnet network id
        let tx = TransactionRequest {
            from: None,
            to: Some(
                "0000F0109fC8DF283027b6285cc889F5aA624EaC1F55".parse::<Address>().unwrap().into(),
            ),
            value: Some(1_000_000_000.into()),
            energy: Some(2_000_000.into()),
            nonce: Some(U256::zero()),
            energy_price: Some(21_000_000_000u128.into()),
            data: None,
            network_id: None,
        }
        .into();
        let network_id = 1337u64;

        // Signer middlewares now rely on a working provider which it can query the network id from
        // so we make sure Shuttle is started with the network id that the expected txwas signed with
        let shuttle =
            Shuttle::new().args(vec!["--chain-id".to_string(), network_id.to_string()]).spawn();
        let provider = Provider::try_from(shuttle.endpoint()).unwrap();
        let key = "ffffffffff8c88cb64309d5bb5f0b5ac13156a48bd297ef08a060f4b43468947b8922c0a89b8c5c16c95215206d2abef34a73ed96b1aaffff7"
            .parse::<LocalWallet>()
            .unwrap()
            .with_network_id(network_id);
        let client = SignerMiddleware::new(provider, key);

        let tx = client.sign_transaction(tx).await.unwrap();

        let expected_rlp =
            Bytes::from(hex::decode("
            f86d808504e3b29200831e8480960000f0109fc8df283027b6285cc889f5aa624eac1f55843b9aca0080820a95a08e5140d537e01ce53ffe46f8b573bf44af35ff1a873dcc0fca2109c77270a84ba0376029800db10847a100ab4cb91d8c838aa889931498dba2b05b41cf034fa736"
            ).unwrap());
        assert_eq!(tx, expected_rlp);
    }

    #[tokio::test]
    async fn shuttle_consistent_networkid() {
        let shuttle = Shuttle::new().spawn();
        let provider = Provider::try_from(shuttle.endpoint()).unwrap();
        let network_id = provider.get_networkid().await.unwrap();
        assert_eq!(network_id, U256::from(31337));

        // Intentionally do not set the network id here so we ensure that the signer pulls the
        // provider's network id.
        let key = LocalWallet::new(&mut rand::thread_rng(), Network::Mainnet);

        // combine the provider and wallet and test that the network id is the same for both the
        // signer returned by the middleware and through the middleware itself.
        let client = SignerMiddleware::new_with_provider_network(provider, key).await.unwrap();
        let middleware_networkid = client.get_networkid().await.unwrap();
        assert_eq!(network_id, middleware_networkid);

        let signer = client.signer();
        let signer_networkid = signer.network_id();
        assert_eq!(network_id.as_u64(), signer_networkid);
    }

    #[tokio::test]
    async fn shuttle_consistent_networkid_not_default() {
        let shuttle = Shuttle::new().args(vec!["--chain-id", "13371337"]).spawn();
        let provider = Provider::try_from(shuttle.endpoint()).unwrap();
        let network_id = provider.get_networkid().await.unwrap();
        assert_eq!(network_id, U256::from(13371337));

        // Intentionally do not set the network id here so we ensure that the signer pulls the
        // provider's network id.
        let key = LocalWallet::new(&mut rand::thread_rng(), Network::Mainnet);

        // combine the provider and wallet and test that the network id is the same for both the
        // signer returned by the middleware and through the middleware itself.
        let client = SignerMiddleware::new_with_provider_network(provider, key).await.unwrap();
        let middleware_networkid = client.get_networkid().await.unwrap();
        assert_eq!(network_id, middleware_networkid);

        let signer = client.signer();
        let signer_networkid = signer.network_id();
        assert_eq!(network_id.as_u64(), signer_networkid);
    }

    #[tokio::test]
    async fn handles_tx_from_field() {
        let shuttle = Shuttle::new().spawn();
        let acc = shuttle.addresses()[0];
        let provider = Provider::try_from(shuttle.endpoint()).unwrap();
        let key = LocalWallet::new(&mut rand::thread_rng(), Network::Mainnet).with_network_id(1u32);
        provider
            .send_transaction(
                TransactionRequest::pay(key.address(), utils::parse_core(1u64).unwrap()).from(acc),
                None,
            )
            .await
            .unwrap()
            .await
            .unwrap()
            .unwrap();
        let client = SignerMiddleware::new_with_provider_network(provider, key).await.unwrap();

        let request = TransactionRequest::new();

        // signing a TransactionRequest with a from field of None should yield
        // a signed transaction from the signer address
        let request_from_none = request.clone();
        let hash = *client.send_transaction(request_from_none, None).await.unwrap();
        let tx = client.get_transaction(hash).await.unwrap().unwrap();
        assert_eq!(tx.from, client.address());

        // signing a TransactionRequest with the signer as the from address
        // should yield a signed transaction from the signer
        let request_from_signer = request.clone().from(client.address());
        let hash = *client.send_transaction(request_from_signer, None).await.unwrap();
        let tx = client.get_transaction(hash).await.unwrap().unwrap();
        assert_eq!(tx.from, client.address());

        // signing a TransactionRequest with a from address that is not the
        // signer should result in the default shuttle account being used
        let request_from_other = request.from(acc);
        let hash = *client.send_transaction(request_from_other, None).await.unwrap();
        let tx = client.get_transaction(hash).await.unwrap().unwrap();
        assert_eq!(tx.from, acc);
    }
}
