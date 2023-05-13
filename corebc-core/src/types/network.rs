use super::{U128, U256, U512, U64};
use serde::{Deserialize, Serialize, Serializer};
use std::{
    convert::{TryFrom, TryInto},
    fmt,
    time::Duration,
};
use strum::{AsRefStr, EnumCount, EnumIter, EnumString, EnumVariantNames};

// compatibility re-export
#[doc(hidden)]
pub use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
#[doc(hidden)]
pub type ParseNetworkError = TryFromPrimitiveError<Network>;

// When adding a new network:
//   1. add new variant to the Network enum;
//   2. add extra information in the last `impl` block (explorer URLs, block time) when applicable;
//   3. (optional) add aliases:
//     - Strum (in kebab-case): `#[strum(to_string = "<main>", serialize = "<aliasX>", ...)]`
//      `to_string = "<main>"` must be present and will be used in `Display`, `Serialize`
//      and `FromStr`, while `serialize = "<aliasX>"` will be appended to `FromStr`.
//      More info: <https://docs.rs/strum/latest/strum/additional_attributes/index.html#attributes-on-variants>
//     - Serde (in snake_case): `#[serde(alias = "<aliasX>", ...)]`
//      Aliases are appended to the `Deserialize` implementation.
//      More info: <https://serde.rs/variant-attrs.html>
//     - Add a test at the bottom of the file

// We don't derive Serialize because it is manually implemented using AsRef<str> and it would
// break a lot of things since Serialize is `kebab-case` vs Deserialize `snake_case`.
// This means that the Network type is not "round-trippable", because the Serialize and Deserialize
// implementations do not use the same case style.

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
    AsRefStr,         // AsRef<str>, fmt::Display and serde::Serialize
    EnumVariantNames, // Network::VARIANTS
    EnumString,       // FromStr, TryFrom<&str>
    EnumIter,         // Network::iter
    EnumCount,        // Network::COUNT
    TryFromPrimitive, // TryFrom<u64>
    Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "kebab-case")]
#[repr(u64)]
pub enum Network {
    #[strum(to_string = "mainnet", serialize = "xcblive")]
    #[serde(alias = "xcblive")]
    Mainnet = 1,
    #[strum(to_string = "devin", serialize = "xablive")]
    #[serde(alias = "xablive")]
    Devin = 3,
}

// === impl Network ===

// This must be implemented manually so we avoid a conflict with `TryFromPrimitive` where it treats
// the `#[default]` attribute as its own `#[num_enum(default)]`
impl Default for Network {
    fn default() -> Self {
        Self::Mainnet
    }
}

macro_rules! impl_into_numeric {
    ($($ty:ty)+) => {$(
        impl From<Network> for $ty {
            fn from(network: Network) -> Self {
                u64::from(network).into()
            }
        }
    )+};
}

macro_rules! impl_try_from_numeric {
    ($($native:ty)+ ; $($primitive:ty)*) => {
        $(
            impl TryFrom<$native> for Network {
                type Error = ParseNetworkError;

                fn try_from(value: $native) -> Result<Self, Self::Error> {
                    (value as u64).try_into()
                }
            }
        )+

        $(
            impl TryFrom<$primitive> for Network {
                type Error = ParseNetworkError;

                fn try_from(value: $primitive) -> Result<Self, Self::Error> {
                    if value.bits() > 64 {
                        // `TryFromPrimitiveError` only has a `number` field which has the same type
                        // as the `#[repr(_)]` attribute on the enum.
                        return Err(ParseNetworkError { number: value.low_u64() })
                    }
                    value.low_u64().try_into()
                }
            }
        )*
    };
}

impl From<Network> for u64 {
    fn from(network: Network) -> Self {
        network as u64
    }
}

impl_into_numeric!(u128 U64 U128 U256 U512);

impl TryFrom<U64> for Network {
    type Error = ParseNetworkError;

    fn try_from(value: U64) -> Result<Self, Self::Error> {
        value.low_u64().try_into()
    }
}

impl_try_from_numeric!(u8 u16 u32 usize; U128 U256 U512);

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.as_ref())
    }
}

impl Serialize for Network {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(self.as_ref())
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
            Mainnet | Devin => 7_000,
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
            Mainnet | Devin => false,
        }
    }

    /// Returns the network's blockchain explorer and its API (Etherscan and Etherscan-like) URLs.
    ///
    /// Returns `(API_URL, BASE_URL)`
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    ///
    /// assert_eq!(
    ///     Network::Mainnet.etherscan_urls(),
    ///     Some(("https://api.etherscan.io/api", "https://etherscan.io"))
    /// );
    /// assert_eq!(
    ///     Network::Devin.etherscan_urls(),
    ///     Some(("https://api-goerli.etherscan.io/api", "https://goerli.etherscan.io"))
    /// );
    /// ```
    pub const fn etherscan_urls(&self) -> Option<(&'static str, &'static str)> {
        use Network::*;

        let urls = match self {
            Mainnet => ("https://api.etherscan.io/api", "https://etherscan.io"),
            Devin => ("https://api-goerli.etherscan.io/api", "https://goerli.etherscan.io"),
        };

        Some(urls)
    }

    /// Returns the network's blockchain explorer's API key environment variable's default name.
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    ///
    /// assert_eq!(Network::Mainnet.etherscan_api_key_name(), Some("ETHERSCAN_API_KEY"));
    /// ```
    pub const fn etherscan_api_key_name(&self) -> Option<&'static str> {
        use Network::*;

        let api_key_name = match self {
            Mainnet | Devin => "ETHERSCAN_API_KEY",
        };

        Some(api_key_name)
    }

    /// Returns the network's blockchain explorer's API key, from the environment variable with the
    /// name specified in [`etherscan_api_key_name`](Network::etherscan_api_key_name).
    ///
    /// # Examples
    ///
    /// ```
    /// use corebc_core::types::Network;
    ///
    /// let network = Network::Mainnet;
    /// std::env::set_var(network.etherscan_api_key_name().unwrap(), "KEY");
    /// assert_eq!(network.etherscan_api_key().as_deref(), Some("KEY"));
    /// ```
    pub fn etherscan_api_key(&self) -> Option<String> {
        self.etherscan_api_key_name().and_then(|name| std::env::var(name).ok())
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
            assert_eq!(network_string.as_str(), network.as_ref());
            assert_eq!(serde_json::to_string(&network).unwrap(), format!("\"{network_string}\""));

            assert_eq!(network_string.parse::<Network>().unwrap(), network);
        }
    }

    #[test]
    fn roundtrip_serde() {
        for network in Network::iter() {
            let network_string = serde_json::to_string(&network).unwrap();
            let network_string = network_string.replace('-', "_");
            assert_eq!(serde_json::from_str::<'_, Network>(&network_string).unwrap(), network);
        }
    }

    #[test]
    // CORETODO: Needs anvil
    fn aliases() {
        use Network::*;

        // kebab-case
        const ALIASES: &[(Network, &[&str])] = &[(Mainnet, &["xcblive"]), (Devin, &["xablive"])];

        for &(network, aliases) in ALIASES {
            for &alias in aliases {
                assert_eq!(alias.parse::<Network>().unwrap(), network);
                let s = alias.to_string().replace('-', "_");
                assert_eq!(serde_json::from_str::<Network>(&format!("\"{s}\"")).unwrap(), network);
            }
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
}
