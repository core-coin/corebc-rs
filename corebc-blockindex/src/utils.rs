use crate::{BlockindexError, Result};
use semver::Version;
use serde_json::Value;

static YLEM_BIN_LIST_URL: &str =
    "https://raw.githubusercontent.com/core-coin/ylem-bins/main/list.json";

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
    let response: Value = reqwest::get(YLEM_BIN_LIST_URL).await?.json().await?;

    let versions: Vec<Version> = response["builds"]
        .as_array()
        .ok_or_else(|| BlockindexError::Builder("txids".to_string()))?
        .iter()
        .map(|x| {
            x["version"]
                .as_str()
                .ok_or_else(|| BlockindexError::Builder("version".to_string()))?
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
