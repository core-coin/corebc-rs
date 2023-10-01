//! [Ethereum Name Service](https://docs.ens.domains/) support
//! Adapted from <https://github.com/hhatto/rust-ens/blob/master/src/lib.rs>
use corebc_core::{
    types::{Address, NameOrAddress, Selector, TransactionRequest, H176, H256},
    utils::sha3,
};

use std::convert::TryInto;

/// ENS registry address (`0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e`)
/// CORETODO: Here should be the real cns address
pub const CNS_ADDRESS: Address = H176([
    // cannot set type aliases as constructors
    0, 0, 0, 0, 0, 0, 0, 12, 46, 7, 78, 198, 154, 13, 251, 41, 151, 186, 108, 125, 46, 30,
]);

// Selectors
const CNS_REVERSE_REGISTRAR_DOMAIN: &str = "addr.reverse";

/// resolver(bytes32)
const RESOLVER: Selector = [1, 120, 184, 191];

/// addr(bytes32)
pub const ADDR_SELECTOR: Selector = [59, 59, 87, 222];

/// name(bytes32)
pub const NAME_SELECTOR: Selector = [105, 31, 52, 49];

/// text(bytes32, string)
pub const FIELD_SELECTOR: Selector = [89, 209, 212, 60];

/// supportsInterface(bytes4 interfaceID)
pub const INTERFACE_SELECTOR: Selector = [1, 255, 201, 167];

/// Returns a transaction request for calling the `resolver` method on the ENS server
pub fn get_resolver<T: Into<NameOrAddress>>(ens_address: T, name: &str) -> TransactionRequest {
    // keccak256('resolver(bytes32)')
    let data = [&RESOLVER[..], &namehash(name).0].concat();
    TransactionRequest {
        data: Some(data.into()),
        to: Some(ens_address.into()),
        ..Default::default()
    }
}

/// Returns a transaction request for checking interface support
pub fn supports_interface<T: Into<NameOrAddress>>(
    resolver_address: T,
    selector: Selector,
) -> TransactionRequest {
    let data = [&INTERFACE_SELECTOR[..], &selector[..], &[0; 28]].concat();
    TransactionRequest {
        data: Some(data.into()),
        to: Some(resolver_address.into()),
        ..Default::default()
    }
}

/// Returns a transaction request for calling
pub fn resolve<T: Into<NameOrAddress>>(
    resolver_address: T,
    selector: Selector,
    name: &str,
    parameters: Option<&[u8]>,
) -> TransactionRequest {
    let data = [&selector[..], &namehash(name).0, parameters.unwrap_or_default()].concat();
    TransactionRequest {
        data: Some(data.into()),
        to: Some(resolver_address.into()),
        ..Default::default()
    }
}

/// Returns the reverse-registrar name of an address.
pub fn reverse_address(addr: Address) -> String {
    format!("{addr:?}.{CNS_REVERSE_REGISTRAR_DOMAIN}")[2..].to_string()
}

/// Returns the ENS namehash as specified in [EIP-137](https://eips.ethereum.org/EIPS/eip-137)
pub fn namehash(name: &str) -> H256 {
    if name.is_empty() {
        return H256::zero()
    }

    // iterate in reverse
    name.rsplit('.')
        .fold([0u8; 32], |node, label| sha3([node, sha3(label.as_bytes())].concat()))
        .into()
}

/// Returns a number in bytes form with padding to fit in 32 bytes.
pub fn bytes_32ify(n: u64) -> Vec<u8> {
    let b = n.to_be_bytes();
    [[0; 32][b.len()..].to_vec(), b.to_vec()].concat()
}

/// Returns the ENS record key hash [EIP-634](https://eips.ethereum.org/EIPS/eip-634)
pub fn parameterhash(name: &str) -> Vec<u8> {
    let bytes = name.as_bytes();
    let key_bytes =
        [&bytes_32ify(64), &bytes_32ify(bytes.len().try_into().unwrap()), bytes].concat();
    match key_bytes.len() % 32 {
        0 => key_bytes,
        n => [key_bytes, [0; 32][n..].to_vec()].concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parametershash() {
        assert_eq!(
            parameterhash("avatar").to_vec(),
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 6, 97, 118, 97, 116, 97, 114, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }
}
