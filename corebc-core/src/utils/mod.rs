/// Utilities for launching a ganache-cli testnet instance
#[cfg(not(target_arch = "wasm32"))]
mod ganache;
#[cfg(not(target_arch = "wasm32"))]
pub use ganache::{Ganache, GanacheInstance};

/// Utilities for launching a go-ethereum dev-mode instance
#[cfg(not(target_arch = "wasm32"))]
mod geth;
#[cfg(not(target_arch = "wasm32"))]
pub use geth::{Geth, GethInstance};

/// Utilities for working with a `genesis.json` and other chain config structs.
mod genesis;
pub use genesis::{ChainConfig, CliqueConfig, EthashConfig, Genesis, GenesisAccount};

/// Utilities for launching an anvil instance
#[cfg(not(target_arch = "wasm32"))]
mod anvil;
#[cfg(not(target_arch = "wasm32"))]
pub use anvil::{Anvil, AnvilInstance};

/// Moonbeam utils
pub mod moonbeam;

mod hash;
pub use hash::{hash_message, id, serialize, sha3};

mod units;
use serde::{Deserialize, Deserializer};
pub use units::Units;

/// Re-export RLP
pub use rlp;

/// Re-export hex
pub use hex;

use crate::types::{Address, Bytes, ParseI256Error, H160, H256, I256, U256, U64};
use ethabi::ethereum_types::FromDecStrErr;
use k256::ecdsa::SigningKey;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fmt,
    str::FromStr,
};
use thiserror::Error;

/// I256 overflows for numbers wider than 77 units.
const OVERFLOW_I256_UNITS: usize = 77;
/// U256 overflows for numbers wider than 78 units.
const OVERFLOW_U256_UNITS: usize = 78;

const MAINNET: &str = "cb";
const TESTNET: &str = "ab";
const PRIVATE: &str = "ce";

// Re-export serde-json for macro usage
#[doc(hidden)]
pub use serde_json as __serde_json;

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Unknown units: {0}")]
    UnrecognizedUnits(String),
    #[error("bytes32 strings must not exceed 32 bytes in length")]
    TextTooLong,
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    InvalidFloat(#[from] std::num::ParseFloatError),
    #[error(transparent)]
    FromDecStrError(#[from] FromDecStrErr),
    #[error("Overflow parsing string")]
    ParseOverflow,
    #[error(transparent)]
    ParseI256Error(#[from] ParseI256Error),
}

#[derive(Debug)]
pub enum NetworkType {
    Mainnet,
    Testnet,
    Private,
}

/// 1 Ether = 1e18 Wei == 0x0de0b6b3a7640000 Wei
pub const WEI_IN_ETHER: U256 = U256([0x0de0b6b3a7640000, 0x0, 0x0, 0x0]);

/// The number of blocks from the past for which the fee rewards are fetched for fee estimation.
pub const EIP1559_FEE_ESTIMATION_PAST_BLOCKS: u64 = 10;
/// The default percentile of gas premiums that are fetched for fee estimation.
pub const EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE: f64 = 5.0;
/// The default max priority fee per gas, used in case the base fee is within a threshold.
pub const EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE: u64 = 3_000_000_000;
/// The threshold for base fee below which we use the default priority fee, and beyond which we
/// estimate an appropriate value for priority fee.
pub const EIP1559_FEE_ESTIMATION_PRIORITY_FEE_TRIGGER: u64 = 100_000_000_000;
/// The threshold max change/difference (in %) at which we will ignore the fee history values
/// under it.
pub const EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE: i64 = 200;

/// This enum holds the numeric types that a possible to be returned by `parse_units` and
/// that are taken by `format_units`.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum ParseUnits {
    U256(U256),
    I256(I256),
}

impl From<ParseUnits> for U256 {
    fn from(n: ParseUnits) -> Self {
        match n {
            ParseUnits::U256(n) => n,
            ParseUnits::I256(n) => n.into_raw(),
        }
    }
}

impl fmt::Display for ParseUnits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseUnits::U256(val) => val.fmt(f),
            ParseUnits::I256(val) => val.fmt(f),
        }
    }
}

macro_rules! construct_format_units_from {
    ($( $t:ty[$convert:ident] ),*) => {
        $(
            impl From<$t> for ParseUnits {
                fn from(num: $t) -> Self {
                    Self::$convert(num.into())
                }
            }
        )*
    }
}

// Generate the From<T> code for the given numeric types below.
construct_format_units_from! {
    u8[U256], u16[U256], u32[U256], u64[U256], u128[U256], U256[U256], usize[U256],
    i8[I256], i16[I256], i32[I256], i64[I256], i128[I256], I256[I256], isize[I256]
}

/// Format the output for the user which prefer to see values
/// in ether (instead of wei)
///
/// Divides the input by 1e18
pub fn format_ether<T: Into<U256>>(amount: T) -> U256 {
    amount.into() / WEI_IN_ETHER
}

