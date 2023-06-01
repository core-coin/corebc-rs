use crate::{BlockindexError, Result};
use semver::Version;
use serde_json::Value;

static YLEM_BIN_LIST_URL: &str =
    "https://raw.githubusercontent.com/core-coin/ylem-bins/main/list.json";

/// Options for querying Ylem versions
#[derive(Clone, Debug)]
pub enum YlemLookupQuery {
    Given(Version),
    Latest(),
    All(),
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
        .unwrap()
        .iter()
        .map(|x| x["version"].as_str().unwrap().parse::<Version>().unwrap())
        .collect();

    match query {
        YlemLookupQuery::Given(requested) => {
            let version = versions
                .iter()
                .find(|v| v == &requested)
                .ok_or_else(|| BlockindexError::MissingYlemVersion(requested.to_string()))?;
            Ok(YlemLookupResult::Version(version.to_owned()))
        }
        YlemLookupQuery::Latest() => {
            Ok(YlemLookupResult::Version(versions.iter().max().unwrap().to_owned()))
        }
        YlemLookupQuery::All() => Ok(YlemLookupResult::All(versions)),
    }
}
