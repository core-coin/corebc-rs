use cargo_metadata::MetadataCommand;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    env, fmt, fs,
    path::{Path, PathBuf},
};
use strum::{EnumCount, EnumIter, EnumString, EnumVariantNames, IntoEnumIterator};

/// `corebc_crate => name`
type CrateNames = HashMap<CorebcCrate, &'static str>;

const DIRS: [&str; 3] = ["benches", "examples", "tests"];

/// Maps an [`CorebcCrate`] to its path string.
///
/// See [`ProjectEnvironment`] for more information.
///
/// Note: this static variable cannot hold [`syn::Path`] because it is not [`Sync`], so the names
/// must be parsed at every call.
static COREBC_CRATE_NAMES: Lazy<CrateNames> = Lazy::new(|| {
    ProjectEnvironment::new_from_env()
        .and_then(|x| x.determine_corebc_crates())
        .unwrap_or_else(|| CorebcCrate::corebc_path_names().collect())
});

/// Returns the `core` crate's [`Path`][syn::Path].
#[inline]
pub fn corebc_core_crate() -> syn::Path {
    get_crate_path(CorebcCrate::CorebcCore)
}

/// Returns the `contract` crate's [`Path`][syn::Path].
#[inline]
pub fn corebc_contract_crate() -> syn::Path {
    get_crate_path(CorebcCrate::CorebcContract)
}

/// Returns the `providers` crate's [`Path`][syn::Path].
#[inline]
pub fn corebc_providers_crate() -> syn::Path {
    get_crate_path(CorebcCrate::CorebcProviders)
}

/// Returns an [`CorebcCrate`]'s [`Path`][syn::Path] in the current project.
#[inline(always)]
pub fn get_crate_path(krate: CorebcCrate) -> syn::Path {
    krate.get_path()
}

/// Represents a generic Rust/Cargo project's environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectEnvironment {
    manifest_dir: PathBuf,
    crate_name: Option<String>,
}

impl ProjectEnvironment {
    /// Creates a new instance using the given manifest dir and crate name.
    pub fn new<T: Into<PathBuf>, U: Into<String>>(manifest_dir: T, crate_name: U) -> Self {
        Self { manifest_dir: manifest_dir.into(), crate_name: Some(crate_name.into()) }
    }

    /// Creates a new instance using the the `CARGO_MANIFEST_DIR` and `CARGO_CRATE_NAME` environment
    /// variables.
    pub fn new_from_env() -> Option<Self> {
        Some(Self {
            manifest_dir: env::var_os("CARGO_MANIFEST_DIR")?.into(),
            crate_name: env::var("CARGO_CRATE_NAME").ok(),
        })
    }

    /// Determines the crate paths to use by looking at the [metadata][cargo_metadata] of the
    /// project.
    ///
    /// The names will be:
    /// - `corebc::*` if `corebc` is a dependency for all crates;
    /// - for each `crate`:
    ///   - `corebc_<crate>` if it is a dependency, otherwise `corebc::<crate>`.
    #[inline]
    pub fn determine_corebc_crates(&self) -> Option<CrateNames> {
        let lock_file = self.manifest_dir.join("Cargo.lock");
        let lock_file_existed = lock_file.exists();

        let names = self.crate_names_from_metadata();

        // remove the lock file created from running the command
        if !lock_file_existed && lock_file.exists() {
            let _ = std::fs::remove_file(lock_file);
        }

        names
    }

    #[inline]
    fn crate_names_from_metadata(&self) -> Option<CrateNames> {
        let metadata = MetadataCommand::new().current_dir(&self.manifest_dir).exec().ok()?;
        let pkg = metadata.root_package()?;

        // return corebc_* if the root package is an internal corebc crate since `corebc` is not
        // available
        if pkg.name.parse::<CorebcCrate>().is_ok() || pkg.name == "corebc" {
            return Some(CorebcCrate::path_names().collect())
        }

        let mut names: CrateNames = CorebcCrate::corebc_path_names().collect();
        for dep in pkg.dependencies.iter() {
            let name = dep.name.as_str();
            if name.starts_with("corebc") {
                if name == "corebc" {
                    return None
                } else if let Ok(dep) = name.parse::<CorebcCrate>() {
                    names.insert(dep, dep.path_name());
                }
            }
        }
        Some(names)
    }

