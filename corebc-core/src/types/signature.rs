// Code adapted from: https://github.com/tomusdrw/rust-web3/blob/master/src/api/accounts.rs
use crate::{
    types::{Address, Network, H1368, H256},
    utils::{hash_message, to_ican},
};
use ethabi::ethereum_types::H160;
use libgoldilocks::{errors::LibgoldilockErrors, goldilocks::ed448_verify_with_error};
use open_fastrlp::Decodable;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, str::FromStr};
use thiserror::Error;

/// An error involving a signature.
#[derive(Debug, Error)]
pub enum SignatureError {
    /// Invalid length, secp256k1 signatures are 65 bytes
    #[error("invalid signature length, got {0}, expected 65")]
    InvalidLength(usize),
    /// When parsing a signature from string to hex
    #[error(transparent)]
    DecodingError(#[from] hex::FromHexError),
    /// Thrown when signature verification failed (i.e. when the address that
    /// produced the signature did not match the expected address)
    #[error("Signature verification failed. Expected {0}, got {1}")]
    VerificationError(Address, Address),
    /// Internal error during signature recovery
    #[error(transparent)]
    ED448Error(#[from] LibgoldilockErrors),
    /// Error in recovering public key from signature
    #[error("Public key recovery error")]
    RecoveryError,
}

/// Recovery message data.
///
/// The message data can either be a binary message that is first hashed
/// according to EIP-191 and then recovered based on the signature or a
/// precomputed hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryMessage {
    /// Message bytes
    Data(Vec<u8>),
    /// Message hash
    Hash(H256),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
/// An ECDSA signature
pub struct Signature {
    /// Sig value
    pub sig: H1368,
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sig = <[u8; 171]>::from(self);
        write!(f, "{}", hex::encode(&sig[..]))
    }
}

// #[cfg(feature = "cip712")]
// impl Signature {
//     /// Recovers the ethereum address which was used to sign a given CIP712
//     /// typed data payload.
//     ///
//     /// Recovery signature data uses 'Electrum' notation, this means the `v`
//     /// value is expected to be either `27` or `28`.
//     pub fn recover_typed_data<T>(&self, payload: T) -> Result<Address, SignatureError>
//     where
//         T: super::transaction::cip712::Cip712,
//     {
//         let encoded = payload.encode_cip712().map_err(|_| SignatureError::RecoveryError)?;
//         self.recover(encoded)
//     }
// }

impl Signature {
    /// Verifies that signature on `message` was produced by `address`
    pub fn verify<M, A>(
        &self,
        message: M,
        network: &Network,
        address: A,
    ) -> Result<(), SignatureError>
    where
        M: Into<RecoveryMessage>,
        A: Into<Address>,
    {
        let address = address.into();
        let recovered = self.recover(message, network)?;
        if recovered != address {
            return Err(SignatureError::VerificationError(address, recovered))
        }

        Ok(())
    }

    /// Recovers the Ethereum address which was used to sign the given message.
    ///
    /// Recovery signature data uses 'Electrum' notation, this means the `v`
    /// value is expected to be either `27` or `28`.
    pub fn recover<M>(&self, message: M, network: &Network) -> Result<Address, SignatureError>
    where
        M: Into<RecoveryMessage>,
    {
        let message = message.into();
        let message_hash = match message {
            RecoveryMessage::Data(ref message) => hash_message(message),
            RecoveryMessage::Hash(hash) => hash,
        };

        let sig_pub_bytes = self.sig.to_fixed_bytes();
        let mut sig_bytes = [0u8; 114];
        let mut pub_bytes = [0u8; 57];
        sig_bytes.copy_from_slice(&sig_pub_bytes[0..114]);
        pub_bytes.copy_from_slice(&sig_pub_bytes[114..171]);

        ed448_verify_with_error(&pub_bytes, &sig_bytes, message_hash.as_ref())?;

        let hash = crate::utils::sha3(&pub_bytes[..]);

        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&hash[12..]);
        let addr = H160::from(bytes);
        // CORETODO: Change the networktype logic
        Ok(to_ican(&addr, network))
    }

    /// Copies and serializes `self` into a new `Vec` with the recovery id included
    #[allow(clippy::wrong_self_convention)]
    pub fn to_vec(&self) -> Vec<u8> {
        self.into()
    }

    /// Decodes a signature from RLP bytes, assuming no RLP header
    pub(crate) fn decode_signature(buf: &mut &[u8]) -> Result<Self, open_fastrlp::DecodeError> {
        let sig = H1368::decode(buf)?;
        Ok(Self { sig })
    }
}

impl open_fastrlp::Decodable for Signature {
    fn decode(buf: &mut &[u8]) -> Result<Self, open_fastrlp::DecodeError> {
        Self::decode_signature(buf)
    }
}