/// Divides the provided amount with 10^{units} provided.
///
/// ```
/// use corebc_core::{types::U256, utils::format_units};
///
/// let eth = format_units(1395633240123456000_u128, "ether").unwrap();
/// assert_eq!(eth.parse::<f64>().unwrap(), 1.395633240123456);
///
/// let eth = format_units(U256::from_dec_str("1395633240123456000").unwrap(), "ether").unwrap();
/// assert_eq!(eth.parse::<f64>().unwrap(), 1.395633240123456);
///
/// let eth = format_units(U256::from_dec_str("1395633240123456789").unwrap(), "ether").unwrap();
/// assert_eq!(eth, "1.395633240123456789");
///
/// let eth = format_units(i64::MIN, "gwei").unwrap();
/// assert_eq!(eth, "-9223372036.854775808");
///
/// let eth = format_units(i128::MIN, 36).unwrap();
/// assert_eq!(eth, "-170.141183460469231731687303715884105728");
/// ```
pub fn format_units<T, K>(amount: T, units: K) -> Result<String, ConversionError>
where
    T: Into<ParseUnits>,
    K: TryInto<Units, Error = ConversionError>,
{
    let units: usize = units.try_into()?.into();
    let amount = amount.into();

    match amount {
        // 2**256 ~= 1.16e77
        ParseUnits::U256(_) if units >= OVERFLOW_U256_UNITS => {
            return Err(ConversionError::ParseOverflow)
        }
        // 2**255 ~= 5.79e76
        ParseUnits::I256(_) if units >= OVERFLOW_I256_UNITS => {
            return Err(ConversionError::ParseOverflow)
        }
        _ => {}
    };
    let exp10 = U256::exp10(units);

    // `decimals` are formatted twice because U256 does not support alignment (`:0>width`).
    match amount {
        ParseUnits::U256(amount) => {
            let integer = amount / exp10;
            let decimals = (amount % exp10).to_string();
            Ok(format!("{integer}.{decimals:0>units$}"))
        }
        ParseUnits::I256(amount) => {
            let exp10 = I256::from_raw(exp10);
            let sign = if amount.is_negative() { "-" } else { "" };
            let integer = (amount / exp10).twos_complement();
            let decimals = ((amount % exp10).twos_complement()).to_string();
            Ok(format!("{sign}{integer}.{decimals:0>units$}"))
        }
    }
}

/// Converts the input to a U256 and converts from Ether to Wei.
///
/// ```
/// use corebc_core::{types::U256, utils::{parse_ether, WEI_IN_ETHER}};
///
/// let eth = U256::from(WEI_IN_ETHER);
/// assert_eq!(eth, parse_ether(1u8).unwrap());
/// assert_eq!(eth, parse_ether(1usize).unwrap());
/// assert_eq!(eth, parse_ether("1").unwrap());
/// ```
pub fn parse_ether<S: ToString>(eth: S) -> Result<U256, ConversionError> {
    Ok(parse_units(eth, "ether")?.into())
}

/// Multiplies the provided amount with 10^{units} provided.
///
/// ```
/// use corebc_core::{types::U256, utils::parse_units};
/// let amount_in_eth = U256::from_dec_str("15230001000000000000").unwrap();
/// let amount_in_gwei = U256::from_dec_str("15230001000").unwrap();
/// let amount_in_wei = U256::from_dec_str("15230001000").unwrap();
/// assert_eq!(amount_in_eth, parse_units("15.230001000000000000", "ether").unwrap().into());
/// assert_eq!(amount_in_gwei, parse_units("15.230001000000000000", "gwei").unwrap().into());
/// assert_eq!(amount_in_wei, parse_units("15230001000", "wei").unwrap().into());
/// ```
/// Example of trying to parse decimal WEI, which should fail, as WEI is the smallest
/// ETH denominator. 1 ETH = 10^18 WEI.
/// ```should_panic
/// use corebc_core::{types::U256, utils::parse_units};
/// let amount_in_wei = U256::from_dec_str("15230001000").unwrap();
/// assert_eq!(amount_in_wei, parse_units("15.230001000000000000", "wei").unwrap().into());
/// ```
pub fn parse_units<K, S>(amount: S, units: K) -> Result<ParseUnits, ConversionError>
where
    S: ToString,
    K: TryInto<Units, Error = ConversionError> + Copy,
{
    let exponent: u32 = units.try_into()?.as_num();
    let mut amount_str = amount.to_string().replace('_', "");
    let negative = amount_str.chars().next().unwrap_or_default() == '-';
    let dec_len = if let Some(di) = amount_str.find('.') {
        amount_str.remove(di);
        amount_str[di..].len() as u32
    } else {
        0
    };

    if dec_len > exponent {
        // Truncate the decimal part if it is longer than the exponent
        let amount_str = &amount_str[..(amount_str.len() - (dec_len - exponent) as usize)];
        if negative {
            // Edge case: We have removed the entire number and only the negative sign is left.
            //            Return 0 as a I256 given the input was signed.
            if amount_str == "-" {
                Ok(ParseUnits::I256(I256::zero()))
            } else {
                Ok(ParseUnits::I256(I256::from_dec_str(amount_str)?))
            }
        } else {
            Ok(ParseUnits::U256(U256::from_dec_str(amount_str)?))
        }
    } else if negative {
        // Edge case: Only a negative sign was given, return 0 as a I256 given the input was signed.
        if amount_str == "-" {
            Ok(ParseUnits::I256(I256::zero()))
        } else {
            let mut n = I256::from_dec_str(&amount_str)?;
            n *= I256::from(10)
                .checked_pow(exponent - dec_len)
                .ok_or(ConversionError::ParseOverflow)?;
            Ok(ParseUnits::I256(n))
        }
    } else {
        let mut a_uint = U256::from_dec_str(&amount_str)?;
        a_uint *= U256::from(10)
            .checked_pow(U256::from(exponent - dec_len))
            .ok_or(ConversionError::ParseOverflow)?;
        Ok(ParseUnits::U256(a_uint))
    }
}