    /// Returns whether the `crate` path identifier refers to the root package.
    ///
    /// This is false for integration tests, benches, and examples, as the `crate` keyword will not
    /// refer to the root package.
    ///
    /// We can find this using some [environment variables set by Cargo during compilation][ref]:
    /// - `CARGO_TARGET_TMPDIR` is only set when building integration test or benchmark code;
    /// - When `CARGO_MANIFEST_DIR` contains `/benches/` or `/examples/`
    /// - `CARGO_CRATE_NAME`, see `is_crate_name_in_dirs`.
    ///
    /// [ref]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    #[inline]
    pub fn is_crate_root(&self) -> bool {
        env::var_os("CARGO_TARGET_TMPDIR").is_none() &&
            self.manifest_dir.components().all(|c| {
                let s = c.as_os_str();
                s != "examples" && s != "benches"
            }) &&
            !self.is_crate_name_in_dirs()
    }

    /// Returns whether `crate_name` is the name of a file or directory in the first level of
    /// `manifest_dir/{benches,examples,tests}/`.
    ///
    /// # Example
    ///
    /// With this project structure:
    ///
    /// ```text
    /// .
    /// ├── Cargo.lock
    /// ├── Cargo.toml
    /// ├── src/
    /// │   ...
    /// ├── benches/
    /// │   ├── large-input.rs
    /// │   └── multi-file-bench/
    /// │       ├── main.rs
    /// │       └── bench_module.rs
    /// ├── examples/
    /// │   ├── simple.rs
    /// │   └── multi-file-example/
    /// │       ├── main.rs
    /// │       └── ex_module.rs
    /// └── tests/
    ///     ├── some-integration-tests.rs
    ///     └── multi-file-test/
    ///         ├── main.rs
    ///         └── test_module.rs
    /// ```
    ///
    /// The resulting `CARGO_CRATE_NAME` values will be:
    ///
    /// |                  Path                  |          Value         |
    /// |:-------------------------------------- | ----------------------:|
    /// | benches/large-input.rs                 |            large-input |
    /// | benches/multi-file-bench/\*\*/\*.rs    |       multi-file-bench |
    /// | examples/simple.rs                     |                 simple |
    /// | examples/multi-file-example/\*\*/\*.rs |     multi-file-example |
    /// | tests/some-integration-tests.rs        | some-integration-tests |
    /// | tests/multi-file-test/\*\*/\*.rs       |        multi-file-test |
    #[inline]
    pub fn is_crate_name_in_dirs(&self) -> bool {
        let crate_name = match self.crate_name.as_ref() {
            Some(name) => name,
            None => return false,
        };
        let dirs = DIRS.map(|dir| self.manifest_dir.join(dir));
        dirs.iter().any(|dir| {
            fs::read_dir(dir)
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .find(|entry| file_stem_eq(entry.path(), crate_name))
                })
                .is_some()
        })
    }
}

/// An `corebc-rs` internal crate.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    EnumCount,
    EnumIter,
    EnumString,
    EnumVariantNames,
)]
#[strum(serialize_all = "kebab-case")]
pub enum CorebcCrate {
    CorebcAddressbook,
    CorebcContract,
    CorebcContractAbigen,
    CorebcContractDerive,
    CorebcCore,
    CorebcBlockindex,
    CorebcMiddleware,
    CorebcProviders,
    CorebcSigners,
    CorebcYlem,
}

impl AsRef<str> for CorebcCrate {
    fn as_ref(&self) -> &str {
        self.crate_name()
    }
}

impl fmt::Display for CorebcCrate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.as_ref())
    }
}