impl open_fastrlp::Encodable for Signature {
    fn length(&self) -> usize {
        self.sig.length()
    }
    fn encode(&self, out: &mut dyn bytes::BufMut) {
        self.sig.encode(out);
    }
}

impl<'a> TryFrom<&'a [u8]> for Signature {
    type Error = SignatureError;

    /// Parses a raw signature which is expected to be 65 bytes long where
    /// the first 32 bytes is the `r` value, the second 32 bytes the `s` value
    /// and the final byte is the `v` value in 'Electrum' notation.
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 171 {
            return Err(SignatureError::InvalidLength(bytes.len()))
        }

        let sig = H1368::from_slice(bytes);

        Ok(Signature { sig })
    }
}

impl FromStr for Signature {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        Signature::try_from(&bytes[..])
    }
}

impl From<&Signature> for [u8; 171] {
    fn from(src: &Signature) -> [u8; 171] {
        src.sig.to_fixed_bytes()
    }
}

impl From<Signature> for [u8; 171] {
    fn from(src: Signature) -> [u8; 171] {
        <[u8; 171]>::from(&src)
    }
}

impl From<&Signature> for Vec<u8> {
    fn from(src: &Signature) -> Vec<u8> {
        <[u8; 171]>::from(src).to_vec()
    }
}

impl From<Signature> for Vec<u8> {
    fn from(src: Signature) -> Vec<u8> {
        <[u8; 171]>::from(&src).to_vec()
    }
}

impl From<&[u8]> for RecoveryMessage {
    fn from(s: &[u8]) -> Self {
        s.to_owned().into()
    }
}

impl From<Vec<u8>> for RecoveryMessage {
    fn from(s: Vec<u8>) -> Self {
        RecoveryMessage::Data(s)
    }
}

impl From<&str> for RecoveryMessage {
    fn from(s: &str) -> Self {
        s.as_bytes().to_owned().into()
    }
}

impl From<String> for RecoveryMessage {
    fn from(s: String) -> Self {
        RecoveryMessage::Data(s.into_bytes())
    }
}

impl From<[u8; 32]> for RecoveryMessage {
    fn from(hash: [u8; 32]) -> Self {
        H256(hash).into()
    }
}

impl From<H256> for RecoveryMessage {
    fn from(hash: H256) -> Self {
        RecoveryMessage::Hash(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recover_web3_signature() {
        // https://web3js.readthedocs.io/en/v1.2.2/web3-eth-accounts.html#sign
        // CORETODO: To Fix this test we will need to create a library
        let signature = Signature::from_str(
            "0x611d178b128095022653965eb0ed3bc8bbea8e7891b5a121a102a5b29bb895770d204354dbbc67c5567186f92cdb58a601397dfe0022e0ce002c1333b6829c37c732fb909501f719df200ceaaa0e0a1533dc22e4c9c999406c071fee2858bc7c76c66d113ff1ac739564d465cd541b0d1e003761457fcdd53dba3dea5848c43aa54fe468284319f032945a3acb9bd4cd0fa7b7c901d978e9acd9eca43fa5b3c32b648c33dcc3f3169e8080"
        ).expect("could not parse signature");
        assert_eq!(
            signature.recover("Some data", &Network::Devin).unwrap(),
            Address::from_str("ab76fc37a3b370a1f22e2fe2f819c210895e098845ed").unwrap()
        );
    }

    #[test]
    fn signature_from_str() {
        let s1 = Signature::from_str(
            "0xda7c602b1be1d7d2d1cef75c4c299cc60fa92ce91504b793df5e522de40a762142c143efc91d963c83981dccc1ba443a82430ee1b9800b61804d1b78e8eb7f642c6cea29daced23fd52087f0c3f8b58c15e252152eb36376aa8298ddfa672ed140ae1dcf2d6a0a352ce08249f4cea93c17009700d3af503d84bc4187ba8c1943ac5553f6d2a5ab68af25a43c4fd436f9a5a2e3c9ac711c90e9cb57bf84f73093906fc331e58647b974b300"
        ).expect("could not parse 0x-prefixed signature");

        let s2 = Signature::from_str(
            "da7c602b1be1d7d2d1cef75c4c299cc60fa92ce91504b793df5e522de40a762142c143efc91d963c83981dccc1ba443a82430ee1b9800b61804d1b78e8eb7f642c6cea29daced23fd52087f0c3f8b58c15e252152eb36376aa8298ddfa672ed140ae1dcf2d6a0a352ce08249f4cea93c17009700d3af503d84bc4187ba8c1943ac5553f6d2a5ab68af25a43c4fd436f9a5a2e3c9ac711c90e9cb57bf84f73093906fc331e58647b974b300"
        ).expect("could not parse non-prefixed signature");

        assert_eq!(s1, s2);
    }
}
