use crate::{
    artifacts::Source,
    error::{Result, YlemError},
    utils, CompilerInput, CompilerOutput,
};
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt,
    io::BufRead,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    str::FromStr,
};
pub mod many;
pub mod output;
pub use output::{contracts, info, sources};
pub mod project;

/// The name of the `ylem` binary on the system
pub const YLEM: &str = "ylem";

pub const NUCLEUS_YLEM: Version = Version::new(1, 0, 1);

pub static SUPPORTS_BASE_PATH: once_cell::sync::Lazy<VersionReq> =
    once_cell::sync::Lazy::new(|| VersionReq::parse("^1.0.1").unwrap());

pub static SUPPORTS_INCLUDE_PATH: once_cell::sync::Lazy<VersionReq> =
    once_cell::sync::Lazy::new(|| VersionReq::parse("^1.0.1").unwrap());

#[cfg(any(test, feature = "tests"))]
use std::sync::Mutex;

#[cfg(any(test, feature = "tests"))]
#[allow(unused)]
static LOCK: once_cell::sync::Lazy<Mutex<()>> = once_cell::sync::Lazy::new(|| Mutex::new(()));

/// take the lock in tests, we use this to enforce that
/// a test does not run while a compiler version is being installed
///
/// This ensures that only one thread installs a missing `ylem` exe.
/// Instead of taking this lock in `Ylem::blocking_install`, the lock should be taken before
/// installation is detected.
#[cfg(any(test, feature = "tests"))]
#[allow(unused)]
pub(crate) fn take_ylem_installer_lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap()
}

/// A list of upstream Ylem releases, used to check which version
/// we should download.
/// The boolean value marks whether there was an error accessing the release list
#[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
pub static RELEASES: once_cell::sync::Lazy<(yvm::Releases, Vec<Version>, bool)> =
    once_cell::sync::Lazy::new(|| {
        match serde_json::from_str::<yvm::Releases>(yvm_builds::RELEASE_LIST_JSON) {
            Ok(releases) => {
                let sorted_versions = releases.clone().into_versions();
                (releases, sorted_versions, true)
            }
            Err(err) => {
                tracing::error!("{:?}", err);
                (yvm::Releases::default(), Vec::new(), false)
            }
        }
    });

/// A `Ylem` version is either installed (available locally) or can be downloaded, from the remote
/// endpoint
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum YlemVersion {
    Installed(Version),
    Remote(Version),
}

impl YlemVersion {
    /// Whether this version is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, YlemVersion::Installed(_))
    }
}

impl AsRef<Version> for YlemVersion {
    fn as_ref(&self) -> &Version {
        match self {
            YlemVersion::Installed(v) | YlemVersion::Remote(v) => v,
        }
    }
}

impl From<YlemVersion> for Version {
    fn from(s: YlemVersion) -> Version {
        match s {
            YlemVersion::Installed(v) | YlemVersion::Remote(v) => v,
        }
    }
}

impl fmt::Display for YlemVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Abstraction over `ylem` command line utility
///
/// Supports sync and async functions.
///
/// By default the ylem path is configured as follows, with descending priority:
///   1. `YLEM_PATH` environment variable
///   2. [yvm](https://github.com/roynalnaruto/yvm-rs)'s  `global_version` (set via `yvm use <version>`), stored at `<yvm_home>/.global_version`
///   3. `ylem` otherwise
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ylem {
    /// Path to the `ylem` executable
    pub ylem: PathBuf,
    /// The base path to set when invoking ylem, see also <https://docs.soliditylang.org/en/v0.8.11/path-resolution.html#base-path-and-include-paths>
    pub base_path: Option<PathBuf>,
    /// Additional arguments passed to the `ylem` exectuable
    pub args: Vec<String>,
}

impl Default for Ylem {
    fn default() -> Self {
        if let Ok(ylem) = std::env::var("YLEM_PATH") {
            return Ylem::new(ylem)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(ylem) = Ylem::yvm_global_version()
                .and_then(|vers| Ylem::find_yvm_installed_version(vers.to_string()).ok())
                .flatten()
            {
                return ylem
            }
        }

        Ylem::new(YLEM)
    }
}

impl fmt::Display for Ylem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ylem.display())?;
        if !self.args.is_empty() {
            write!(f, " {}", self.args.join(" "))?;
        }
        Ok(())
    }
}