/// The address for an Ethereum contract is deterministically computed from the
/// address of its creator (sender) and how many transactions the creator has
/// sent (nonce). The sender and nonce are RLP encoded and then hashed with Keccak-256.
pub fn get_contract_address(
    sender: impl Into<Address>,
    nonce: impl Into<U256>,
    network: &NetworkType,
) -> Address {
    let mut stream = rlp::RlpStream::new();
    stream.begin_list(2);
    stream.append(&sender.into());
    stream.append(&nonce.into());

    let hash = sha3(&stream.out());

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    let addr = H160::from(bytes);

    to_ican(&addr, network)
}

/// Returns the CREATE2 address of a smart contract as specified in
/// [EIP1014](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1014.md)
///
/// keccak256( 0xff ++ senderAddress ++ salt ++ keccak256(init_code))[12..]
pub fn get_create2_address(
    from: impl Into<Address>,
    salt: impl AsRef<[u8]>,
    init_code: impl AsRef<[u8]>,
    network: NetworkType,
) -> Address {
    let init_code_hash = sha3(init_code.as_ref());
    get_create2_address_from_hash(from, salt, init_code_hash, network)
}

pub fn get_create2_address_from_hash(
    from: impl Into<Address>,
    salt: impl AsRef<[u8]>,
    init_code_hash: impl AsRef<[u8]>,
    network: NetworkType,
) -> Address {
    let from = from.into();
    let salt = salt.as_ref();
    let init_code_hash = init_code_hash.as_ref();

    let mut bytes = Vec::with_capacity(1 + 20 + salt.len() + init_code_hash.len());
    bytes.push(0xff);
    bytes.extend_from_slice(from.as_bytes());
    bytes.extend_from_slice(salt);
    bytes.extend_from_slice(init_code_hash);

    let hash = sha3(bytes);

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    let addr = H160::from(bytes);

    to_ican(&addr, &network)
}

pub fn to_ican(addr: &H160, network: &NetworkType) -> Address {
    let prefix = match network {
        NetworkType::Mainnet => MAINNET,
        NetworkType::Testnet => TESTNET,
        NetworkType::Private => PRIVATE,
    };

    let number_str = get_number_string(addr, network);

    let checksum = calculate_checksum(&number_str);

    construct_ican_address(prefix, &checksum, addr)
}

fn get_number_string(addr: &H160, network: &NetworkType) -> String {
    let prefix = match network {
        NetworkType::Mainnet => MAINNET,
        NetworkType::Testnet => TESTNET,
        NetworkType::Private => PRIVATE,
    };

    // We have to use the Debug trait for addr https://github.com/paritytech/parity-common/issues/656
    let mut addr_str = format!("{:?}{}{}", addr, prefix, "00");
    // Remove the 0x prefix
    addr_str = addr_str.replace("0x", "");

    // Convert every hex digit to decimal and then to String
    addr_str
        .chars()
        .map(|x| x.to_digit(16).expect("Invalid Address").to_string())
        .collect::<String>()
}

fn calculate_checksum(number_str: &str) -> u64 {
    // number_str % 97
    let result = number_str.chars().fold(0, |acc, ch| {
        let digit = ch.to_digit(10).expect("Invalid Digit") as u64;
        (acc * 10 + digit) % 97
    });

    98 - result
}

fn construct_ican_address(prefix: &str, checksum: &u64, addr: &H160) -> Address {
    // We need to use debug for the address https://github.com/paritytech/parity-common/issues/656
    let addr = format!("{:?}", addr);
    // Remove 0x prefix
    let addr = addr.replace("0x", "");

    // If the checksum is less than 10 we need to add a zero to the address
    if *checksum < 10 {
        Address::from_str(&format!("{prefix}{zero}{checksum}{addr}", zero = "0")).unwrap()
    } else {
        Address::from_str(&format!("{prefix}{checksum}{addr}")).unwrap()
    }
}

/// Converts a K256 SigningKey to an Ethereum Address
/// CORETODO: FIX ASAP ICAN ADDRESSES
pub fn secret_key_to_address(secret_key: &SigningKey, network: &NetworkType) -> Address {
    let public_key = secret_key.verifying_key();
    let public_key = public_key.to_encoded_point(/* compress = */ false);
    let public_key = public_key.as_bytes();
    debug_assert_eq!(public_key[0], 0x04);
    let hash = sha3(&public_key[1..]);

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    let addr = H160::from(bytes);
    to_ican(&addr, &network)
}

