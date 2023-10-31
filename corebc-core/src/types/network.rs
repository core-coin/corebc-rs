use super::{U128, U256, U512, U64};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{convert::TryFrom, fmt, str::FromStr, time::Duration};
use strum::{EnumCount, EnumIter, EnumVariantNames};

#[derive(Debug)]
pub struct ParseNetworkError {
    pub number: u64,
}

impl std::fmt::Display for ParseNetworkError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Cannot parse network with id {}", self.number)
    }
}

// When adding a new network:
//   1. add new variant to the Network enum;
//   2. add extra information in the last `impl` block (explorer URLs, block time) when applicable;
//     - Add a test at the bottom of the file

/// An Ethereum EIP-155 Network.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    EnumVariantNames, // Network::VARIANTS
    EnumIter,         // Network::iter
    EnumCount,        /* Network::COUNT */
)]
#[repr(u64)]
pub enum Network {
    Mainnet = 1,
    Devin = 3,
    Private(u64),
}

// === impl Network ===

impl Default for Network {
    fn default() -> Self {
        Self::Mainnet
    }
}

macro_rules! impl_into_numeric {
    ($($ty:ty)+) => {$(
        impl From<Network> for $ty {
            fn from(network: Network) -> Self {
                match network {
                    Network::Mainnet => (1 as u64).into(),
                    Network::Devin =>(3 as u64).into(),
                    Network::Private(n) => (n as u64).into(),
                }
            }
        }
    )+};
}

macro_rules! impl_from_numeric {
    ($($native:ty)+ ; $($primitive:ty)*) => {
        $(
            impl From<$native> for Network {
                fn from(value: $native) -> Self {
                    match value as u64 {
                        1 => Network::Mainnet,
                        3 => Network::Devin,
                        n => Network::Private(n),
                    }
                }
            }
        )+

        $(
            impl From<$primitive> for Network {
                fn from(value: $primitive) -> Self {
                    if value.bits() > 64 {
                        panic!("{:?}",  ParseNetworkError { number: value.low_u64() });
                    }
                    match value.low_u64() {
                        1 => Network::Mainnet,
                        3 => Network::Devin,
                        n => Network::Private(n),
                    }
                }
            }
        )*
    };
}

// macro_rules! impl_try_from_numeric {
//     ($($native:ty)+ ; $($primitive:ty)*) => {
//         $(
//             impl TryFrom<$native> for Network {
//                 type Error = ParseNetworkError;

//                 fn try_from(value: $native) -> Result<Self, Self::Error> {
//                     match value as u64 {
//                         1 => Ok(Network::Mainnet),
//                         3 => Ok(Network::Devin),
//                         n => Ok(Network::Private(n)),
//                     }
//                 }
//             }
//         )+

//         $(
//             impl TryFrom<$primitive> for Network {
//                 type Error = ParseNetworkError;

//                 fn try_from(value: $primitive) -> Result<Self, Self::Error> {
//                     if value.bits() > 64 {
//                         return Err(ParseNetworkError { number: value.low_u64() })
//                     }
//                     match value.low_u64() {
//                         1 => Ok(Network::Mainnet),
//                         3 => Ok(Network::Devin),
//                         n => Ok(Network::Private(n)),
//                     }
//                 }
//             }
//         )*
//     };
// }

impl From<Network> for u64 {
    fn from(network: Network) -> Self {
        match network {
            Network::Mainnet => 1,
            Network::Devin => 3,
            Network::Private(n) => n,
        }
    }
}

impl_into_numeric!(u128 U64 U128 U256 U512);

impl TryFrom<U64> for Network {
    type Error = ParseNetworkError;

    fn try_from(value: U64) -> Result<Self, Self::Error> {
        match value.low_u64() {
            1 => Ok(Network::Mainnet),
            3 => Ok(Network::Devin),
            n => Ok(Network::Private(n)),
        }
    }
}

impl TryFrom<&str> for Network {
    type Error = ParseNetworkError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Network::from(value.to_string()))
    }
}

impl_from_numeric!(u8 u16 u32 u64 usize; U128 U256 U512);

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Devin => write!(f, "devin"),
            Network::Private(id) => write!(f, "private-{}", id),
        }
    }
}

impl Serialize for Network {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(format!("{}", self).as_str())
    }
}

impl<'de> Deserialize<'de> for Network {
    fn deserialize<D>(deserializer: D) -> Result<Network, D::Error>
    where
        D: Deserializer<'de>,
    {
        println!("33: {}", 3); 

        let s = String::deserialize(deserializer)?;

        println!("s22: {}", s); 

        Ok(Network::from(s))
    }
}

impl From<String> for Network {
    fn from(s: String) -> Network {
        println!("s: {}", s); 
        match s.as_str() {
            "mainnet" | "1" => Network::Mainnet,
            "devin" | "3" => Network::Devin,
            unknown => {
                if let ["private", id_str] = unknown.split('-').collect::<Vec<_>>().as_slice() {
                    if let Ok(id) = id_str.parse::<u64>() {
                        return Network::Private(id)
                    }
                }
                if let Ok(id) = unknown.parse::<u64>() {
                    return Network::Private(id)
                }
                panic!("Unknown network: {}", unknown);
            }
        }
    }
}