impl CorebcCrate {
    /// "`<self as kebab-case>`"
    #[inline]
    pub const fn crate_name(self) -> &'static str {
        match self {
            Self::CorebcAddressbook => "corebc-addressbook",
            Self::CorebcContract => "corebc-contract",
            Self::CorebcContractAbigen => "corebc-contract-abigen",
            Self::CorebcContractDerive => "corebc-contract-derive",
            Self::CorebcCore => "corebc-core",
            Self::CorebcBlockindex => "corebc-blockindex",
            Self::CorebcMiddleware => "corebc-middleware",
            Self::CorebcProviders => "corebc-providers",
            Self::CorebcSigners => "corebc-signers",
            Self::CorebcYlem => "corebc-ylem",
        }
    }

    /// "`::<self as snake_case>`"
    #[inline]
    pub const fn path_name(self) -> &'static str {
        match self {
            Self::CorebcAddressbook => "::corebc_addressbook",
            Self::CorebcContract => "::corebc_contract",
            Self::CorebcContractAbigen => "::corebc_contract_abigen",
            Self::CorebcContractDerive => "::corebc_contract_derive",
            Self::CorebcCore => "::corebc_core",
            Self::CorebcBlockindex => "::corebc_blockindex",
            Self::CorebcMiddleware => "::corebc_middleware",
            Self::CorebcProviders => "::corebc_providers",
            Self::CorebcSigners => "::corebc_signers",
            Self::CorebcYlem => "::corebc_ylem",
        }
    }

    /// "::corebc::`<self in corebc>`"
    #[inline]
    pub const fn corebc_path_name(self) -> &'static str {
        match self {
            // re-exported in corebc::contract
            Self::CorebcContractAbigen => "::corebc::contract", // partially
            Self::CorebcContractDerive => "::corebc::contract",

            Self::CorebcAddressbook => "::corebc::addressbook",
            Self::CorebcContract => "::corebc::contract",
            Self::CorebcCore => "::corebc::core",
            Self::CorebcBlockindex => "::corebc::blockindex",
            Self::CorebcMiddleware => "::corebc::middleware",
            Self::CorebcProviders => "::corebc::providers",
            Self::CorebcSigners => "::corebc::signers",
            Self::CorebcYlem => "::corebc::ylem",
        }
    }

    /// The path on the file system, from an `corebc-rs` root directory.
    #[inline]
    pub const fn fs_path(self) -> &'static str {
        match self {
            Self::CorebcContractAbigen => "corebc-contract/corebc-contract-abigen",
            Self::CorebcContractDerive => "corebc-contract/corebc-contract-derive",
            _ => self.crate_name(),
        }
    }

    /// `<corebc_*>`
    #[inline]
    pub fn path_names() -> impl Iterator<Item = (Self, &'static str)> {
        Self::iter().map(|x| (x, x.path_name()))
    }

    /// `<corebc::*>`
    #[inline]
    pub fn corebc_path_names() -> impl Iterator<Item = (Self, &'static str)> {
        Self::iter().map(|x| (x, x.corebc_path_name()))
    }

    /// Returns the [`Path`][syn::Path] in the current project.
    #[inline]
    pub fn get_path(&self) -> syn::Path {
        let name = COREBC_CRATE_NAMES[self];
        syn::parse_str(name).unwrap()
    }
}

