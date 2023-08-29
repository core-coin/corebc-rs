use crate::{BlockindexError, Result, contract::SourceCodeMetadata};
use corebc_core::types::Address;
use serde::{Deserialize, Deserializer};
use semver::Version;

static YLEM_BIN_LIST_URL: &str =
    "https://raw.githubusercontent.com/core-coin/ylem-bins/main/list.txt";

/// Options for querying Ylem versions
#[derive(Clone, Debug)]
pub enum YlemLookupQuery {
    Given(Version),
    Latest,
    All,
}

/// Result of a Ylem version lookup
#[derive(Clone, Debug)]
pub enum YlemLookupResult {
    Version(Version),
    All(Vec<Version>),
}

/// Returns the requested Ylem version(s).
pub async fn lookup_compiler_version(query: &YlemLookupQuery) -> Result<YlemLookupResult> {
    let response = reqwest::get(YLEM_BIN_LIST_URL).await?.text().await?;

    let versions: Vec<Version> = response
        .lines()
        .map(|l| {
            l.to_string()
                .trim_start_matches('v')
                .parse::<Version>()
                .map_err(|_| BlockindexError::Builder("version".to_string()))
        })
        .collect::<Result<Vec<Version>>>()?;
    // let versions = versions?;
    match query {
        YlemLookupQuery::Given(requested) => {
            let version = versions
                .iter()
                .find(|v| v == &requested)
                .ok_or_else(|| BlockindexError::MissingYlemVersion(requested.to_string()))?;
            Ok(YlemLookupResult::Version(version.to_owned()))
        }
        YlemLookupQuery::Latest => {
            let version = versions
                .iter()
                .max()
                .ok_or_else(|| BlockindexError::MissingYlemVersion("latest".to_string()))?;
            Ok(YlemLookupResult::Version(version.to_owned()))
        }
        YlemLookupQuery::All => Ok(YlemLookupResult::All(versions)),
    }
}

/// Return None if empty, otherwise parse as [Address].
pub fn deserialize_address_opt<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Address>, D::Error> {
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(s) => match s.is_empty() {
            true => Ok(None),
            _ => Ok(Some(s.parse().map_err(serde::de::Error::custom)?)),
        },
    }
}

/// Deserializes as JSON either:
///
/// - Object: `{ "SourceCode": { language: "Solidity", .. }, ..}`
/// - Stringified JSON object:
///     - `{ "SourceCode": "{{\r\n  \"language\": \"Solidity\", ..}}", ..}`
///     - `{ "SourceCode": "{ \"file.sol\": \"...\" }", ... }`
/// - Normal source code string: `{ "SourceCode": "// SPDX-License-Identifier: ...", .. }`
pub fn deserialize_source_code<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<SourceCodeMetadata, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SourceCode {
        String(String), // this must come first
        Obj(SourceCodeMetadata),
    }
    let s = SourceCode::deserialize(deserializer)?;
    match s {
        SourceCode::String(s) => {
            if s.starts_with('{') && s.ends_with('}') {
                let mut s = s.as_str();
                // skip double braces
                if s.starts_with("{{") && s.ends_with("}}") {
                    s = &s[1..s.len() - 1];
                }
                serde_json::from_str(s).map_err(serde::de::Error::custom)
            } else {
                Ok(SourceCodeMetadata::SourceCode(s))
            }
        }
        SourceCode::Obj(obj) => Ok(obj),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_deserialize_address_opt() {
        #[derive(serde::Serialize, Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_address_opt")]
            address: Option<Address>,
        }

        // https://api.etherscan.io/api?module=contract&action=getsourcecode&address=0xBB9bc244D798123fDe783fCc1C72d3Bb8C189413
        let json = r#"{"address":""}"#;
        let de: Test = serde_json::from_str(json).unwrap();
        assert_eq!(de.address, None);

        // Round-trip the above
        let json = serde_json::to_string(&de).unwrap();
        let de: Test = serde_json::from_str(&json).unwrap();
        assert_eq!(de.address, None);

        // https://api.etherscan.io/api?module=contract&action=getsourcecode&address=0xDef1C0ded9bec7F1a1670819833240f027b25EfF
        let json = r#"{"address":"ab654efcf28707488885abbe9d1fc80cbe6d6036f250"}"#;
        let de: Test = serde_json::from_str(json).unwrap();
        let expected = "ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse().unwrap();
        assert_eq!(de.address, Some(expected));
    }

    #[test]
    fn can_deserialize_source_code() {
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_source_code")]
            source_code: SourceCodeMetadata,
        }

        let src = "source code text";

        // Normal JSON
        let json = r#"{
            "source_code": { "language": "Solidity", "sources": { "Contract": { "content": "source code text" } } }
        }"#;
        let de: Test = serde_json::from_str(json).unwrap();
        assert_eq!(de.source_code.sources().len(), 1);
        assert_eq!(de.source_code.sources().get("Contract").unwrap().content, src);
        #[cfg(feature = "corebc-ylem")]
        assert!(de.source_code.settings().unwrap().is_none());

        // Stringified JSON
        let json = r#"{
            "source_code": "{{ \"language\": \"Solidity\", \"sources\": { \"Contract\": { \"content\": \"source code text\" } } }}"
        }"#;
        let de: Test = serde_json::from_str(json).unwrap();
        assert_eq!(de.source_code.sources().len(), 1);
        assert_eq!(de.source_code.sources().get("Contract").unwrap().content, src);
        #[cfg(feature = "corebc-ylem")]
        assert!(de.source_code.settings().unwrap().is_none());

        let json = r#"{"source_code": "source code text"}"#;
        let de: Test = serde_json::from_str(json).unwrap();
        assert_eq!(de.source_code.source_code(), src);
    }
}