/// Encodes an Ethereum address to its [EIP-55] checksum.
///
/// You can optionally specify an [EIP-155 chain ID] to encode the address using the [EIP-1191]
/// extension.
///
/// [EIP-55]: https://eips.ethereum.org/EIPS/eip-55
/// [EIP-155 chain ID]: https://eips.ethereum.org/EIPS/eip-155
/// [EIP-1191]: https://eips.ethereum.org/EIPS/eip-1191
pub fn to_checksum(addr: &Address, chain_id: Option<u8>) -> String {
    let prefixed_addr = match chain_id {
        Some(chain_id) => format!("{chain_id}0x{addr:x}"),
        None => format!("{addr:x}"),
    };
    let hash = hex::encode(sha3(prefixed_addr));
    let hash = hash.as_bytes();

    let addr_hex = hex::encode(addr.as_bytes());
    let addr_hex = addr_hex.as_bytes();

    addr_hex.iter().zip(hash).fold("0x".to_owned(), |mut encoded, (addr, hash)| {
        encoded.push(if *hash >= 56 {
            addr.to_ascii_uppercase() as char
        } else {
            addr.to_ascii_lowercase() as char
        });
        encoded
    })
}

/// Returns a bytes32 string representation of text. If the length of text exceeds 32 bytes,
/// an error is returned.
pub fn format_bytes32_string(text: &str) -> Result<[u8; 32], ConversionError> {
    let str_bytes: &[u8] = text.as_bytes();
    if str_bytes.len() > 32 {
        return Err(ConversionError::TextTooLong)
    }

    let mut bytes32: [u8; 32] = [0u8; 32];
    bytes32[..str_bytes.len()].copy_from_slice(str_bytes);

    Ok(bytes32)
}

/// Returns the decoded string represented by the bytes32 encoded data.
pub fn parse_bytes32_string(bytes: &[u8; 32]) -> Result<&str, ConversionError> {
    let mut length = 0;
    while length < 32 && bytes[length] != 0 {
        length += 1;
    }

    Ok(std::str::from_utf8(&bytes[..length])?)
}

/// The default EIP-1559 fee estimator which is based on the work by [MyCrypto](https://github.com/MyCryptoHQ/MyCrypto/blob/master/src/services/ApiService/Gas/eip1559.ts)
pub fn eip1559_default_estimator(base_fee_per_gas: U256, rewards: Vec<Vec<U256>>) -> (U256, U256) {
    let max_priority_fee_per_gas =
        if base_fee_per_gas < U256::from(EIP1559_FEE_ESTIMATION_PRIORITY_FEE_TRIGGER) {
            U256::from(EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE)
        } else {
            std::cmp::max(
                estimate_priority_fee(rewards),
                U256::from(EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE),
            )
        };
    let potential_max_fee = base_fee_surged(base_fee_per_gas);
    let max_fee_per_gas = if max_priority_fee_per_gas > potential_max_fee {
        max_priority_fee_per_gas + potential_max_fee
    } else {
        potential_max_fee
    };
    (max_fee_per_gas, max_priority_fee_per_gas)
}

/// Converts a Bytes value into a H256, accepting inputs that are less than 32 bytes long. These
/// inputs will be left padded with zeros.
pub fn from_bytes_to_h256<'de, D>(bytes: Bytes) -> Result<H256, D::Error>
where
    D: Deserializer<'de>,
{
    if bytes.0.len() > 32 {
        return Err(serde::de::Error::custom("input too long to be a H256"))
    }

    // left pad with zeros to 32 bytes
    let mut padded = [0u8; 32];
    padded[32 - bytes.0.len()..].copy_from_slice(&bytes.0);

    // then convert to H256 without a panic
    Ok(H256::from_slice(&padded))
}

/// Deserializes the input into an Option<HashMap<H256, H256>>, using from_unformatted_hex to
/// deserialize the keys and values.
pub fn from_unformatted_hex_map<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<H256, H256>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map = Option::<HashMap<Bytes, Bytes>>::deserialize(deserializer)?;
    match map {
        Some(mut map) => {
            let mut res_map = HashMap::new();
            for (k, v) in map.drain() {
                let k_deserialized = from_bytes_to_h256::<'de, D>(k)?;
                let v_deserialized = from_bytes_to_h256::<'de, D>(v)?;
                res_map.insert(k_deserialized, v_deserialized);
            }
            Ok(Some(res_map))
        }
        None => Ok(None),
    }
}

/// Deserializes the input into a U256, accepting both 0x-prefixed hex and decimal strings with
/// arbitrary precision, defined by serde_json's [`Number`](serde_json::Number).
pub fn from_int_or_hex<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrString {
        Int(serde_json::Number),
        JsonString(String),
    }

    match IntOrString::deserialize(deserializer)? {
        IntOrString::JsonString(s) => {
            if s.starts_with("0x") {
                U256::from_str(s.as_str()).map_err(serde::de::Error::custom)
            } else {
                U256::from_dec_str(&s).map_err(serde::de::Error::custom)
            }
        }
        IntOrString::Int(n) => U256::from_dec_str(&n.to_string()).map_err(serde::de::Error::custom),
    }
}