impl FromStr for Network {
    type Err = ParseNetworkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Network::from(s.to_string()))
    }
}

// NB: all utility functions *should* be explicitly exhaustive (not use `_` matcher) so we don't
//     forget to update them when adding a new `Network` variant.
#[allow(clippy::match_like_matches_macro)]
impl Network {
    /// Returns the network's average blocktime, if applicable.
    ///
    /// It can be beneficial to know the average blocktime to adjust the polling of an HTTP provider
    /// for example.
    ///
    /// **Note:** this is not an accurate average, but is rather a sensible default derived from
    /// blocktime charts such as [Etherscan's](https://etherscan.com/chart/blocktime)
    /// or [Polygonscan's](https://polygonscan.com/chart/blocktime).
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    /// use std::time::Duration;
    ///
    /// assert_eq!(
    ///     Network::Mainnet.average_blocktime_hint(),
    ///     Some(Duration::from_millis(7_000)),
    /// );
    /// ```
    pub const fn average_blocktime_hint(&self) -> Option<Duration> {
        use Network::*;

        let ms = match self {
            Mainnet | Devin | Private(_) => 7_000,
        };

        Some(Duration::from_millis(ms))
    }

    /// Returns whether the network implements EIP-1559 (with the type 2 EIP-2718 transaction type).
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    ///
    /// assert!(!Network::Mainnet.is_legacy());
    /// assert!(!Network::Devin.is_legacy());
    /// ```
    pub const fn is_legacy(&self) -> bool {
        use Network::*;

        match self {
            // Known EIP-1559 networks
            Mainnet | Devin | Private(_) => false,
        }
    }

    /// Returns the network's blockchain explorer and its API URLs.
    ///
    /// Returns `(API_URL, BASE_URL)`
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    ///
    /// assert_eq!(
    ///     Network::Mainnet.blockindex_urls(),
    ///     Some(("https://blockindex.net/api/v2", "https://blockindex.net"))
    /// );
    /// assert_eq!(
    ///     Network::Devin.blockindex_urls(),
    ///     Some(("https://devin.blockindex.net/api/v2", "https://devin.blockindex.net"))
    /// );
    /// ```
    pub const fn blockindex_urls(&self) -> Option<(&'static str, &'static str)> {
        use Network::*;
        //CORETODO change to core coin blockchain explorers
        let urls = match self {
            Mainnet => ("https://blockindex.net/api/v2", "https://blockindex.net"),
            Devin => ("https://devin.blockindex.net/api/v2", "https://devin.blockindex.net"),
            Private(_) => ("", ""),
        };

        Some(urls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn default() {
        assert_eq!(serde_json::to_string(&Network::default()).unwrap(), "\"mainnet\"");
    }

    #[test]
    fn enum_iter() {
        assert_eq!(Network::COUNT, Network::iter().size_hint().0);
    }

    #[test]
    fn roundtrip_string() {
        for network in Network::iter() {
            let network_string = network.to_string();
            assert_eq!(network_string, format!("{network}"));
            assert_eq!(serde_json::to_string(&network).unwrap(), format!("\"{network_string}\""));

            assert_eq!(network_string.parse::<Network>().unwrap(), network);
        }
    }

    #[test]
    fn roundtrip_serde() {
        for network in Network::iter() {
            let network_string = serde_json::to_string(&network).unwrap();
            assert_eq!(serde_json::from_str::<'_, Network>(&network_string).unwrap(), network);
        }
    }

    #[test]
    fn serde_to_string_match() {
        for network in Network::iter() {
            let network_serde = serde_json::to_string(&network).unwrap();
            let network_string = format!("\"{network}\"");
            assert_eq!(network_serde, network_string);
        }
    }

    #[test]
    fn u64_to_network() {
        let mainnet = Network::try_from(1u64).expect("cannot parse mainnet network_id");
        assert_eq!(mainnet, Network::Mainnet);

        let devin = Network::try_from(3u64).expect("cannot parse devin network_id");
        assert_eq!(devin, Network::Devin);

        let private = Network::try_from(6u64).expect("cannot parse private network_id 6");
        assert_eq!(private, Network::Private(6));

        for network_id in 1..10u64 {
            let network = Network::try_from(network_id).expect("cannot parse u64 as network_id");
            assert_eq!(u64::from(network), network_id);
        }
    }

    #[test]
    fn parse_networks_as_number_strings() {
        let mut private = Network::try_from("6").expect("cannot parse private network_id 6");
        assert_eq!(private, Network::Private(6));

        private = Network::try_from("private-4").expect("cannot parse private network_id 4");
        assert_eq!(private, Network::Private(4));

        let mainnet = Network::try_from("1").expect("cannot parse mainnet network_id");
        assert_eq!(mainnet, Network::Mainnet);

        let devin = Network::try_from("3").expect("cannot parse devin network_id");
        assert_eq!(devin, Network::Devin);
    }
}