impl Ylem {
    /// A new instance which points to `ylem`
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Ylem { ylem: path.into(), base_path: None, args: Vec::new() }
    }

    /// Sets ylem's base path
    ///
    /// Ref: <https://docs.soliditylang.org/en/v0.8.11/path-resolution.html#base-path-and-include-paths>
    pub fn with_base_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(base_path.into());
        self
    }

    /// Adds an argument to pass to the `ylem` command.
    #[must_use]
    pub fn arg<T: Into<String>>(mut self, arg: T) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to pass to the `ylem`.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for arg in args {
            self = self.arg(arg);
        }
        self
    }

    /// Returns the directory in which [yvm](https://github.com/roynalnaruto/yvm-rs) stores all versions
    ///
    /// This will be `~/.yvm` on unix
    #[cfg(not(target_arch = "wasm32"))]
    pub fn yvm_home() -> Option<PathBuf> {
        home::home_dir().map(|dir| dir.join(".yvm"))
    }

    /// Returns the `semver::Version` [yvm](https://github.com/roynalnaruto/yvm-rs)'s `.global_version` is currently set to.
    ///  `global_version` is configured with (`yvm use <version>`)
    ///
    /// This will read the version string (eg: "0.8.9") that the  `~/.yvm/.global_version` file
    /// contains
    #[cfg(not(target_arch = "wasm32"))]
    pub fn yvm_global_version() -> Option<Version> {
        let version =
            std::fs::read_to_string(Self::yvm_home().map(|p| p.join(".global_version"))?).ok()?;
        Version::parse(&version).ok()
    }

    /// Returns the list of all ylem instances installed at `YVM_HOME`
    #[cfg(not(target_arch = "wasm32"))]
    pub fn installed_versions() -> Vec<YlemVersion> {
        if let Some(home) = Self::yvm_home() {
            utils::installed_versions(home)
                .unwrap_or_default()
                .into_iter()
                .map(YlemVersion::Installed)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the list of all versions that are available to download and marking those which are
    /// already installed.
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub fn all_versions() -> Vec<YlemVersion> {
        let mut all_versions = Self::installed_versions();
        let mut uniques = all_versions
            .iter()
            .map(|v| {
                let v = v.as_ref();
                (v.major, v.minor, v.patch)
            })
            .collect::<std::collections::HashSet<_>>();
        all_versions.extend(
            RELEASES
                .1
                .clone()
                .into_iter()
                .filter(|v| uniques.insert((v.major, v.minor, v.patch)))
                .map(YlemVersion::Remote),
        );
        all_versions.sort_unstable();
        all_versions
    }

    /// Returns the path for a [yvm](https://github.com/roynalnaruto/yvm-rs) installed version.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///  use corebc_ylem::Ylem;
    /// let ylem = Ylem::find_yvm_installed_version("1.0.1").unwrap();
    /// assert_eq!(ylem, Some(Ylem::new("~/.yvm/1.0.1/ylem-1.0.1")));
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn find_yvm_installed_version(version: impl AsRef<str>) -> Result<Option<Self>> {
        let version = version.as_ref();
        let ylem = Self::yvm_home()
            .ok_or_else(|| YlemError::ylem("yvm home dir not found"))?
            .join(version)
            .join(format!("ylem-{version}"));

        if !ylem.is_file() {
            return Ok(None)
        }
        Ok(Some(Ylem::new(ylem)))
    }

    /// Returns the path for a [yvm](https://github.com/roynalnaruto/yvm-rs) installed version.
    ///
    /// If the version is not installed yet, it will install it.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///  use corebc_ylem::Ylem;
    /// let ylem = Ylem::find_or_install_yvm_version("1.0.1").unwrap();
    /// assert_eq!(ylem, Ylem::new("~/.yvm/1.0.1/ylem-1.0.1"));
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(not(target_arch = "wasm32"), all(feature = "yvm-ylem")))]
    pub fn find_or_install_yvm_version(version: impl AsRef<str>) -> Result<Self> {
        let version = version.as_ref();
        if let Some(ylem) = Ylem::find_yvm_installed_version(version)? {
            Ok(ylem)
        } else {
            Ok(Ylem::blocking_install(&version.parse::<Version>()?)?)
        }
    }

    /// Assuming the `versions` array is sorted, it returns the first element which satisfies
    /// the provided [`VersionReq`]
    pub fn find_matching_installation(
        versions: &[Version],
        required_version: &VersionReq,
    ) -> Option<Version> {
        // iterate in reverse to find the last match
        versions.iter().rev().find(|version| required_version.matches(version)).cloned()
    }

    /// Given a Solidity source, it detects the latest compiler version which can be used
    /// to build it, and returns it.
    ///
    /// If the required compiler version is not installed, it also proceeds to install it.
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub fn detect_version(source: &Source) -> Result<Version> {
        // detects the required ylem version
        let sol_version = Self::source_version_req(source)?;
        Self::ensure_installed(&sol_version)
    }

    /// Given a Solidity version requirement, it detects the latest compiler version which can be
    /// used to build it, and returns it.
    ///
    /// If the required compiler version is not installed, it also proceeds to install it.
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub fn ensure_installed(sol_version: &VersionReq) -> Result<Version> {
        #[cfg(any(test, feature = "tests"))]
        let _lock = take_ylem_installer_lock();

        // load the local / remote versions
        let versions = utils::installed_versions(yvm::YVM_DATA_DIR.as_path()).unwrap_or_default();

        let local_versions = Self::find_matching_installation(&versions, sol_version);
        let remote_versions = Self::find_matching_installation(&RELEASES.1, sol_version);
        // if there's a better upstream version than the one we have, install it
        Ok(match (local_versions, remote_versions) {
            (Some(local), None) => local,
            (Some(local), Some(remote)) => {
                if remote > local {
                    Self::blocking_install(&remote)?;
                    remote
                } else {
                    local
                }
            }
            (None, Some(version)) => {
                Self::blocking_install(&version)?;
                version
            }
            // do nothing otherwise
            _ => return Err(YlemError::VersionNotFound),
        })
    }

    /// Parses the given source looking for the `pragma` definition and
    /// returns the corresponding SemVer version requirement.
    pub fn source_version_req(source: &Source) -> Result<VersionReq> {
        let version =
            utils::find_version_pragma(&source.content).ok_or(YlemError::PragmaNotFound)?;
        Self::version_req(version.as_str())
    }

    /// Returns the corresponding SemVer version requirement for the solidity version
    pub fn version_req(version: &str) -> Result<VersionReq> {
        let version = version.replace(' ', ",");

        // Somehow, Ylem semver without an operator is considered to be "exact",
        // but lack of operator automatically marks the operator as Caret, so we need
        // to manually patch it? :shrug:
        let exact = !matches!(&version[0..1], "*" | "^" | "=" | ">" | "<" | "~");
        let mut version = VersionReq::parse(&version)?;
        if exact {
            version.comparators[0].op = semver::Op::Exact;
        }

        Ok(version)
    }

    /// Installs the provided version of Ylem in the machine under the yvm dir and returns the
    /// [Ylem] instance pointing to the installation.
    ///
    /// # Example
    /// ```no_run
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    ///  use corebc_ylem::{Ylem, NUCLEUS_YLEM};
    ///  let ylem = Ylem::install(&NUCLEUS_YLEM).await.unwrap();
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub async fn install(version: &Version) -> std::result::Result<Self, yvm::YlemVmError> {
        tracing::trace!("installing ylem version \"{}\"", version);
        crate::report::ylem_installation_start(version);
        let result = yvm::install(version).await;
        crate::report::ylem_installation_success(version);
        result.map(Ylem::new)
    }

    /// Blocking version of `Self::install`
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub fn blocking_install(version: &Version) -> std::result::Result<Self, yvm::YlemVmError> {
        use crate::utils::RuntimeOrHandle;

        tracing::trace!("blocking installing ylem version \"{}\"", version);
        crate::report::ylem_installation_start(version);
        // the async version `yvm::install` is used instead of `yvm::blocking_intsall`
        // because the underlying `reqwest::blocking::Client` does not behave well
        // in tokio rt. see https://github.com/seanmonstar/reqwest/issues/1017
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let installation = yvm::blocking_install(version);
            } else {
                let installation = RuntimeOrHandle::new().block_on(yvm::install(version));
            }
        };
        match installation {
            Ok(path) => {
                crate::report::ylem_installation_success(version);
                Ok(Ylem::new(path))
            }
            Err(err) => {
                crate::report::ylem_installation_error(version, &err.to_string());
                Err(err)
            }
        }
    }

    /// Verify that the checksum for this version of ylem is correct. We check against the SHA256
    /// checksum from the build information published by [binaries.soliditylang.org](https://binaries.soliditylang.org/)
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    pub fn verify_checksum(&self) -> Result<()> {
        let version = self.version_short()?;
        let mut version_path = yvm::version_path(version.to_string().as_str());
        version_path.push(format!("ylem-{}", version.to_string().as_str()));
        tracing::trace!(target:"ylem", "reading ylem binary for checksum {:?}", version_path);
        let content =
            std::fs::read(&version_path).map_err(|err| YlemError::io(err, version_path.clone()))?;

        if !RELEASES.2 {
            // we skip checksum verification because the underlying request to fetch release info
            // failed so we have nothing to compare against
            return Ok(())
        }

        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(content);
        let checksum_calc = &hasher.finalize()[..];

        let checksum_found = &RELEASES.0.get_checksum(&version).expect("checksum not found");

        if checksum_calc == checksum_found {
            Ok(())
        } else {
            let expected = hex::encode(checksum_found);
            let detected = hex::encode(checksum_calc);
            tracing:: warn!(target : "ylem", "checksum mismatch for {:?}, expected {}, but found {} for file {:?}", version, expected, detected, version_path);
            Err(YlemError::ChecksumMismatch { version, expected, detected, file: version_path })
        }
    }

    /// Convenience function for compiling all sources under the given path
    pub fn compile_source(&self, path: impl AsRef<Path>) -> Result<CompilerOutput> {
        let path = path.as_ref();
        let mut res: CompilerOutput = Default::default();
        for input in CompilerInput::new(path)? {
            let output = self.compile(&input)?;
            res.merge(output)
        }
        Ok(res)
    }

    /// Same as [`Self::compile()`], but only returns those files which are included in the
    /// `CompilerInput`.
    ///
    /// In other words, this removes those files from the `CompilerOutput` that are __not__ included
    /// in the provided `CompilerInput`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///  use corebc_ylem::{CompilerInput, Ylem};
    /// let ylem = Ylem::default();
    /// let input = CompilerInput::new("./contracts")?[0].clone();
    /// let output = ylem.compile_exact(&input)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn compile_exact(&self, input: &CompilerInput) -> Result<CompilerOutput> {
        let mut out = self.compile(input)?;
        out.retain_files(input.sources.keys().filter_map(|p| p.to_str()));
        Ok(out)
    }

    /// Run `ylem --stand-json` and return the `ylem`'s output as
    /// `CompilerOutput`
    ///
    /// # Example
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///  use corebc_ylem::{CompilerInput, Ylem};
    /// let ylem = Ylem::default();
    /// let input = CompilerInput::new("./contracts")?;
    /// let output = ylem.compile(&input)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn compile<T: Serialize>(&self, input: &T) -> Result<CompilerOutput> {
        self.compile_as(input)
    }

    /// Run `ylem --standart-json` and return the `ylem`'s output as the given json
    /// output
    pub fn compile_as<T: Serialize, D: DeserializeOwned>(&self, input: &T) -> Result<D> {
        let output = self.compile_output(input)?;
        Ok(serde_json::from_slice(&output)?)
    }

    pub fn compile_output<T: Serialize>(&self, input: &T) -> Result<Vec<u8>> {
        let mut cmd = Command::new(&self.ylem);

        if let Some(ref base_path) = self.base_path {
            cmd.current_dir(base_path);
            cmd.arg("--base-path").arg(base_path);
        }

        let mut child = cmd
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|err| YlemError::io(err, &self.ylem))?;

        let stdin = child.stdin.take().expect("Stdin exists.");

        serde_json::to_writer(stdin, input)?;
        let output = child.wait_with_output().map_err(|err| YlemError::io(err, &self.ylem))?;
        compile_output(output)
    }

    pub fn version_short(&self) -> Result<Version> {
        let version = self.version()?;
        Ok(Version::new(version.major, version.minor, version.patch))
    }

    /// Returns the version from the configured `ylem`
    pub fn version(&self) -> Result<Version> {
        version_from_output(
            Command::new(&self.ylem)
                .arg("--version")
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .output()
                .map_err(|err| YlemError::io(err, &self.ylem))?,
        )
    }
}

