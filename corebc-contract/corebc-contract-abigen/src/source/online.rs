use super::Source;
use crate::util;
use corebc_blockindex::Client;
use corebc_core::types::{Address, Network};
use eyre::{Context, Result};
use std::{fmt, str::FromStr};
use url::Url;

/// An [etherscan](https://etherscan.io)-like blockchain explorer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
//CORETODO remove unused variants
pub enum Explorer {
    /// <https://etherscan.io>
    #[default]
    Etherscan,
    /// <https://bscscan.com>
    Bscscan,
    /// <https://polygonscan.com>
    Polygonscan,
    /// <https://snowtrace.io>
    Snowtrace,
}

impl FromStr for Explorer {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "etherscan" | "etherscan.io" => Ok(Self::Etherscan),
            "bscscan" | "bscscan.com" => Ok(Self::Bscscan),
            "polygonscan" | "polygonscan.com" => Ok(Self::Polygonscan),
            "snowtrace" | "snowtrace.io" => Ok(Self::Snowtrace),
            _ => Err(eyre::eyre!("Invalid or unsupported blockchain explorer: {s}")),
        }
    }
}

impl fmt::Display for Explorer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Explorer {
    /// Returns the network's Explorer, if it is known.
    pub fn from_network(network: Network) -> Result<Self> {
        match network {
            Network::Mainnet => Ok(Self::Etherscan),
            _ => Err(eyre::eyre!("Provided network has no known blockchain explorer")),
        }
    }

    /// Returns the Explorer's network. If it has multiple, the main one is returned.
    pub const fn network(&self) -> Network {
        match self {
            Self::Etherscan => Network::Mainnet,
            Self::Bscscan => Network::Devin,
            Self::Polygonscan => Network::Devin,
            Self::Snowtrace => Network::Devin,
        }
    }

    /// Creates an `corebc-blockindex` client using this Explorer's settings.
    pub fn client(self) -> Result<Client> {
        let network = self.network();
        let client = Client::new(network)?;
        Ok(client)
    }
}

impl Source {
    #[inline]
    pub(super) fn parse_online(source: &str) -> Result<Self> {
        if let Ok(url) = Url::parse(source) {
            match url.scheme() {
                // file://<path>
                "file" => Self::local(source),

                // npm:<npm package>
                "npm" => Ok(Self::npm(url.path())),

                // custom scheme: <network>:<address>
                // fallback: local fs path
                _ => Self::local(source).wrap_err("Invalid path or URL"),
            }
        } else {
            // not a valid URL so fallback to path
            Self::local(source)
        }
    }

    /// Creates an HTTP source from a URL.
    pub fn http(url: impl AsRef<str>) -> Result<Self> {
        Ok(Self::Http(Url::parse(url.as_ref())?))
    }

    /// Creates an Etherscan source from an address string.
    pub fn npm(package_path: impl Into<String>) -> Self {
        Self::Npm(package_path.into())
    }

    #[inline]
    pub(super) fn get_online(&self) -> Result<String> {
        match self {
            Self::Http(url) => {
                util::http_get(url.clone()).wrap_err("Failed to retrieve ABI from URL")
            }
            Self::Npm(package) => {
                // TODO: const?
                let unpkg = Url::parse("https://unpkg.io/").unwrap();
                let url = unpkg.join(package).wrap_err("Invalid NPM package")?;
                util::http_get(url).wrap_err("Failed to retrieve ABI from NPM package")
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_online_source() {
        assert_eq!(
            Source::parse("https://my.domain.eth/path/to/Contract.json").unwrap(),
            Source::http("https://my.domain.eth/path/to/Contract.json").unwrap()
        );

        assert_eq!(
            Source::parse("npm:@openzeppelin/contracts@2.5.0/build/contracts/IERC20.json").unwrap(),
            Source::npm("@openzeppelin/contracts@2.5.0/build/contracts/IERC20.json")
        );
    }

    #[test]
    fn get_mainnet_contract() {
        // Skip if ETHERSCAN_API_KEY is not set
        if std::env::var("ETHERSCAN_API_KEY").is_err() {
            return
        }

        let source = Source::parse("mainnet:0x6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let abi = source.get().unwrap();
        assert!(!abi.is_empty());
    }
}
