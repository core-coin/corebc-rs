//! Various utilities for manipulating Ethereum related data.

use ethabi::ethereum_types::H256;
use tiny_keccak::{Hasher, Sha3};

/// Hash a message according to [EIP-191] (version `0x01`).
///
/// The final message is a UTF-8 string, encoded as follows:
/// `"\x19Ethereum Signed Message:\n" + message.length + message`
///
/// This message is then hashed using [Keccak-256](keccak256).
///
/// [EIP-191]: https://eips.ethereum.org/EIPS/eip-191
/// CORETODO: Change the prefix.
pub fn hash_message<T: AsRef<[u8]>>(message: T) -> H256 {
    const PREFIX: &str = "\x19Ethereum Signed Message:\n";

    let message = message.as_ref();
    let len = message.len();
    let len_string = len.to_string();

    let mut eth_message = Vec::with_capacity(PREFIX.len() + len_string.len() + len);
    eth_message.extend_from_slice(PREFIX.as_bytes());
    eth_message.extend_from_slice(len_string.as_bytes());
    eth_message.extend_from_slice(message);

    H256(sha3(&eth_message))
}

/// Compute the Keccak-256 hash of input bytes.
///
/// Note that strings are interpreted as UTF-8 bytes,
// TODO: Add Solidity Keccak256 packing support
pub fn sha3<T: AsRef<[u8]>>(bytes: T) -> [u8; 32] {
    let mut output = [0u8; 32];

    let mut hasher = Sha3::v256();
    hasher.update(bytes.as_ref());
    hasher.finalize(&mut output);

    output
}

/// Calculate the function selector as per the contract ABI specification. This
/// is defined as the first 4 bytes of the Keccak256 hash of the function
/// signature.
pub fn id<S: AsRef<str>>(signature: S) -> [u8; 4] {
    let mut output = [0u8; 4];

    let mut hasher = Sha3::v256();
    hasher.update(signature.as_ref().as_bytes());
    hasher.finalize(&mut output);

    output
}

/// Serialize a type.
///
/// # Panics
///
/// If the type returns an error during serialization.
pub fn serialize<T: serde::Serialize>(t: &T) -> serde_json::Value {
    serde_json::to_value(t).expect("Failed to serialize value")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // from https://emn178.github.io/online-tools/keccak_256.html
    fn test_sha3() {
        assert_eq!(
            hex::encode(sha3(b"hello")),
            "3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392"
        );
    }

    // test vector taken from:
    // https://web3js.readthedocs.io/en/v1.2.2/web3-eth-accounts.html#hashmessage
    #[test]
    fn test_hash_message() {
        let hash = hash_message("Hello World");

        assert_eq!(
            hash,
            "0xa52b31ab9ca0eb0c1e15f1acd9b7a50113625a74b14a8916a63daed37c955799".parse().unwrap()
        );
    }

    #[test]
    fn simple_function_signature() {
        // test vector retrieved from
        // https://web3js.readthedocs.io/en/v1.2.4/web3-eth-abi.html#encodefunctionsignature
        assert_eq!(id("myMethod(uint256,string)"), [0x61, 0xe0, 0x2e, 0xb0]);
    }

    #[test]
    fn revert_function_signature() {
        assert_eq!(id("Error(string)"), [0x4e, 0x40, 0x1c, 0xbe]);
    }
}
