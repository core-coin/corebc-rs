use crate::*;
use corebc_blockindex::utils::{lookup_compiler_version, YlemLookupQuery, YlemLookupResult};
use semver::Version;

#[tokio::test]
async fn can_lookup_compiler_version_latest() {
    let version: YlemLookupResult =
        lookup_compiler_version(&YlemLookupQuery::Latest).await.unwrap();
    if let YlemLookupResult::Version(returned) = version {
        assert_eq!(returned, Version::new(0, 0, 14));
    } else {
        panic!("Expected YlemLookupResult::Version, got {:?}", version);
    }
}

#[tokio::test]
async fn can_lookup_compiler_version_exact() {
    let requested = Version::new(0, 0, 14);
    let version: YlemLookupResult =
        lookup_compiler_version(&YlemLookupQuery::Given(requested.clone())).await.unwrap();
    if let YlemLookupResult::Version(returned) = version {
        assert_eq!(returned, requested);
    } else {
        panic!("Expected YlemLookupResult::Version, got {:?}", version);
    }
}

#[tokio::test]
async fn can_lookup_compiler_version_all() {
    let version: YlemLookupResult = lookup_compiler_version(&YlemLookupQuery::All).await.unwrap();
    if let YlemLookupResult::All(versions) = version {
        assert_eq!(versions, vec!(Version::new(0, 0, 14)));
    } else {
        panic!("Expected YlemLookupResult::All, got {:?}", version);
    }
}

#[tokio::test]
async fn can_lookup_compiler_version_exact_wrong() {
    let requested = Version::new(0, 0, 999);
    let version = lookup_compiler_version(&YlemLookupQuery::Given(requested.clone())).await;
    assert_eq!(
        version.unwrap_err().to_string(),
        BlockindexError::MissingYlemVersion("0.0.999".to_string()).to_string()
    );
}