/// Deserializes the input into a U64, accepting both 0x-prefixed hex and decimal strings with
/// arbitrary precision, defined by serde_json's [`Number`](serde_json::Number).
pub fn from_u64_or_hex<'de, D>(deserializer: D) -> Result<U64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrHex {
        Int(serde_json::Number),
        Hex(String),
    }

    match IntOrHex::deserialize(deserializer)? {
        IntOrHex::Hex(s) => {
            if s.starts_with("0x") {
                U64::from_str(s.as_str()).map_err(serde::de::Error::custom)
            } else {
                U64::from_dec_str(&s).map_err(serde::de::Error::custom)
            }
        }
        IntOrHex::Int(n) => U64::from_dec_str(&n.to_string()).map_err(serde::de::Error::custom),
    }
}

/// Deserializes the input into an `Option<U256>`, using [`from_int_or_hex`] to deserialize the
/// inner value.
pub fn from_int_or_hex_opt<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(from_int_or_hex(deserializer)?))
}

/// Deserializes the input into an `Option<u64>`, using [`from_u64_or_hex`] to deserialize the
/// inner value.
pub fn from_u64_or_hex_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(from_u64_or_hex(deserializer)?.as_u64()))
}

fn estimate_priority_fee(rewards: Vec<Vec<U256>>) -> U256 {
    let mut rewards: Vec<U256> =
        rewards.iter().map(|r| r[0]).filter(|r| *r > U256::zero()).collect();
    if rewards.is_empty() {
        return U256::zero()
    }
    if rewards.len() == 1 {
        return rewards[0]
    }
    // Sort the rewards as we will eventually take the median.
    rewards.sort();

    // A copy of the same vector is created for convenience to calculate percentage change
    // between subsequent fee values.
    let mut rewards_copy = rewards.clone();
    rewards_copy.rotate_left(1);

    let mut percentage_change: Vec<I256> = rewards
        .iter()
        .zip(rewards_copy.iter())
        .map(|(a, b)| {
            let a = I256::try_from(*a).expect("priority fee overflow");
            let b = I256::try_from(*b).expect("priority fee overflow");
            ((b - a) * 100) / a
        })
        .collect();
    percentage_change.pop();

    // Fetch the max of the percentage change, and that element's index.
    let max_change = percentage_change.iter().max().unwrap();
    let max_change_index = percentage_change.iter().position(|&c| c == *max_change).unwrap();

    // If we encountered a big change in fees at a certain position, then consider only
    // the values >= it.
    let values = if *max_change >= EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE.into() &&
        (max_change_index >= (rewards.len() / 2))
    {
        rewards[max_change_index..].to_vec()
    } else {
        rewards
    };

    // Return the median.
    values[values.len() / 2]
}

fn base_fee_surged(base_fee_per_gas: U256) -> U256 {
    if base_fee_per_gas <= U256::from(40_000_000_000u64) {
        base_fee_per_gas * 2
    } else if base_fee_per_gas <= U256::from(100_000_000_000u64) {
        base_fee_per_gas * 16 / 10
    } else if base_fee_per_gas <= U256::from(200_000_000_000u64) {
        base_fee_per_gas * 14 / 10
    } else {
        base_fee_per_gas * 12 / 10
    }
}

