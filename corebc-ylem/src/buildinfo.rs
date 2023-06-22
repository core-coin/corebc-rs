//! Represents an entire build

use crate::{utils, CompilerInput, CompilerOutput, YlemError};
use md5::Digest;
use semver::Version;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::{cell::RefCell, path::Path, rc::Rc};

pub const ETHERS_FORMAT_VERSION: &str = "corebc-rs-sol-build-info-1";

// A hardhat compatible build info representation
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub id: String,
    #[serde(rename = "_format")]
    pub format: String,
    pub ylem_version: Version,
    pub ylem_long_version: Version,
    pub input: CompilerInput,
    pub output: CompilerOutput,
}

impl BuildInfo {
    /// Deserializes the `BuildInfo` object from the given file
    pub fn read(path: impl AsRef<Path>) -> Result<Self, YlemError> {
        utils::read_json_file(path)
    }
}

/// Represents `BuildInfo` object
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RawBuildInfo {
    /// The hash that identifies the BuildInfo
    pub id: String,
    /// serialized `BuildInfo` json
    pub build_info: String,
}

// === impl RawBuildInfo ===

impl RawBuildInfo {
    /// Serializes a `BuildInfo` object
    pub fn new(
        input: &CompilerInput,
        output: &CompilerOutput,
        version: &Version,
    ) -> serde_json::Result<RawBuildInfo> {
        let mut hasher = md5::Md5::new();
        let w = BuildInfoWriter { buf: Rc::new(RefCell::new(Vec::with_capacity(128))) };
        let mut buf = w.clone();
        let mut serializer = serde_json::Serializer::pretty(&mut buf);
        let mut s = serializer.serialize_struct("BuildInfo", 6)?;
        s.serialize_field("_format", &ETHERS_FORMAT_VERSION)?;
        let ylem_short = format!("{}.{}.{}", version.major, version.minor, version.patch);
        s.serialize_field("ylemVersion", &ylem_short)?;
        s.serialize_field("ylemLongVersion", &version)?;
        s.serialize_field("input", input)?;

        // create the hash for `{_format,ylemVersion,ylemLongVersion,input}`
        // N.B. this is not exactly the same as hashing the json representation of these values but
        // the must efficient one
        hasher.update(&*w.buf.borrow());
        let result = hasher.finalize();
        let id = hex::encode(result);

        s.serialize_field("id", &id)?;
        s.serialize_field("output", output)?;
        s.end()?;

        drop(buf);

        let build_info = unsafe {
            // serde_json does not emit non UTF8
            String::from_utf8_unchecked(w.buf.take())
        };

        Ok(RawBuildInfo { id, build_info })
    }
}

#[derive(Clone)]
struct BuildInfoWriter {
    buf: Rc<RefCell<Vec<u8>>>,
}

impl std::io::Write for BuildInfoWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.borrow_mut().flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Source;
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn build_info_serde() {
        let inputs = CompilerInput::with_sources(BTreeMap::from([(
            PathBuf::from("input.sol"),
            Source::new(""),
        )]));
        let output = CompilerOutput::default();
        let v: Version = "1.0.1+commit.c7e474f2".parse().unwrap();
        let _raw_info = RawBuildInfo::new(&inputs[0], &output, &v).unwrap();
        // let _info: BuildInfo = serde_json::from_str(&raw_info.build_info).unwrap();
    }
}