#[cfg(feature = "async")]
impl Ylem {
    /// Convenience function for compiling all sources under the given path
    pub async fn async_compile_source(&self, path: impl AsRef<Path>) -> Result<CompilerOutput> {
        self.async_compile(&CompilerInput::with_sources(Source::async_read_all_from(path).await?))
            .await
    }

    /// Run `ylem --stand-json` and return the `ylem`'s output as
    /// `CompilerOutput`
    pub async fn async_compile<T: Serialize>(&self, input: &T) -> Result<CompilerOutput> {
        self.async_compile_as(input).await
    }

    /// Run `ylem --stand-json` and return the `ylem`'s output as the given json
    /// output
    pub async fn async_compile_as<T: Serialize, D: DeserializeOwned>(
        &self,
        input: &T,
    ) -> Result<D> {
        let output = self.async_compile_output(input).await?;
        Ok(serde_json::from_slice(&output)?)
    }

    pub async fn async_compile_output<T: Serialize>(&self, input: &T) -> Result<Vec<u8>> {
        use tokio::io::AsyncWriteExt;
        let content = serde_json::to_vec(input)?;
        let mut cmd = tokio::process::Command::new(&self.ylem);
        if let Some(ref base_path) = self.base_path {
            cmd.current_dir(base_path);
        }
        let mut child = cmd
            .args(&self.args)
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|err| YlemError::io(err, &self.ylem))?;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&content).await.map_err(|err| YlemError::io(err, &self.ylem))?;
        stdin.flush().await.map_err(|err| YlemError::io(err, &self.ylem))?;
        compile_output(
            child.wait_with_output().await.map_err(|err| YlemError::io(err, &self.ylem))?,
        )
    }

    pub async fn async_version(&self) -> Result<Version> {
        version_from_output(
            tokio::process::Command::new(&self.ylem)
                .arg("--version")
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|err| YlemError::io(err, &self.ylem))?
                .wait_with_output()
                .await
                .map_err(|err| YlemError::io(err, &self.ylem))?,
        )
    }

    /// Compiles all `CompilerInput`s with their associated `Ylem`.
    ///
    /// This will buffer up to `n` `ylem` processes and then return the `CompilerOutput`s in the
    /// order in which they complete. No more than `n` futures will be buffered at any point in
    /// time, and less than `n` may also be buffered depending on the state of each future.
    ///
    /// # Example
    ///
    /// Compile 2 `CompilerInput`s at once
    ///
    /// ```no_run
    /// # async fn example() {
    /// use corebc_ylem::{CompilerInput, Ylem};
    /// let ylem1 = Ylem::default();
    /// let ylem2 = Ylem::default();
    /// let input1 = CompilerInput::new("contracts").unwrap()[0].clone();
    /// let input2 = CompilerInput::new("src").unwrap()[0].clone();
    ///
    /// let outputs = Ylem::compile_many([(ylem1, input1), (ylem2, input2)], 2).await.flattened().unwrap();
    /// # }
    /// ```
    pub async fn compile_many<I>(jobs: I, n: usize) -> crate::many::CompiledMany
    where
        I: IntoIterator<Item = (Ylem, CompilerInput)>,
    {
        use futures_util::stream::StreamExt;

        let outputs = futures_util::stream::iter(
            jobs.into_iter()
                .map(|(ylem, input)| async { (ylem.async_compile(&input).await, ylem, input) }),
        )
        .buffer_unordered(n)
        .collect::<Vec<_>>()
        .await;

        crate::many::CompiledMany::new(outputs)
    }
}

