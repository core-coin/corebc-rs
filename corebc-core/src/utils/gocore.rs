use super::{unused_ports, CliqueConfig, Genesis};
use crate::{
    types::{Bytes, Network, H256},
    utils::secret_key_to_address,
};
use k256::ecdsa::SigningKey;
use std::{
    fs::{create_dir, File},
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, ChildStderr, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::tempdir;

/// How long we will wait for gocore to indicate that it is ready.
const GOCORE_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for waiting for gocore to add a peer.
const GOCORE_DIAL_LOOP_TIMEOUT: Duration = Duration::from_secs(20);

/// The exposed APIs
const API: &str = "xcb,net,web3,txpool,admin,personal,miner,debug";

/// The gocore command
const GOCORE: &str = "gocore";

/// Errors that can occur when working with the [`GocoreInstance`].
#[derive(Debug)]
pub enum GoCoreInstanceError {
    /// Timed out waiting for a message from gocore's stderr.
    Timeout(String),

    /// A line could not be read from the gocore stderr.
    ReadLineError(std::io::Error),

    /// The child gocore process's stderr was not captured.
    NoStderr,
}

/// A gocore instance. Will close the instance when dropped.
///
/// Construct this using [`Gocore`](crate::utils::GoCore).
#[derive(Debug)]
pub struct GoCoreInstance {
    pid: Child,
    port: u16,
    ipc: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    p2p_port: Option<u16>,
    genesis: Option<Genesis>,
    clique_private_key: Option<SigningKey>,
}

impl GoCoreInstance {
    /// Returns the port of this instance
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the p2p port of this instance
    pub fn p2p_port(&self) -> Option<u16> {
        self.p2p_port
    }

    /// Returns the HTTP endpoint of this instance
    pub fn endpoint(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Returns the Websocket endpoint of this instance
    pub fn ws_endpoint(&self) -> String {
        format!("ws://localhost:{}", self.port)
    }

    /// Returns the path to this instances' IPC socket
    pub fn ipc_path(&self) -> &Option<PathBuf> {
        &self.ipc
    }

    /// Returns the path to this instances' data directory
    pub fn data_dir(&self) -> &Option<PathBuf> {
        &self.data_dir
    }

    /// Returns the genesis configuration used to configure this instance
    pub fn genesis(&self) -> &Option<Genesis> {
        &self.genesis
    }

    /// Returns the private key used to configure clique on this instance
    pub fn clique_private_key(&self) -> &Option<SigningKey> {
        &self.clique_private_key
    }

    /// Takes the stderr contained in the child process.
    ///
    /// This leaves a `None` in its place, so calling methods that require a stderr to be present
    /// will fail if called after this.
    pub fn stderr(&mut self) -> Result<ChildStderr, GoCoreInstanceError> {
        self.pid.stderr.take().ok_or(GoCoreInstanceError::NoStderr)
    }

    /// Blocks until gocore adds the specified peer, using 20s as the timeout.
    ///
    /// Requires the stderr to be present in the `GoCoreInstance`.
    pub fn wait_to_add_peer(&mut self, id: H256) -> Result<(), GoCoreInstanceError> {
        let mut stderr = self.pid.stderr.as_mut().ok_or(GoCoreInstanceError::NoStderr)?;
        let mut err_reader = BufReader::new(&mut stderr);
        let mut line = String::new();
        let start = Instant::now();

        while start.elapsed() < GOCORE_DIAL_LOOP_TIMEOUT {
            line.clear();
            err_reader.read_line(&mut line).map_err(GoCoreInstanceError::ReadLineError)?;

            // gocore ids are trunated
            let truncated_id = hex::encode(&id.0[..8]);
            if line.contains("Adding p2p peer") && line.contains(&truncated_id) {
                return Ok(())
            }
        }
        Err(GoCoreInstanceError::Timeout("Timed out waiting for gocore to add a peer".into()))
    }
}

impl Drop for GoCoreInstance {
    fn drop(&mut self) {
        self.pid.kill().expect("could not kill gocore");
    }
}

/// Whether or not gocore is in `dev` mode and configuration options that depend on the mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoCoreMode {
    /// Options that can be set in dev mode
    Dev(DevOptions),
    /// Options that cannot be set in dev mode
    NonDev(PrivateNetOptions),
}

impl Default for GoCoreMode {
    fn default() -> Self {
        Self::Dev(Default::default())
    }
}

/// Configuration options that can be set in dev mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DevOptions {
    /// The interval at which the dev network will mine new blocks.
    pub block_time: Option<u64>,
}

