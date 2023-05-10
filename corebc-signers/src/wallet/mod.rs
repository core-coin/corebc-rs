mod mnemonic;
pub use mnemonic::{MnemonicBuilder, MnemonicBuilderError};

mod private_key;
pub use private_key::WalletError;

#[cfg(all(feature = "yubihsm", not(target_arch = "wasm32")))]
mod yubi;

use crate::{to_eip155_v, Signer};
use corebc_core::{
    k256::{
        ecdsa::{signature::hazmat::PrehashSigner, RecoveryId, Signature as RecoverableSignature},
        elliptic_curve::FieldBytes,
        Secp256k1,
    },
    types::{
        transaction::{eip2718::TypedTransaction, eip712::Eip712},
        Address, Signature, H256, U256,
    },
    utils::hash_message,
};

use async_trait::async_trait;
use std::fmt;

/// An Ethereum private-public key pair which can be used for signing messages.
///
/// # Examples
///
/// ## Signing and Verifying a message
///
/// The wallet can be used to produce ECDSA [`Signature`] objects, which can be
/// then verified. Note that this uses [`hash_message`] under the hood which will
/// prefix the message being hashed with the `Ethereum Signed Message` domain separator.
///
/// ```
/// use corebc_core::rand::thread_rng;
/// use corebc_core::utils::NetworkType;
/// use corebc_signers::{LocalWallet, Signer};
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let wallet = LocalWallet::new(&mut thread_rng(), NetworkType::Mainnet);
///
/// // Optionally, the wallet's network id can be set, in order to use EIP-155
/// // replay protection with different networks
/// let wallet = wallet.with_network_id(1337u64);
///
/// // The wallet can be used to sign messages
/// let message = b"hello";
/// let signature = wallet.sign_message(message).await?;
/// assert_eq!(signature.recover(&message[..]).unwrap(), wallet.address());
///
/// // LocalWallet is clonable:
/// let wallet_clone = wallet.clone();
/// let signature2 = wallet_clone.sign_message(message).await?;
/// assert_eq!(signature, signature2);
/// # Ok(())
/// # }
/// ```
///
/// [`Signature`]: corebc_core::types::Signature
/// [`hash_message`]: fn@corebc_core::utils::hash_message
#[derive(Clone)]
pub struct Wallet<D: PrehashSigner<(RecoverableSignature, RecoveryId)>> {
    /// The Wallet's private Key
    pub(crate) signer: D,
    /// The wallet's address
    pub(crate) address: Address,
    /// The wallet's network id (for EIP-155)
    pub(crate) network_id: u64,
}

impl<D: PrehashSigner<(RecoverableSignature, RecoveryId)>> Wallet<D> {
    /// Construct a new wallet with an external Signer
    pub fn new_with_signer(signer: D, address: Address, network_id: u64) -> Self {
        Wallet { signer, address, network_id }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<D: Sync + Send + PrehashSigner<(RecoverableSignature, RecoveryId)>> Signer for Wallet<D> {
    type Error = WalletError;

    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error> {
        let message = message.as_ref();
        let message_hash = hash_message(message);

        self.sign_hash(message_hash)
    }

    async fn sign_transaction(&self, tx: &TypedTransaction) -> Result<Signature, Self::Error> {
        let mut tx_with_network = tx.clone();
        if tx_with_network.network_id().is_none() {
            // in the case we don't have a network_id, let's use the signer network id instead
            tx_with_network.set_network_id(self.network_id);
        }
        self.sign_transaction_sync(&tx_with_network)
    }

    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error> {
        let encoded =
            payload.encode_eip712().map_err(|e| Self::Error::Eip712Error(e.to_string()))?;

        self.sign_hash(H256::from(encoded))
    }

    fn address(&self) -> Address {
        self.address
    }

    /// Gets the wallet's network id
    fn network_id(&self) -> u64 {
        self.network_id
    }

    /// Sets the wallet's network_id, used in conjunction with EIP-155 signing
    fn with_network_id<T: Into<u64>>(mut self, network_id: T) -> Self {
        self.network_id = network_id.into();
        self
    }
}

impl<D: PrehashSigner<(RecoverableSignature, RecoveryId)>> Wallet<D> {
    /// Synchronously signs the provided transaction, normalizing the signature `v` value with
    /// EIP-155 using the transaction's `network_id`, or the signer's `network_id` if the
    /// transaction does not specify one.
    pub fn sign_transaction_sync(&self, tx: &TypedTransaction) -> Result<Signature, WalletError> {
        // rlp (for sighash) must have the same network id as v in the signature
        let network_id = tx.network_id().map(|id| id.as_u64()).unwrap_or(self.network_id);
        let mut tx = tx.clone();
        tx.set_network_id(network_id);

        let sighash = tx.sighash();
        let mut sig = self.sign_hash(sighash)?;

        // sign_hash sets `v` to recid + 27, so we need to subtract 27 before normalizing
        sig.v = to_eip155_v(sig.v as u8 - 27, network_id);
        Ok(sig)
    }

    /// Signs the provided hash.
    pub fn sign_hash(&self, hash: H256) -> Result<Signature, WalletError> {
        let (recoverable_sig, recovery_id) = self.signer.sign_prehash(hash.as_ref())?;

        let v = u8::from(recovery_id) as u64 + 27;

        let r_bytes: FieldBytes<Secp256k1> = recoverable_sig.r().into();
        let s_bytes: FieldBytes<Secp256k1> = recoverable_sig.s().into();
        let r = U256::from_big_endian(r_bytes.as_slice());
        let s = U256::from_big_endian(s_bytes.as_slice());

        Ok(Signature { r, s, v })
    }

    /// Gets the wallet's signer
    pub fn signer(&self) -> &D {
        &self.signer
    }
}

// do not log the signer
impl<D: PrehashSigner<(RecoverableSignature, RecoveryId)>> fmt::Debug for Wallet<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address)
            .field("network_Id", &self.network_id)
            .finish()
    }
}
