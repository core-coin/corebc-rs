use crate::{
    source_tree::{SourceTree, SourceTreeEntry},
    utils::{deserialize_address_opt, deserialize_source_code},
    BlockindexError, Client, Result,
};
use corebc_core::{
    abi::{Abi, Address, RawAbi},
    types::{serde_helpers::deserialize_stringified_u64, Bytes},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[cfg(feature = "corebc-ylem")]
use corebc_ylem::{artifacts::Settings, CvmVersion, Project, ProjectBuilder, YlemConfig};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceCodeEntry {
    pub content: String,
}

impl<T: Into<String>> From<T> for SourceCodeEntry {
    fn from(s: T) -> Self {
        Self { content: s.into() }
    }
}

/// The contract metadata's SourceCode field.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceCodeMetadata {
    /// Contains just mapped source code.
    // NOTE: this must come before `Metadata`
    Sources(HashMap<String, SourceCodeEntry>),
    /// Contains metadata and path mapped source code.
    Metadata {
        /// Source path => source code
        #[serde(default)]
        sources: HashMap<String, SourceCodeEntry>,
        /// Compiler settings, None if the language is not Solidity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<serde_json::Value>,
    },
    /// Contains only the source code.
    SourceCode(String),
}

impl SourceCodeMetadata {
    pub fn source_code(&self) -> String {
        match self {
            Self::Metadata { sources, .. } => {
                sources.values().map(|s| s.content.clone()).collect::<Vec<_>>().join("\n")
            }
            Self::Sources(sources) => {
                sources.values().map(|s| s.content.clone()).collect::<Vec<_>>().join("\n")
            }
            Self::SourceCode(s) => s.clone(),
        }
    }

    pub fn sources(&self) -> HashMap<String, SourceCodeEntry> {
        match self {
            Self::Metadata { sources, .. } => sources.clone(),
            Self::Sources(sources) => sources.clone(),
            Self::SourceCode(s) => HashMap::from([("Contract".into(), s.into())]),
        }
    }

    #[cfg(feature = "corebc-ylem")]
    pub fn settings(&self) -> Result<Option<Settings>> {
        match self {
            Self::Metadata { settings, .. } => match settings {
                Some(value) => {
                    if value.is_null() {
                        Ok(None)
                    } else {
                        Ok(Some(serde_json::from_value(value.to_owned())?))
                    }
                }
                None => Ok(None),
            },
            Self::Sources(_) => Ok(None),
            Self::SourceCode(_) => Ok(None),
        }
    }

    #[cfg(not(feature = "corebc-ylem"))]
    pub fn settings(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Metadata { settings, .. } => settings.as_ref(),
            Self::Sources(_) => None,
            Self::SourceCode(_) => None,
        }
    }
}

/// Etherscan contract metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Metadata {
    /// Includes metadata for compiler settings and language.
    #[serde(deserialize_with = "deserialize_source_code")]
    pub source_code: SourceCodeMetadata,

    /// The ABI of the contract.
    #[serde(rename = "ABI")]
    pub abi: String,

    /// The name of the contract.
    pub contract_name: String,

    /// The version that this contract was compiled with.
    pub compiler_version: String,

    /// Whether the optimizer was used. This value should only be 0 or 1.
    #[serde(deserialize_with = "deserialize_stringified_u64")]
    pub optimization_used: u64,

    /// The number of optimizations performed.
    #[serde(deserialize_with = "deserialize_stringified_u64")]
    pub runs: u64,

    /// The constructor arguments the contract was deployed with.
    pub constructor_arguments: Bytes,

    /// The version of the YVM the contract was deployed in. Can be either a variant of YvmVersion
    /// or "Default" which indicates the compiler's default.
    #[serde(rename = "YVMVersion")]
    pub yvm_version: String,

    // ?
    pub library: String,

    /// The license of the contract.
    pub license_type: String,

    /// Whether this contract is a proxy. This value should only be 0 or 1.
    #[serde(deserialize_with = "deserialize_stringified_u64")]
    pub proxy: u64,

    /// If this contract is a proxy, the address of its implementation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_address_opt"
    )]
    pub implementation: Option<Address>,

    /// The swarm source of the contract.
    pub swarm_source: String,
}

impl Metadata {
    /// Returns the contract's source code.
    pub fn source_code(&self) -> String {
        self.source_code.source_code()
    }

    /// Returns the contract's path mapped source code.
    pub fn sources(&self) -> HashMap<String, SourceCodeEntry> {
        self.source_code.sources()
    }

    /// Parses the Abi String as an [RawAbi] struct.
    pub fn raw_abi(&self) -> Result<RawAbi> {
        Ok(serde_json::from_str(&self.abi)?)
    }