/// Configuration options that cannot be set in dev mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivateNetOptions {
    /// The p2p port to use.
    pub p2p_port: Option<u16>,

    /// Whether or not peer discovery is enabled.
    pub discovery: bool,
}

impl Default for PrivateNetOptions {
    fn default() -> Self {
        Self { p2p_port: None, discovery: true }
    }
}

/// Builder for launching `gocore`.
///
/// # Panics
///
/// If `spawn` is called without `gocore` being available in the user's $PATH
///
/// # Example
///
/// ```no_run
/// use corebc_core::utils::GoCore;
///
/// let port = 8545u16;
/// let url = format!("http://localhost:{}", port).to_string();
///
/// let gocore = GoCore::new()
///     .port(port)
///     .block_time(5000u64)
///     .spawn();
///
/// drop(gocore); // this will kill the instance
/// ```
#[derive(Clone, Debug, Default)]
#[must_use = "This Builder struct does nothing unless it is `spawn`ed"]
pub struct GoCore {
    program: Option<PathBuf>,
    port: Option<u16>,
    ipc_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    network_id: Option<u64>,
    insecure_unlock: bool,
    genesis: Option<Genesis>,
    mode: GoCoreMode,
    clique_private_key: Option<SigningKey>,
}

impl GoCore {
    /// Creates an empty GoCore builder.
    ///
    /// The mnemonic is chosen randomly.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a GoCore builder which will execute `gocore` at the given path.
    ///
    /// # Example
    ///
    /// ```
    /// use corebc_core::utils::GoCore;
    /// # fn a() {
    ///  let gocore = GoCore::at("../go-ethereum/build/bin/gocore").spawn();
    ///
    ///  println!("GoCore running at `{}`", gocore.endpoint());
    /// # }
    /// ```
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self::new().path(path)
    }

    /// Returns whether the node is launched in Clique consensus mode
    pub fn is_clique(&self) -> bool {
        self.clique_private_key.is_some()
    }

    /// Sets the `path` to the `gocore` executable
    ///
    /// By default, it's expected that `gocore` is in `$PATH`, see also
    /// [`std::process::Command::new()`]
    pub fn path<T: Into<PathBuf>>(mut self, path: T) -> Self {
        self.program = Some(path.into());
        self
    }

    /// Sets the Clique Private Key to the `gocore` executable, which will be later
    /// loaded on the node.
    ///
    /// The address derived from this private key will be used to set the `miner.etherbase` field
    /// on the node.
    pub fn set_clique_private_key<T: Into<SigningKey>>(mut self, private_key: T) -> Self {
        self.clique_private_key = Some(private_key.into());
        self
    }

    /// Sets the port which will be used when the `gocore-cli` instance is launched.
    pub fn port<T: Into<u16>>(mut self, port: T) -> Self {
        self.port = Some(port.into());
        self
    }

    /// Sets the port which will be used for incoming p2p connections.
    ///
    /// This will put the gocore instance into non-dev mode, discarding any previously set dev-mode
    /// options.
    pub fn p2p_port(mut self, port: u16) -> Self {
        match self.mode {
            GoCoreMode::Dev(_) => {
                self.mode = GoCoreMode::NonDev(PrivateNetOptions {
                    p2p_port: Some(port),
                    ..Default::default()
                })
            }
            GoCoreMode::NonDev(ref mut opts) => opts.p2p_port = Some(port),
        }
        self
    }

    /// Sets the block-time which will be used when the `gocore-cli` instance is launched.
    ///
    /// This will put the gocore instance in `dev` mode, discarding any previously set options that
    /// cannot be used in dev mode.
    pub fn block_time<T: Into<u64>>(mut self, block_time: T) -> Self {
        self.mode = GoCoreMode::Dev(DevOptions { block_time: Some(block_time.into()) });
        self
    }

    /// Sets the network id for the gocore instance.
    pub fn network_id<T: Into<u64>>(mut self, network_id: T) -> Self {
        self.network_id = Some(network_id.into());
        self
    }

    /// Allow gocore to unlock accounts when rpc apis are open.
    pub fn insecure_unlock(mut self) -> Self {
        self.insecure_unlock = true;
        self
    }

    /// Disable discovery for the gocore instance.
    ///
    /// This will put the gocore instance into non-dev mode, discarding any previously set dev-mode
    /// options.
    pub fn disable_discovery(mut self) -> Self {
        self.inner_disable_discovery();
        self
    }

    fn inner_disable_discovery(&mut self) {
        match self.mode {
            GoCoreMode::Dev(_) => {
                self.mode =
                    GoCoreMode::NonDev(PrivateNetOptions { discovery: false, ..Default::default() })
            }
            GoCoreMode::NonDev(ref mut opts) => opts.discovery = false,
        }
    }

    /// Manually sets the IPC path for the socket manually.
    pub fn ipc_path<T: Into<PathBuf>>(mut self, path: T) -> Self {
        self.ipc_path = Some(path.into());
        self
    }

    /// Sets the data directory for gocore.
    pub fn data_dir<T: Into<PathBuf>>(mut self, path: T) -> Self {
        self.data_dir = Some(path.into());
        self
    }

    /// Sets the `genesis.json` for the gocore instance.
    ///
    /// If this is set, gocore will be initialized with `gocore init` and the `--datadir` option
    /// will be set to the same value as `data_dir`.
    ///
    /// This is destructive and will overwrite any existing data in the data directory.
    pub fn genesis(mut self, genesis: Genesis) -> Self {
        self.genesis = Some(genesis);
        self
    }

    /// Consumes the builder and spawns `gocore`.
    ///
    /// # Panics
    ///
    /// If spawning the instance fails at any point.
    #[track_caller]
    pub fn spawn(mut self) -> GoCoreInstance {
        let bin_path = match self.program.as_ref() {
            Some(bin) => bin.as_os_str(),
            None => GOCORE.as_ref(),
        }
        .to_os_string();
        let mut cmd = Command::new(&bin_path);
        // gocore uses stderr for its logs
        cmd.stderr(Stdio::piped());

        let mut unused_ports = unused_ports::<3>().into_iter();
        let mut unused_port = || unused_ports.next().unwrap();

        let port = self.port.unwrap_or_else(&mut unused_port);
        let port_s = port.to_string();

        // Open the HTTP API
        cmd.arg("--http");
        cmd.arg("--http.port").arg(&port_s);
        cmd.arg("--http.api").arg(API);

        // Open the WS API
        cmd.arg("--ws");
        cmd.arg("--ws.port").arg(port_s);
        cmd.arg("--ws.api").arg(API);

        let network: Network;
        // pass insecure unlock flag if set
        let is_clique = self.is_clique();
        if self.insecure_unlock || is_clique {
            cmd.arg("--allow-insecure-unlock");
        }

        if is_clique {
            self.inner_disable_discovery();
        }

        // use gocore init to initialize the datadir if the genesis exists
        if is_clique {
            network = Network::Devin;
            if let Some(genesis) = &mut self.genesis {
                // set up a clique config with an instant sealing period and short (8 block) epoch
                let clique_config = CliqueConfig { period: Some(0), epoch: Some(8) };
                genesis.config.clique = Some(clique_config);

                let clique_addr = secret_key_to_address(
                    self.clique_private_key.as_ref().expect("is_clique == true"),
                    &network,
                );

                // set the extraData field
                let extra_data_bytes =
                    [&[0u8; 32][..], clique_addr.as_ref(), &[0u8; 65][..]].concat();
                let extra_data = Bytes::from(extra_data_bytes);
                genesis.extra_data = extra_data;

                // we must set the etherbase if using clique
                // need to use format! / Debug here because the Address Display impl doesn't show
                // the entire address
                cmd.arg("--miner.etherbase").arg(format!("{clique_addr:?}"));
            }

            let clique_addr = secret_key_to_address(
                self.clique_private_key.as_ref().expect("is_clique == true"),
                &network,
            );

            self.genesis = Some(Genesis::new(
                self.network_id.expect("network id must be set in clique mode"),
                clique_addr,
            ));

            // we must set the etherbase if using clique
            // need to use format! / Debug here because the Address Display impl doesn't show the
            // entire address
            cmd.arg("--miner.etherbase").arg(format!("{clique_addr:?}"));
        }

        if let Some(ref genesis) = self.genesis {
            // create a temp dir to store the genesis file
            let temp_genesis_dir_path =
                tempdir().expect("should be able to create temp dir for genesis init").into_path();

            // create a temp dir to store the genesis file
            let temp_genesis_path = temp_genesis_dir_path.join("genesis.json");

            // create the genesis file
            let mut file = File::create(&temp_genesis_path).expect("could not create genesis file");

            // serialize genesis and write to file
            serde_json::to_writer_pretty(&mut file, &genesis)
                .expect("could not write genesis to file");

            let mut init_cmd = Command::new(bin_path);
            if let Some(ref data_dir) = self.data_dir {
                init_cmd.arg("--datadir").arg(data_dir);
            }

            // set the stderr to null so we don't pollute the test output
            init_cmd.stderr(Stdio::null());

            init_cmd.arg("init").arg(temp_genesis_path);
            let res = init_cmd
                .spawn()
                .expect("failed to spawn gocore init")
                .wait()
                .expect("failed to wait for gocore init to exit");
            if !res.success() {
                panic!("gocore init failed");
            }

            // clean up the temp dir which is now persisted
            std::fs::remove_dir_all(temp_genesis_dir_path)
                .expect("could not remove genesis temp dir");
        }

        if let Some(ref data_dir) = self.data_dir {
            cmd.arg("--datadir").arg(data_dir);

            // create the directory if it doesn't exist
            if !data_dir.exists() {
                create_dir(data_dir).expect("could not create data dir");
            }
        }

        // Dev mode with custom block time
        let p2p_port = match self.mode {
            GoCoreMode::Dev(DevOptions { block_time }) => {
                cmd.arg("--dev");
                if let Some(block_time) = block_time {
                    cmd.arg("--dev.period").arg(block_time.to_string());
                }
                None
            }
            GoCoreMode::NonDev(PrivateNetOptions { p2p_port, discovery }) => {
                let port = p2p_port.unwrap_or_else(unused_port);
                cmd.arg("--port").arg(port.to_string());

                // disable discovery if the flag is set
                if !discovery {
                    cmd.arg("--nodiscover");
                }
                Some(port)
            }
        };

        if let Some(network_id) = self.network_id {
            cmd.arg("--networkid").arg(network_id.to_string());
        }

        // debug verbosity is needed to check when peers are added
        cmd.arg("--verbosity").arg("4");

        if let Some(ref ipc) = self.ipc_path {
            cmd.arg("--ipcpath").arg(ipc);
        }

        let mut child = cmd.spawn().expect("couldnt start gocore");

        let stderr = child.stderr.expect("Unable to get stderr for gocore child process");

        let start = Instant::now();
        let mut reader = BufReader::new(stderr);

        // we shouldn't need to wait for p2p to start if gocore is in dev mode - p2p is disabled in
        // dev mode
        let mut p2p_started = matches!(self.mode, GoCoreMode::Dev(_));
        let mut http_started = false;

        loop {
            if start + GOCORE_STARTUP_TIMEOUT <= Instant::now() {
                panic!("Timed out waiting for gocore to start. Is gocore installed?")
            }

            let mut line = String::with_capacity(120);
            reader.read_line(&mut line).expect("Failed to read line from gocore process");

            if matches!(self.mode, GoCoreMode::NonDev(_)) && line.contains("Started P2P networking")
            {
                p2p_started = true;
            }

            // gocore 1.9.23 uses "server started" while 1.9.18 uses "endpoint opened"
            // the unauthenticated api is used for regular non-engine API requests
            if line.contains("HTTP endpoint opened") ||
                (line.contains("HTTP server started") && !line.contains("auth=true"))
            {
                http_started = true;
            }

            if p2p_started && http_started {
                break
            }
        }

        child.stderr = Some(reader.into_inner());

        GoCoreInstance {
            pid: child,
            port,
            ipc: self.ipc_path,
            data_dir: self.data_dir,
            p2p_port,
            genesis: self.genesis,
            clique_private_key: self.clique_private_key,
        }
    }
}

