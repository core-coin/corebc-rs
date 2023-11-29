//! Specific helper functions for loading an offline K256 Private Key stored on disk
use super::Wallet;

use crate::wallet::mnemonic::MnemonicBuilderError;
use coins_bip32::Bip32Error;
use coins_bip39::MnemonicError;
use corebc_core::{
    libgoldilocks::{errors::LibgoldilockErrors, SigningKey},
    rand::{CryptoRng, Rng},
    types::Network,
    utils::secret_key_to_address,
};
#[cfg(not(target_arch = "wasm32"))]
use corebc_keystore::KeystoreError;
#[cfg(not(target_arch = "wasm32"))]
use elliptic_curve::rand_core;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
/// Error thrown by the Wallet module
pub enum WalletError {
    /// Error propagated from the BIP-32 crate
    #[error(transparent)]
    Bip32Error(#[from] Bip32Error),
    /// Error propagated from the BIP-39 crate
    #[error(transparent)]
    Bip39Error(#[from] MnemonicError),
    /// Underlying eth keystore error
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    EthKeystoreError(#[from] KeystoreError),
    /// Error propagated from k256's ECDSA module
    #[error(transparent)]
    ED448Error(#[from] LibgoldilockErrors),
    /// Error propagated from the hex crate.
    #[error(transparent)]
    HexError(#[from] hex::FromHexError),
    /// Error propagated by IO operations
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    /// Error propagated from the mnemonic builder module.
    #[error(transparent)]
    MnemonicBuilderError(#[from] MnemonicBuilderError),
    /// Error type from Cip712Error message
    #[error("error encoding cip712 struct: {0:?}")]
    Cip712Error(String),
}

impl Wallet<SigningKey> {
    /// Creates a new random encrypted JSON with the provided password and stores it in the
    /// provided directory. Returns a tuple (Wallet, String) of the wallet instance for the
    /// keystore with its random UUID. Accepts an optional name for the keystore file. If `None`,
    /// the keystore is stored as the stringified UUID.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_keystore<P, R, S>(
        dir: P,
        rng: &mut R,
        password: S,
        name: Option<&str>,
        network: Network,
    ) -> Result<(Self, String), WalletError>
    where
        P: AsRef<Path>,
        R: Rng + CryptoRng + rand_core::CryptoRng,
        S: AsRef<[u8]>,
    {
        let (secret, uuid) = corebc_keystore::new(dir, rng, password, name, &network)?;
        let signer = SigningKey::from_bytes(secret.as_slice())?;
        let address = secret_key_to_address(&signer, &network);
        Ok((Self { signer, address, network_id: 1 }, uuid))
    }

    /// Decrypts an encrypted JSON from the provided path to construct a Wallet instance
    #[cfg(not(target_arch = "wasm32"))]
    pub fn decrypt_keystore<P, S>(
        keypath: P,
        password: S,
        network: Network,
    ) -> Result<Self, WalletError>
    where
        P: AsRef<Path>,
        S: AsRef<[u8]>,
    {
        let secret = corebc_keystore::decrypt_key(keypath, password)?;
        let signer = SigningKey::from_bytes(secret.as_slice())?;
        let address = secret_key_to_address(&signer, &network);
        Ok(Self { signer, address, network_id: 1 })
    }

    /// Creates a new random keypair seeded with the provided Network
    pub fn new<R: Rng + CryptoRng>(rng: &mut R, network: Network) -> Self {
        let signer = SigningKey::random(rng);
        let address = secret_key_to_address(&signer, &network);
        Self { signer, address, network_id: 1 }
    }

    /// Creates a new Wallet instance from a raw scalar value (big endian).
    pub fn from_bytes(bytes: &[u8], network: Network) -> Result<Self, WalletError> {
        let signer = SigningKey::from_bytes(bytes)?;
        let address = secret_key_to_address(&signer, &network);
        Ok(Self { signer, address, network_id: 1 })
    }
}

impl PartialEq for Wallet<SigningKey> {
    fn eq(&self, other: &Self) -> bool {
        self.signer.to_bytes().eq(&other.signer.to_bytes()) &&
            self.address == other.address &&
            self.network_id == other.network_id
    }
}

impl From<SigningKey> for Wallet<SigningKey> {
    fn from(signer: SigningKey) -> Self {
        let network = Network::Mainnet;
        let address = secret_key_to_address(&signer, &network);

        Self { signer, address, network_id: 1 }
    }
}

use corebc_core::libgoldilocks::SecretKey as Ed448SecretKey;

impl From<Ed448SecretKey> for Wallet<SigningKey> {
    fn from(key: Ed448SecretKey) -> Self {
        let network = Network::Mainnet;
        let signer = key.into();
        let address = secret_key_to_address(&signer, &network);

        Self { signer, address, network_id: 1 }
    }
}

impl FromStr for Wallet<SigningKey> {
    type Err = WalletError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let src = src.strip_prefix("0x").or_else(|| src.strip_prefix("0X")).unwrap_or(src);
        let src = hex::decode(src)?;
        let sk = SigningKey::from_bytes(src.as_slice())?;
        Ok(sk.into())
    }
}

impl TryFrom<&str> for Wallet<SigningKey> {
    type Error = WalletError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for Wallet<SigningKey> {
    type Error = WalletError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::Signer;
    use corebc_core::types::Address;
    use tempfile::tempdir;

    #[test]
    fn parse_pk() {
        let s = "c6447b83ce0fd138cea4574d35edba162e57f8762935e6652d63805253860a254ef9199ad708423c2ab1434f5e5dac43014ddc5daa88c99b1f";
        let _pk: Wallet<SigningKey> = s.parse().unwrap();
    }

    #[tokio::test]
    async fn encrypted_json_keystore() {
        // create and store a random encrypted JSON keystore in this directory
        let dir = tempdir().unwrap();
        let mut rng = rand::thread_rng();
        let (key, uuid) =
            Wallet::<SigningKey>::new_keystore(&dir, &mut rng, "randpsswd", None, Network::Mainnet)
                .unwrap();

        // sign a message using the above key
        let message = "Some data";
        let signature = key.sign_message(message).await.unwrap();

        // read from the encrypted JSON keystore and decrypt it, while validating that the
        // signatures produced by both the keys should match
        let path = Path::new(dir.path()).join(uuid);
        let key2 =
            Wallet::<SigningKey>::decrypt_keystore(path.clone(), "randpsswd", Network::Mainnet)
                .unwrap();
        let signature2 = key2.sign_message(message).await.unwrap();
        assert_eq!(signature, signature2);
        std::fs::remove_file(&path).unwrap();
    }

    #[tokio::test]
    async fn signs_msg() {
        let message = "Some data";
        let hash = corebc_core::utils::hash_message(message);
        let key = Wallet::<SigningKey>::new(&mut rand::thread_rng(), Network::Mainnet);
        let address = key.address;

        // sign a message
        let signature = key.sign_message(message).await.unwrap();

        // ecrecover via the message will hash internally
        let recovered = signature.recover(message, &Network::Mainnet).unwrap();

        // if provided with a hash, it will skip hashing
        let recovered2 = signature.recover(hash, &Network::Mainnet).unwrap();

        // verifies the signature is produced by `address`
        signature.verify(message, &Network::Mainnet, address).unwrap();

        assert_eq!(recovered, address);
        assert_eq!(recovered2, address);
    }

    #[tokio::test]
    async fn signs_tx() {
        use crate::TypedTransaction;
        use corebc_core::types::{TransactionRequest, U64};
        // retrieved test vector from:
        // https://web3js.readthedocs.io/en/v1.2.0/web3-eth-accounts.html#eth-accounts-signtransaction
        let tx: TypedTransaction = TransactionRequest {
            from: None,
            to: Some(
                "cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839".parse::<Address>().unwrap().into(),
            ),
            value: Some(1_000_000_000.into()),
            energy: Some(2_000_000.into()),
            nonce: Some(0.into()),
            energy_price: Some(21_000_000_000u128.into()),
            data: None,
            network_id: Some(U64::one()),
        }
        .into();
        let wallet: Wallet<SigningKey> =
            "c6447b83ce0fd138cea4574d35edba162e57f8762935e6652d63805253860a254ef9199ad708423c2ab1434f5e5dac43014ddc5daa88c99b1f".parse().unwrap();
        let wallet = wallet.with_network_id(tx.network_id().unwrap().as_u64());

        let sig = wallet.sign_transaction(&tx).await.unwrap();
        let sighash = tx.sighash();
        sig.verify(sighash, &Network::Mainnet, wallet.address).unwrap();
    }

    #[tokio::test]
    async fn signs_tx_empty_network_id() {
        use crate::TypedTransaction;
        use corebc_core::types::TransactionRequest;
        // retrieved test vector from:
        // https://web3js.readthedocs.io/en/v1.2.0/web3-eth-accounts.html#eth-accounts-signtransaction
        let tx: TypedTransaction = TransactionRequest {
            from: None,
            to: Some(
                "cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839".parse::<Address>().unwrap().into(),
            ),
            value: Some(1_000_000_000.into()),
            energy: Some(2_000_000.into()),
            nonce: Some(0.into()),
            energy_price: Some(21_000_000_000u128.into()),
            data: None,
            network_id: None,
        }
        .into();
        let wallet: Wallet<SigningKey> =
            "c6447b83ce0fd138cea4574d35edba162e57f8762935e6652d63805253860a254ef9199ad708423c2ab1434f5e5dac43014ddc5daa88c99b1f".parse().unwrap();
        let wallet = wallet.with_network_id(1u64);

        // this should populate the tx network_id as the signer's network_id (1) before signing
        let sig = wallet.sign_transaction(&tx).await.unwrap();

        // since we initialize with None we need to re-set the network_id for the sighash to be
        // correct
        let mut tx = tx;
        tx.set_network_id(1);
        let sighash = tx.sighash();
        sig.verify(sighash, &Network::Mainnet, wallet.address).unwrap();
    }

    #[test]
    fn signs_tx_empty_network_id_sync() {
        use crate::TypedTransaction;
        use corebc_core::types::TransactionRequest;

        let network_id = 1337u64;
        // retrieved test vector from:
        // https://web3js.readthedocs.io/en/v1.2.0/web3-eth-accounts.html#eth-accounts-signtransaction
        let tx: TypedTransaction = TransactionRequest {
            from: None,
            to: Some(
                "ce15d3649d846a2bd426c0ceaca24fab50f7cba8f839".parse::<Address>().unwrap().into(),
            ),
            value: Some(1_000_000_000u64.into()),
            energy: Some(2_000_000u64.into()),
            nonce: Some(0u64.into()),
            energy_price: Some(21_000_000_000u128.into()),
            data: None,
            network_id: None,
        }
        .into();
        let wallet: Wallet<SigningKey> =
            "c6447b83ce0fd138cea4574d35edba162e57f8762935e6652d63805253860a254ef9199ad708423c2ab1434f5e5dac43014ddc5daa88c99b1f".parse().unwrap();
        let wallet = wallet.with_network_id(network_id);

        // this should populate the tx network_id as the signer's network_id (1337) before signing
        // and normalize the v
        let sig = wallet.sign_transaction_sync(&tx).unwrap();

        // since we initialize with None we need to re-set the network_id for the sighash to be
        // correct
        let mut tx = tx;
        tx.set_network_id(network_id);
        let sighash = tx.sighash();
        let network = Network::try_from(network_id).unwrap();
        sig.verify(sighash, &network, wallet.address).unwrap();
    }

    #[test]
    fn key_to_address() {
        let wallet: Wallet<SigningKey> =
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".parse().unwrap();
        assert_eq!(
            wallet.address,
            Address::from_str("0xcb58e5dd06163a480c22d540ec763325a0b5860fb56c")
                .expect("Decoding failed")
        );

        let wallet: Wallet<SigningKey> =
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002".parse().unwrap();
        assert_eq!(
            wallet.address,
            Address::from_str("0xcb732536ad1a311f40a2f2cd1871246685d572afe700")
                .expect("Decoding failed")
        );

        let wallet: Wallet<SigningKey> =
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003".parse().unwrap();
        assert_eq!(
            wallet.address,
            Address::from_str("0xcb671298e5136e4f115805d292170a8c66b4d595fda9")
                .expect("Decoding failed")
        );
    }

    #[test]
    fn key_from_bytes() {
        let wallet: Wallet<SigningKey> =
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".parse().unwrap();

        let key_as_bytes = wallet.signer.to_bytes();
        let wallet_from_bytes = Wallet::from_bytes(&key_as_bytes, Network::Mainnet).unwrap();

        assert_eq!(wallet.address, wallet_from_bytes.address);
        assert_eq!(wallet.network_id, wallet_from_bytes.network_id);
        assert_eq!(wallet.signer, wallet_from_bytes.signer);
    }

    #[test]
    fn key_from_str() {
        let wallet: Wallet<SigningKey> =
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".parse().unwrap();

        // Check FromStr and `0x`
        let wallet_0x: Wallet<SigningKey> =
            "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".parse().unwrap();
        assert_eq!(wallet.address, wallet_0x.address);
        assert_eq!(wallet.network_id, wallet_0x.network_id);
        assert_eq!(wallet.signer, wallet_0x.signer);

        // Check FromStr and `0X`
        let wallet_0x_cap: Wallet<SigningKey> =
            "0X000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001".parse().unwrap();
        assert_eq!(wallet.address, wallet_0x_cap.address);
        assert_eq!(wallet.network_id, wallet_0x_cap.network_id);
        assert_eq!(wallet.signer, wallet_0x_cap.signer);

        // Check TryFrom<&str>
        let wallet_0x_tryfrom_str: Wallet<SigningKey> =
            "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .try_into()
                .unwrap();
        assert_eq!(wallet.address, wallet_0x_tryfrom_str.address);
        assert_eq!(wallet.network_id, wallet_0x_tryfrom_str.network_id);
        assert_eq!(wallet.signer, wallet_0x_tryfrom_str.signer);

        // Check TryFrom<String>
        let wallet_0x_tryfrom_string: Wallet<SigningKey> =
            "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .to_string()
                .try_into()
                .unwrap();
        assert_eq!(wallet.address, wallet_0x_tryfrom_string.address);
        assert_eq!(wallet.network_id, wallet_0x_tryfrom_string.network_id);
        assert_eq!(wallet.signer, wallet_0x_tryfrom_string.signer);

        // Must fail because of `0z`
        "0z000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
            .parse::<Wallet<SigningKey>>()
            .unwrap_err();
    }
}