/// `path.file_stem() == s`
#[inline]
fn file_stem_eq<T: AsRef<Path>, U: AsRef<str>>(path: T, s: U) -> bool {
    if let Some(stem) = path.as_ref().file_stem() {
        if let Some(stem) = stem.to_str() {
            return stem == s.as_ref()
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{
        distributions::{Distribution, Standard},
        thread_rng, Rng,
    };
    use std::{
        collections::{BTreeMap, HashSet},
        env, fs,
    };
    use tempfile::TempDir;

    impl Distribution<CorebcCrate> for Standard {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CorebcCrate {
            const RANGE: std::ops::Range<u8> = 0..CorebcCrate::COUNT as u8;
            // SAFETY: generates in the safe range
            unsafe { std::mem::transmute(rng.gen_range(RANGE)) }
        }
    }

    #[test]
    #[ignore = "TODO: flaky and slow"]
    fn test_names() {
        fn assert_names(s: &ProjectEnvironment, corebc: bool, dependencies: &[CorebcCrate]) {
            write_manifest(s, corebc, dependencies);

            // speeds up consecutive runs by not having to re-create and delete the lockfile
            // this is tested separately: test_lock_file
            std::fs::write(s.manifest_dir.join("Cargo.lock"), "").unwrap();

            let names = s
                .determine_corebc_crates()
                .unwrap_or_else(|| CorebcCrate::corebc_path_names().collect());

            let krate = s.crate_name.as_ref().and_then(|x| x.parse::<CorebcCrate>().ok());
            let is_internal = krate.is_some();
            let expected: CrateNames = match (is_internal, corebc) {
                // internal
                (true, _) => CorebcCrate::path_names().collect(),

                // corebc
                (_, true) => CorebcCrate::corebc_path_names().collect(),

                // no corebc
                (_, false) => {
                    let mut n: CrateNames = CorebcCrate::corebc_path_names().collect();
                    for &dep in dependencies {
                        n.insert(dep, dep.path_name());
                    }
                    n
                }
            };

            // don't use assert for a better custom message
            if names != expected {
                // BTreeMap sorts the keys
                let names: BTreeMap<_, _> = names.into_iter().collect();
                let expected: BTreeMap<_, _> = expected.into_iter().collect();
                panic!("\nCase failed: (`{:?}`, `{corebc}`, `{dependencies:?}`)\nNames: {names:#?}\nExpected: {expected:#?}\n", s.crate_name);
            }
        }

        fn gen_unique<const N: usize>() -> [CorebcCrate; N] {
            assert!(N < CorebcCrate::COUNT);
            let rng = &mut thread_rng();
            let mut set = HashSet::with_capacity(N);
            while set.len() < N {
                set.insert(rng.gen());
            }
            let vec: Vec<_> = set.into_iter().collect();
            vec.try_into().unwrap()
        }

        let (s, _dir) = test_project();
        // crate_name        -> represents an external crate
        // "corebc-contract" -> represents an internal crate
        for name in [s.crate_name.as_ref().unwrap(), "corebc-contract"] {
            let s = ProjectEnvironment::new(&s.manifest_dir, name);
            // only corebc
            assert_names(&s, true, &[]);

            // only others
            assert_names(&s, false, gen_unique::<3>().as_slice());

            // corebc and others
            assert_names(&s, true, gen_unique::<3>().as_slice());
        }
    }

    #[test]
    #[ignore = "TODO: flaky and slow"]
    fn test_lock_file() {
        let (s, _dir) = test_project();
        write_manifest(&s, true, &[]);
        let lock_file = s.manifest_dir.join("Cargo.lock");

        assert!(!lock_file.exists());
        s.determine_corebc_crates();
        assert!(!lock_file.exists());

        std::fs::write(&lock_file, "").unwrap();

        assert!(lock_file.exists());
        s.determine_corebc_crates();
        assert!(lock_file.exists());
        assert!(!std::fs::read(lock_file).unwrap().is_empty());
    }

    #[test]
    fn test_is_crate_root() {
        let (s, _dir) = test_project();
        assert!(s.is_crate_root());

        // `CARGO_MANIFEST_DIR`
        // complex path has `/{dir_name}/` in the path
        // name or path validity not checked
        let s = ProjectEnvironment::new(
            s.manifest_dir.join("examples/complex_examples"),
            "complex-examples",
        );
        assert!(!s.is_crate_root());
        let s = ProjectEnvironment::new(
            s.manifest_dir.join("benches/complex_benches"),
            "complex-benches",
        );
        assert!(!s.is_crate_root());
    }

    #[test]
    fn test_is_crate_name_in_dirs() {
        let (s, _dir) = test_project();
        let root = &s.manifest_dir;

        for dir_name in DIRS {
            for ty in ["simple", "complex"] {
                let s = ProjectEnvironment::new(root, format!("{ty}_{dir_name}"));
                assert!(s.is_crate_name_in_dirs(), "{s:?}");
            }
        }

        let s = ProjectEnvironment::new(root, "non_existant");
        assert!(!s.is_crate_name_in_dirs());
        let s = ProjectEnvironment::new(root.join("does-not-exist"), "foo_bar");
        assert!(!s.is_crate_name_in_dirs());
    }

    #[test]
    fn test_file_stem_eq() {
        let path = Path::new("/tmp/foo.rs");
        assert!(file_stem_eq(path, "foo"));
        assert!(!file_stem_eq(path, "tmp"));
        assert!(!file_stem_eq(path, "foo.rs"));
        assert!(!file_stem_eq(path, "fo"));
        assert!(!file_stem_eq(path, "f"));
        assert!(!file_stem_eq(path, ""));

        let path = Path::new("/tmp/foo/");
        assert!(file_stem_eq(path, "foo"));
        assert!(!file_stem_eq(path, "tmp"));
        assert!(!file_stem_eq(path, "fo"));
        assert!(!file_stem_eq(path, "f"));
        assert!(!file_stem_eq(path, ""));
    }

    // utils

    /// Creates:
    ///
    /// ```text
    /// - new_dir
    ///   - src
    ///     - main.rs
    ///   - {dir_name} for dir_name in DIRS
    ///     - simple_{dir_name}.rs
    ///     - complex_{dir_name}
    ///       - src if not "tests"
    ///         - main.rs
    ///         - module.rs
    /// ```
    fn test_project() -> (ProjectEnvironment, TempDir) {
        // change the prefix to one without the default `.` because it is not a valid crate name
        let dir = tempfile::Builder::new().prefix("tmp").tempdir().unwrap();
        let root = dir.path();
        let name = root.file_name().unwrap().to_str().unwrap();

        // No Cargo.toml, git
        fs::create_dir_all(root).unwrap();
        let src = root.join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main(){}").unwrap();

        for dir_name in DIRS {
            let new_dir = root.join(dir_name);
            fs::create_dir(&new_dir).unwrap();

            let simple = new_dir.join(format!("simple_{dir_name}.rs"));
            fs::write(simple, "").unwrap();

            let mut complex = new_dir.join(format!("complex_{dir_name}"));
            if dir_name != "tests" {
                fs::create_dir(&complex).unwrap();
                fs::write(complex.join("Cargo.toml"), "").unwrap();
                complex.push("src");
            }
            fs::create_dir(&complex).unwrap();
            fs::write(complex.join("main.rs"), "").unwrap();
            fs::write(complex.join("module.rs"), "").unwrap();
        }

        // create target dirs
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        fs::create_dir_all(target.join("tmp")).unwrap();

        (ProjectEnvironment::new(root, name), dir)
    }

    /// Writes a test manifest to `{root}/Cargo.toml`.
    fn write_manifest(s: &ProjectEnvironment, corebc: bool, dependencies: &[CorebcCrate]) {
        // use paths to avoid downloading dependencies
        const COREBC_CORE: &str = env!("CARGO_MANIFEST_DIR");
        let corebc_root = Path::new(COREBC_CORE).parent().unwrap();
        let mut dependencies_toml =
            String::with_capacity(150 * (corebc as usize + dependencies.len()));

        if corebc {
            let path = corebc_root.join("corebc");
            let corebc = format!("corebc = {{ path = \"{}\" }}\n", path.display());
            dependencies_toml.push_str(&corebc);
        }

        for dep in dependencies.iter() {
            let path = corebc_root.join(dep.fs_path());
            let dep = format!("{dep} = {{ path = \"{}\" }}\n", path.display());
            dependencies_toml.push_str(&dep);
        }

        let contents = format!(
            r#"
[package]
name = "{}"
version = "0.0.0"
edition = "2021"

[dependencies]
{dependencies_toml}
"#,
            s.crate_name.as_ref().unwrap()
        );
        fs::write(s.manifest_dir.join("Cargo.toml"), contents).unwrap();
    }
}