/// A bit of hack to find unused TCP ports.
///
/// Does not guarantee that the given port is unused after the function exists, just that it was
/// unused before the function started (i.e., it does not reserve a port).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn unused_ports<const N: usize>() -> [u16; N] {
    use std::net::{SocketAddr, TcpListener};

    std::array::from_fn(|_| {
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        TcpListener::bind(addr).expect("Failed to create TCP listener to find unused port")
    })
    .map(|listener| {
        listener
            .local_addr()
            .expect("Failed to read TCP listener local_addr to find unused port")
            .port()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn wei_in_ether() {
        assert_eq!(WEI_IN_ETHER.as_u64(), 1e18 as u64);
    }

    #[test]
    fn test_format_units_unsigned() {
        let gwei_in_ether = format_units(WEI_IN_ETHER, 9).unwrap();
        assert_eq!(gwei_in_ether.parse::<f64>().unwrap() as u64, 1e9 as u64);

        let eth = format_units(WEI_IN_ETHER, "ether").unwrap();
        assert_eq!(eth.parse::<f64>().unwrap() as u64, 1);

        let eth = format_units(1395633240123456000_u128, "ether").unwrap();
        assert_eq!(eth.parse::<f64>().unwrap(), 1.395633240123456);

        let eth =
            format_units(U256::from_dec_str("1395633240123456000").unwrap(), "ether").unwrap();
        assert_eq!(eth.parse::<f64>().unwrap(), 1.395633240123456);

        let eth =
            format_units(U256::from_dec_str("1395633240123456789").unwrap(), "ether").unwrap();
        assert_eq!(eth, "1.395633240123456789");

        let eth =
            format_units(U256::from_dec_str("1005633240123456789").unwrap(), "ether").unwrap();
        assert_eq!(eth, "1.005633240123456789");

        let eth = format_units(u8::MAX, 4).unwrap();
        assert_eq!(eth, "0.0255");

        let eth = format_units(u16::MAX, "ether").unwrap();
        assert_eq!(eth, "0.000000000000065535");

        // Note: This covers usize on 32 bit systems.
        let eth = format_units(u32::MAX, 18).unwrap();
        assert_eq!(eth, "0.000000004294967295");

        // Note: This covers usize on 64 bit systems.
        let eth = format_units(u64::MAX, "gwei").unwrap();
        assert_eq!(eth, "18446744073.709551615");

        let eth = format_units(u128::MAX, 36).unwrap();
        assert_eq!(eth, "340.282366920938463463374607431768211455");

        let eth = format_units(U256::MAX, 77).unwrap();
        assert_eq!(
            eth,
            "1.15792089237316195423570985008687907853269984665640564039457584007913129639935"
        );

        let err = format_units(U256::MAX, 78).unwrap_err();
        assert!(matches!(err, ConversionError::ParseOverflow));
    }

    #[test]
    fn test_format_units_signed() {
        let eth =
            format_units(I256::from_dec_str("-1395633240123456000").unwrap(), "ether").unwrap();
        assert_eq!(eth.parse::<f64>().unwrap(), -1.395633240123456);

        let eth =
            format_units(I256::from_dec_str("-1395633240123456789").unwrap(), "ether").unwrap();
        assert_eq!(eth, "-1.395633240123456789");

        let eth =
            format_units(I256::from_dec_str("1005633240123456789").unwrap(), "ether").unwrap();
        assert_eq!(eth, "1.005633240123456789");

        let eth = format_units(i8::MIN, 4).unwrap();
        assert_eq!(eth, "-0.0128");
        assert_eq!(eth.parse::<f64>().unwrap(), -0.0128_f64);

        let eth = format_units(i8::MAX, 4).unwrap();
        assert_eq!(eth, "0.0127");
        assert_eq!(eth.parse::<f64>().unwrap(), 0.0127);

        let eth = format_units(i16::MIN, "ether").unwrap();
        assert_eq!(eth, "-0.000000000000032768");

        // Note: This covers isize on 32 bit systems.
        let eth = format_units(i32::MIN, 18).unwrap();
        assert_eq!(eth, "-0.000000002147483648");

        // Note: This covers isize on 64 bit systems.
        let eth = format_units(i64::MIN, "gwei").unwrap();
        assert_eq!(eth, "-9223372036.854775808");

        let eth = format_units(i128::MIN, 36).unwrap();
        assert_eq!(eth, "-170.141183460469231731687303715884105728");

        let eth = format_units(I256::MIN, 76).unwrap();
        assert_eq!(
            eth,
            "-5.7896044618658097711785492504343953926634992332820282019728792003956564819968"
        );

        let err = format_units(I256::MIN, 77).unwrap_err();
        assert!(matches!(err, ConversionError::ParseOverflow));
    }

    #[test]
    fn parse_large_units() {
        let decimals = 27u32;
        let val = "10.55";

        let n: U256 = parse_units(val, decimals).unwrap().into();
        assert_eq!(n.to_string(), "10550000000000000000000000000");
    }

    #[test]
    fn test_parse_units() {
        let gwei: U256 = parse_units(1.5, 9).unwrap().into();
        assert_eq!(gwei.as_u64(), 15e8 as u64);

        let token: U256 = parse_units(1163.56926418, 8).unwrap().into();
        assert_eq!(token.as_u64(), 116356926418);

        let eth_dec_float: U256 = parse_units(1.39563324, "ether").unwrap().into();
        assert_eq!(eth_dec_float, U256::from_dec_str("1395633240000000000").unwrap());

        let eth_dec_string: U256 = parse_units("1.39563324", "ether").unwrap().into();
        assert_eq!(eth_dec_string, U256::from_dec_str("1395633240000000000").unwrap());

        let eth: U256 = parse_units(1, "ether").unwrap().into();
        assert_eq!(eth, WEI_IN_ETHER);

        let val: U256 = parse_units("2.3", "ether").unwrap().into();
        assert_eq!(val, U256::from_dec_str("2300000000000000000").unwrap());

        let n: U256 = parse_units(".2", 2).unwrap().into();
        assert_eq!(n, U256::from(20), "leading dot");

        let n: U256 = parse_units("333.21", 2).unwrap().into();
        assert_eq!(n, U256::from(33321), "trailing dot");

        let n: U256 = parse_units("98766", 16).unwrap().into();
        assert_eq!(n, U256::from_dec_str("987660000000000000000").unwrap(), "no dot");

        let n: U256 = parse_units("3_3_0", 3).unwrap().into();
        assert_eq!(n, U256::from(330000), "underscore");

        let n: U256 = parse_units("330", 0).unwrap().into();
        assert_eq!(n, U256::from(330), "zero decimals");

        let n: U256 = parse_units(".1234", 3).unwrap().into();
        assert_eq!(n, U256::from(123), "truncate too many decimals");

        assert!(parse_units("1", 80).is_err(), "overflow");
        assert!(parse_units("1", -1).is_err(), "neg units");

        let two_e30 = U256::from(2) * U256([0x4674edea40000000, 0xc9f2c9cd0, 0x0, 0x0]);
        let n: U256 = parse_units("2", 30).unwrap().into();
        assert_eq!(n, two_e30, "2e30");

        let n: U256 = parse_units(".33_319_2", 0).unwrap().into();
        assert_eq!(n, U256::zero(), "mix");

        let n: U256 = parse_units("", 3).unwrap().into();
        assert_eq!(n, U256::zero(), "empty");
    }

    #[test]
    fn test_signed_parse_units() {
        let gwei: I256 = parse_units(-1.5, 9).unwrap().into();
        assert_eq!(gwei.as_i64(), -15e8 as i64);

        let token: I256 = parse_units(-1163.56926418, 8).unwrap().into();
        assert_eq!(token.as_i64(), -116356926418);

        let eth_dec_float: I256 = parse_units(-1.39563324, "ether").unwrap().into();
        assert_eq!(eth_dec_float, I256::from_dec_str("-1395633240000000000").unwrap());

        let eth_dec_string: I256 = parse_units("-1.39563324", "ether").unwrap().into();
        assert_eq!(eth_dec_string, I256::from_dec_str("-1395633240000000000").unwrap());

        let eth: I256 = parse_units(-1, "ether").unwrap().into();
        assert_eq!(eth, I256::from_raw(WEI_IN_ETHER) * I256::minus_one());

        let val: I256 = parse_units("-2.3", "ether").unwrap().into();
        assert_eq!(val, I256::from_dec_str("-2300000000000000000").unwrap());

        let n: I256 = parse_units("-.2", 2).unwrap().into();
        assert_eq!(n, I256::from(-20), "leading dot");

        let n: I256 = parse_units("-333.21", 2).unwrap().into();
        assert_eq!(n, I256::from(-33321), "trailing dot");

        let n: I256 = parse_units("-98766", 16).unwrap().into();
        assert_eq!(n, I256::from_dec_str("-987660000000000000000").unwrap(), "no dot");

        let n: I256 = parse_units("-3_3_0", 3).unwrap().into();
        assert_eq!(n, I256::from(-330000), "underscore");

        let n: I256 = parse_units("-330", 0).unwrap().into();
        assert_eq!(n, I256::from(-330), "zero decimals");

        let n: I256 = parse_units("-.1234", 3).unwrap().into();
        assert_eq!(n, I256::from(-123), "truncate too many decimals");

        assert!(parse_units("-1", 80).is_err(), "overflow");

        let two_e30 =
            I256::from(-2) * I256::from_raw(U256([0x4674edea40000000, 0xc9f2c9cd0, 0x0, 0x0]));
        let n: I256 = parse_units("-2", 30).unwrap().into();
        assert_eq!(n, two_e30, "-2e30");

        let n: I256 = parse_units("-.33_319_2", 0).unwrap().into();
        assert_eq!(n, I256::zero(), "mix");

        let n: I256 = parse_units("-", 3).unwrap().into();
        assert_eq!(n, I256::zero(), "empty");
    }

    #[test]
    fn contract_address() {
        // http://ethereum.stackexchange.com/questions/760/how-is-the-address-of-an-ethereum-contract-computed
        let from = "00006ac7ea33f8831ea9dcc53393aaa88b25a785dbf0".parse::<Address>().unwrap();
        for (nonce, expected) in [
            "cb7060435dcdcd1fde7fe4238204365e486a1c345e03",
            "cb45936ded405892dc4f98cc5e04805d03c200c706a5",
            "cb38a7d7c76ecab54e75a0d6a06faf3b441a304a9c64",
            "cb64355702d8b1f36617d3554a5fad1187a74934d42d",
        ]
        .iter()
        .enumerate()
        {
            let address = get_contract_address(from, nonce, &NetworkType::Mainnet);
            assert_eq!(address, expected.parse::<Address>().unwrap());
        }
    }

    #[test]
    fn create2_address() {
        for (from, salt, init_code, expected) in &[
            (
                "00000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "00",
                "cb52a55032de3186cea55fdef3fdb0dbd45b18bba964",
            ),
            (
                "0000deadbeef00000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "00",
                "cb6480d15ddb300c9631bb03d106e8638a823701ecac",
            ),
            (
                "0000deadbeef00000000000000000000000000000000",
                "000000000000000000000000feed000000000000000000000000000000000000",
                "00",
                "cb875362c88b4f021806a12e94623c7a83e8c01473af",
            ),
            (
                "00000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "deadbeef",
                "cb40fce72bafe6e89f533630b9a876c1d56bfc0d7707",
            ),
            (
                "000000000000000000000000000000000000deadbeef",
                "00000000000000000000000000000000000000000000000000000000cafebabe",
                "deadbeef",
                "cb43d3c6aee116a1f8f82a0a4f2ea2a02059cafe6789",
            ),
            (
                "000000000000000000000000000000000000deadbeef",
                "00000000000000000000000000000000000000000000000000000000cafebabe",
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                "cb516997e86459f44af92ab7e927c54fe47fadcbbd6c",
            ),
            (
                "00000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "",
                "cb3680e5927ec9af1efc619ee6d4198e97c91ab72a96",
            ),
        ] {
            // get_create2_address()
            let from = from.parse::<Address>().unwrap();
            let salt = hex::decode(salt).unwrap();
            let init_code = hex::decode(init_code).unwrap();
            let expected = expected.parse::<Address>().unwrap();
            assert_eq!(expected, get_create2_address(from, salt.clone(), init_code.clone(), NetworkType::Mainnet));

            // get_create2_address_from_hash()
            let init_code_hash = sha3(init_code).to_vec();
            assert_eq!(expected, get_create2_address_from_hash(from, salt, init_code_hash, NetworkType::Mainnet))
        }
    }

    #[test]
    fn bytes32_string_parsing() {
        let text_bytes_list = vec![
            ("", hex!("0000000000000000000000000000000000000000000000000000000000000000")),
            ("A", hex!("4100000000000000000000000000000000000000000000000000000000000000")),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
                hex!("4142434445464748494a4b4c4d4e4f505152535455565758595a303132333435"),
            ),
            (
                "!@#$%^&*(),./;'[]",
                hex!("21402324255e262a28292c2e2f3b275b5d000000000000000000000000000000"),
            ),
        ];

        for (text, bytes) in text_bytes_list {
            assert_eq!(text, parse_bytes32_string(&bytes).unwrap());
        }
    }

    #[test]
    fn bytes32_string_formatting() {
        let text_bytes_list = vec![
            ("", hex!("0000000000000000000000000000000000000000000000000000000000000000")),
            ("A", hex!("4100000000000000000000000000000000000000000000000000000000000000")),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
                hex!("4142434445464748494a4b4c4d4e4f505152535455565758595a303132333435"),
            ),
            (
                "!@#$%^&*(),./;'[]",
                hex!("21402324255e262a28292c2e2f3b275b5d000000000000000000000000000000"),
            ),
        ];

        for (text, bytes) in text_bytes_list {
            assert_eq!(bytes, format_bytes32_string(text).unwrap());
        }
    }

    #[test]
    fn bytes32_string_formatting_too_long() {
        assert!(matches!(
            format_bytes32_string("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456").unwrap_err(),
            ConversionError::TextTooLong
        ));
    }

    #[test]
    fn test_eip1559_default_estimator() {
        // If the base fee is below the triggering base fee, we should get the default priority fee
        // with the base fee surged.
        let base_fee_per_gas = U256::from(EIP1559_FEE_ESTIMATION_PRIORITY_FEE_TRIGGER) - 1;
        let rewards: Vec<Vec<U256>> = vec![vec![]];
        let (base_fee, priority_fee) = eip1559_default_estimator(base_fee_per_gas, rewards);
        assert_eq!(priority_fee, U256::from(EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE));
        assert_eq!(base_fee, base_fee_surged(base_fee_per_gas));

        // If the base fee is above the triggering base fee, we calculate the priority fee using
        // the fee history (rewards).
        let base_fee_per_gas = U256::from(EIP1559_FEE_ESTIMATION_PRIORITY_FEE_TRIGGER) + 1;
        let rewards: Vec<Vec<U256>> = vec![
            vec![100_000_000_000u64.into()],
            vec![105_000_000_000u64.into()],
            vec![102_000_000_000u64.into()],
        ]; // say, last 3 blocks
        let (base_fee, priority_fee) = eip1559_default_estimator(base_fee_per_gas, rewards.clone());
        assert_eq!(base_fee, base_fee_surged(base_fee_per_gas));
        assert_eq!(priority_fee, estimate_priority_fee(rewards.clone()));

        // The median should be taken because none of the changes are big enough to ignore values.
        assert_eq!(estimate_priority_fee(rewards), 102_000_000_000u64.into());

        // Ensure fee estimation doesn't panic when overflowing a u32. This had been a divide by
        // zero.
        let overflow = U256::from(u32::MAX) + 1;
        let rewards_overflow: Vec<Vec<U256>> = vec![vec![overflow], vec![overflow]];
        assert_eq!(estimate_priority_fee(rewards_overflow), overflow);
    }

    #[test]
    fn int_or_hex_combinations() {
        // make sure we can deserialize all combinations of int and hex
        // including large numbers that would overflow u64
        //
        // format: (string, expected value)
        let cases = vec![
            // hex strings
            ("\"0x0\"", U256::from(0)),
            ("\"0x1\"", U256::from(1)),
            ("\"0x10\"", U256::from(16)),
            ("\"0x100000000000000000000000000000000000000000000000000\"", U256::from_dec_str("1606938044258990275541962092341162602522202993782792835301376").unwrap()),
            // small num, both num and str form
            ("10", U256::from(10)),
            ("\"10\"", U256::from(10)),
            // max u256, in both num and str form
            ("115792089237316195423570985008687907853269984665640564039457584007913129639935", U256::from_dec_str("115792089237316195423570985008687907853269984665640564039457584007913129639935").unwrap()),
            ("\"115792089237316195423570985008687907853269984665640564039457584007913129639935\"", U256::from_dec_str("115792089237316195423570985008687907853269984665640564039457584007913129639935").unwrap())
        ];

        #[derive(Deserialize)]
        struct TestUint(#[serde(deserialize_with = "from_int_or_hex")] U256);

        for (string, expected) in cases {
            let test: TestUint = serde_json::from_str(string).unwrap();
            assert_eq!(test.0, expected);
        }
    }
}
