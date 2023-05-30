use super::Source;
use crate::util;
use eyre::{Context, Result};
use url::Url;


impl Source {
    #[inline]
    pub(super) fn parse_online(source: &str) -> Result<Self> {
        if let Ok(url) = Url::parse(source) {
            match url.scheme() {
                // file://<path>
                "file" => Self::local(source),

                // npm:<npm package>
                "npm" => Ok(Self::npm(url.path())),

                // any http url
                "http" | "https" => Ok(Self::Http(url)),

                // custom scheme: <network>:<address>
                 _ =>  Self::local(source)
                    .wrap_err("Invalid path or URL"),
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
