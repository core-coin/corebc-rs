use semver::Version;
use std::{
    io,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, YlemError>;

/// Various error types
#[derive(Debug, Error)]
pub enum YlemError {
    /// Internal ylem error
    #[error("Ylem Error: {0}")]
    YlemError(String),
    #[error("Missing pragma from solidity file")]
    PragmaNotFound,
    #[error("Could not find ylem version locally or upstream")]
    VersionNotFound,
    #[error("Checksum mismatch for {file}: expected {expected} found {detected} for {version}")]
    ChecksumMismatch { version: Version, expected: String, detected: String, file: PathBuf },
    #[error(transparent)]
    SemverError(#[from] semver::Error),
    /// Deserialization error
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    /// Filesystem IO error
    #[error(transparent)]
    Io(#[from] YlemIoError),
    #[error("File could not be resolved due to broken symlink: {0}.")]
    ResolveBadSymlink(YlemIoError),
    /// Failed to resolve a file
    #[error("Failed to resolve file: {0}.\n Check configured remappings.")]
    Resolve(YlemIoError),
    #[error("File cannot be resolved due to mismatch of file name case: {error}.\n Found existing file: {existing_file:?}\n Please check the case of the import.")]
    ResolveCaseSensitiveFileName { error: YlemIoError, existing_file: PathBuf },
    #[error(
        r#"{0}.
    --> {1:?}
        {2:?}"#
    )]
    FailedResolveImport(Box<YlemError>, PathBuf, PathBuf),
    #[cfg(all(feature = "svm-ylem", not(target_arch = "wasm32")))]
    #[error(transparent)]
    SvmError(#[from] svm::SolcVmError),
    #[error("No contracts found at \"{0}\"")]
    NoContracts(String),
    #[error(transparent)]
    PatternError(#[from] glob::PatternError),
    /// General purpose message
    #[error("{0}")]
    Message(String),

    #[error("No artifact found for `{}:{}`", .0.display(), .1)]
    ArtifactNotFound(PathBuf, String),

    #[cfg(feature = "project-util")]
    #[error(transparent)]
    FsExtra(#[from] fs_extra::error::Error),
}

impl YlemError {
    pub(crate) fn io(err: io::Error, path: impl Into<PathBuf>) -> Self {
        YlemIoError::new(err, path).into()
    }
    pub(crate) fn ylem(msg: impl Into<String>) -> Self {
        YlemError::YlemError(msg.into())
    }
    pub fn msg(msg: impl Into<String>) -> Self {
        YlemError::Message(msg.into())
    }
}

macro_rules! _format_err {
    ($($tt:tt)*) => {
        $crate::error::YlemError::msg(format!($($tt)*))
    };
}
#[allow(unused)]
pub(crate) use _format_err as format_err;

macro_rules! _bail {
    ($($tt:tt)*) => { return Err($crate::error::format_err!($($tt)*)) };
}
#[allow(unused)]
pub(crate) use _bail as bail;

#[derive(Debug, Error)]
#[error("\"{}\": {io}", self.path.display())]
pub struct YlemIoError {
    io: io::Error,
    path: PathBuf,
}

impl YlemIoError {
    pub fn new(io: io::Error, path: impl Into<PathBuf>) -> Self {
        Self { io, path: path.into() }
    }

    /// The path at which the error occurred
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The underlying `io::Error`
    pub fn source(&self) -> &io::Error {
        &self.io
    }
}

impl From<YlemIoError> for io::Error {
    fn from(err: YlemIoError) -> Self {
        err.io
    }
}