fn compile_output(output: Output) -> Result<Vec<u8>> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(YlemError::ylem(String::from_utf8_lossy(&output.stderr).to_string()))
    }
}

fn version_from_output(output: Output) -> Result<Version> {
    if output.status.success() {
        let version = output
            .stdout
            .lines()
            .map_while(std::result::Result::ok)
            .filter(|l| !l.trim().is_empty())
            .last()
            .ok_or_else(|| YlemError::ylem("version not found in ylem output"))?;
        // NOTE: semver doesn't like `+` in g++ in build metadata which is invalid semver
        Ok(Version::from_str(&version.trim_start_matches("Version: ").replace(".g++", ".gcc"))?)
    } else {
        Err(YlemError::ylem(String::from_utf8_lossy(&output.stderr).to_string()))
    }
}

impl AsRef<Path> for Ylem {
    fn as_ref(&self) -> &Path {
        &self.ylem
    }
}

impl<T: Into<PathBuf>> From<T> for Ylem {
    fn from(ylem: T) -> Self {
        Ylem::new(ylem.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{artifact_output::Artifact, CompilerInput};

    fn ylem() -> Ylem {
        Ylem::default()
    }

    #[test]
    fn ylem_version_works() {
        ylem().version().unwrap();
    }

    #[test]
    fn can_parse_version_metadata() {
        let _version = Version::from_str("0.6.6+commit.6c089d02.Linux.gcc").unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_ylem_version_works() {
        let _version = ylem().async_version().await.unwrap();
    }

    #[test]
    fn ylem_compile_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = ylem().compile(&input).unwrap();
        let other = ylem().compile(&serde_json::json!(input)).unwrap();
        assert_eq!(out, other);
    }

    #[test]
    fn ylem_metadata_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let mut input: CompilerInput = serde_json::from_str(input).unwrap();
        input.settings.push_output_selection("metadata");
        let out = ylem().compile(&input).unwrap();
        for (_, c) in out.split().1.contracts_iter() {
            assert!(c.metadata.is_some());
        }
    }

    #[test]
    fn can_compile_with_remapped_links() {
        let input: CompilerInput =
            serde_json::from_str(include_str!("../../test-data/library-remapping-in.json"))
                .unwrap();
        let out = ylem().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[test]
    fn can_compile_with_remapped_links_temp_dir() {
        let input: CompilerInput =
            serde_json::from_str(include_str!("../../test-data/library-remapping-in-2.json"))
                .unwrap();
        let out = ylem().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_ylem_compile_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = ylem().async_compile(&input).await.unwrap();
        let other = ylem().async_compile(&serde_json::json!(input)).await.unwrap();
        assert_eq!(out, other);
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_ylem_compile_works2() {
        let input = include_str!("../../test-data/in/compiler-in-2.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = ylem().async_compile(&input).await.unwrap();
        let other = ylem().async_compile(&serde_json::json!(input)).await.unwrap();
        assert_eq!(out, other);
        let sync_out = ylem().compile(&input).unwrap();
        assert_eq!(out, sync_out);
    }

    #[test]
    // This test might be a bit hard to maintain
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    fn test_detect_version() {
        for (pragma, expected) in [
            // pinned
            ("^1.0.1", "1.0.1"),
            // pinned too
        ]
        .iter()
        {
            let source = source(pragma);
            let res = Ylem::detect_version(&source).unwrap();
            assert_eq!(res, Version::from_str(expected).unwrap());
        }
    }

    #[test]
    #[cfg(feature = "full")]
    fn test_find_installed_version_path() {
        // this test does not take the lock by default, so we need to manually
        // add it here.
        let _lock = LOCK.lock();
        let ver = "1.0.1";
        let version = Version::from_str(ver).unwrap();
        if utils::installed_versions(yvm::YVM_DATA_DIR.as_path())
            .map(|versions| !versions.contains(&version))
            .unwrap_or_default()
        {
            Ylem::blocking_install(&version).unwrap();
        }
        let res = Ylem::find_yvm_installed_version(version.to_string()).unwrap().unwrap();
        let expected = yvm::YVM_DATA_DIR.join(ver).join(format!("ylem-{ver}"));
        assert_eq!(res.ylem, expected);
    }

    #[test]
    #[cfg(all(feature = "yvm-ylem", not(target_arch = "wasm32")))]
    fn can_install_ylem_in_tokio_rt() {
        let version = Version::from_str("1.0.1").unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { Ylem::blocking_install(&version) });
        assert!(result.is_ok());
    }

    #[test]
    fn does_not_find_not_installed_version() {
        let ver = "1.1.1";
        let version = Version::from_str(ver).unwrap();
        let res = Ylem::find_yvm_installed_version(version.to_string()).unwrap();
        assert!(res.is_none());
    }

    ///// helpers

    fn source(version: &str) -> Source {
        Source::new(format!("pragma solidity {version};\n"))
    }
}
