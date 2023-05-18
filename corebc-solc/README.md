# corebc-solc

Utilities for working with native `solc` and compiling projects.

To also compile contracts during `cargo build` (so that ethers `abigen!` can pull in updated abi automatically) you can configure a `corebc_solc::Project` in your `build.rs` file

First add `corebc-solc` to your cargo build-dependencies.

Once you compiled the project, you can configure cargo change detection with `rerun_if_sources_changed`, so that cargo will execute the `build.rs` file if a contract in the sources directory has changed

```toml
[build-dependencies]
corebc-solc = { git = "https://github.com/gakonst/ethers-rs" }
```

```rust
use corebc_solc::{Project, ProjectPathsConfig};

fn main() {
    // configure the project with all its paths, solc, cache etc.
    let project = Project::builder()
        .paths(ProjectPathsConfig::hardhat(env!("CARGO_MANIFEST_DIR")).unwrap())
        .build()
        .unwrap();
    let output = project.compile().unwrap();

    // Tell Cargo that if a source file changes, to rerun this build script.
    project.rerun_if_sources_changed();
}
```