    /// Parses the Abi String as an [Abi] struct.
    pub fn abi(&self) -> Result<Abi> {
        Ok(serde_json::from_str(&self.abi)?)
    }

    /// Parses the compiler version.
    pub fn compiler_version(&self) -> Result<Version> {
        let v = &self.compiler_version;
        let v = v.strip_prefix('v').unwrap_or(v);
        match v.parse() {
            Err(e) => {
                let v = v.replace('a', "-alpha.");
                let v = v.replace('b', "-beta.");
                v.parse()
                    .map_err(|_| BlockindexError::Unknown(format!("bad compiler version: {e}")))
            }
            Ok(v) => Ok(v),
        }
    }

    /// Maps this contract's sources to a [SourceTreeEntry] vector.
    pub fn source_entries(&self) -> Vec<SourceTreeEntry> {
        let root = Path::new(&self.contract_name);
        self.sources()
            .into_iter()
            .map(|(path, entry)| {
                let path = root.join(path);
                SourceTreeEntry { path, contents: entry.content }
            })
            .collect()
    }

    /// Returns the source tree of this contract's sources.
    pub fn source_tree(&self) -> SourceTree {
        SourceTree { entries: self.source_entries() }
    }

    /// Returns the contract's compiler settings.
    #[cfg(feature = "corebc-ylem")]
    pub fn settings(&self) -> Result<Settings> {
        let mut settings = self.source_code.settings()?.unwrap_or_default();

        if self.optimization_used == 1 && !settings.optimizer.enabled.unwrap_or_default() {
            settings.optimizer.enable();
            settings.optimizer.runs(self.runs as usize);
        }

        settings.evm_version = self.yvm_version()?;

        Ok(settings)
    }

    /// Creates a Ylem [ProjectBuilder] with this contract's settings.
    #[cfg(feature = "corebc-ylem")]
    pub fn project_builder(&self) -> Result<ProjectBuilder> {
        let ylem_config = YlemConfig::builder().settings(self.settings()?).build();

        Ok(Project::builder().ylem_config(ylem_config))
    }

    /// Parses the YVM version.
    #[cfg(feature = "corebc-ylem")]
    pub fn yvm_version(&self) -> Result<Option<CvmVersion>> {
        match self.yvm_version.as_str() {
            "" | "Default" => {
                Ok(CvmVersion::default().normalize_version(&self.compiler_version()?))
            }
            _ => {
                let yvm_version = self
                    .yvm_version
                    .parse()
                    .map_err(|e| BlockindexError::Unknown(format!("bad yvm version: {e}")))?;
                Ok(Some(yvm_version))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractMetadata {
    pub items: Vec<Metadata>,
}

impl IntoIterator for ContractMetadata {
    type Item = Metadata;
    type IntoIter = std::vec::IntoIter<Metadata>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl ContractMetadata {
    /// Returns the ABI of all contracts.
    pub fn abis(&self) -> Result<Vec<Abi>> {
        self.items.iter().map(|c| c.abi()).collect()
    }

    /// Returns the raw ABI of all contracts.
    pub fn raw_abis(&self) -> Result<Vec<RawAbi>> {
        self.items.iter().map(|c| c.raw_abi()).collect()
    }

    /// Returns the combined source code of all contracts.
    pub fn source_code(&self) -> String {
        self.items.iter().map(|c| c.source_code()).collect::<Vec<_>>().join("\n")
    }

    /// Returns the combined [SourceTree] of all contracts.
    pub fn source_tree(&self) -> SourceTree {
        SourceTree { entries: self.items.iter().flat_map(|item| item.source_entries()).collect() }
    }
}

impl Client {
    /// Fetches a verified contract's ABI.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = "0xBB9bc244D798123fDe783fCc1C72d3Bb8C189413".parse()?;
    /// let abi = client.contract_abi(address).await?;
    /// # Ok(()) }
    /// ```
    pub async fn contract_abi(&self, address: Address) -> Result<Abi> {
        //TODO:error2215 implement when blockindex wiil be ready
        Ok(Abi { ..Default::default() })
    }

    /// Fetches a contract's verified source code and its metadata.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = "0xBB9bc244D798123fDe783fCc1C72d3Bb8C189413".parse()?;
    /// let metadata = client.contract_source_code(address).await?;
    /// assert_eq!(metadata.items[0].contract_name, "DAO");
    /// # Ok(()) }
    /// ```
    pub async fn contract_source_code(&self, address: Address) -> Result<ContractMetadata> {
        //TODO:error2215 implement when blockindex wiil be ready
        Ok(ContractMetadata { items: vec![] })
    }
}