// These tests should use a different datadir for each `GoCore` spawned
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Allows running tests with a temporary directory, which is cleaned up after the function is
    /// called.
    ///
    /// Helps with tests that spawn a helper instance, which has to be dropped before the temporary
    /// directory is cleaned up.
    #[track_caller]
    fn run_with_tempdir(f: impl Fn(&Path)) {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();
        f(temp_dir_path);
        #[cfg(not(windows))]
        temp_dir.close().unwrap();
    }

    #[test]
    fn p2p_port() {
        run_with_tempdir(|temp_dir_path| {
            let gocore = GoCore::new().disable_discovery().data_dir(temp_dir_path).spawn();
            let p2p_port = gocore.p2p_port();
            assert!(p2p_port.is_some());
        });
    }

    #[test]
    fn explicit_p2p_port() {
        run_with_tempdir(|temp_dir_path| {
            // if a p2p port is explicitly set, it should be used
            let gocore = GoCore::new().p2p_port(1234).data_dir(temp_dir_path).spawn();
            let p2p_port = gocore.p2p_port();
            assert_eq!(p2p_port, Some(1234));
        });
    }

    #[test]
    fn dev_mode() {
        run_with_tempdir(|temp_dir_path| {
            // dev mode should not have a p2p port, and dev should be the default
            let gocore = GoCore::new().data_dir(temp_dir_path).spawn();
            let p2p_port = gocore.p2p_port();
            assert!(p2p_port.is_none());
        })
    }
}