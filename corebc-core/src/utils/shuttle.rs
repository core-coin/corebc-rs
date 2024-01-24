use crate::{
    types::{Address, Network},
    utils::{secret_key_to_address, unused_ports},
};
use libgoldilocks::{SecretKey as LibgoldilocksSecretKey, SigningKey};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command},
    time::{Duration, Instant},
};

/// How long we will wait for shuttle to indicate that it is ready.
const SHUTTLE_STARTUP_TIMEOUT_MILLIS: u64 = 10_000;

/// An shuttle CLI instance. Will close the instance when dropped.
///
/// Construct this using [`Shuttle`](crate::utils::Shuttle)
pub struct ShuttleInstance {
    pid: Child,
    private_keys: Vec<LibgoldilocksSecretKey>,
    addresses: Vec<Address>,
    port: u16,
    network_id: Option<u64>,
}

impl ShuttleInstance {
    /// Returns the private keys used to instantiate this instance
    pub fn keys(&self) -> &[LibgoldilocksSecretKey] {
        &self.private_keys
    }

    /// Returns the addresses used to instantiate this instance
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }

    /// Returns the port of this instance
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the network of the shuttle instance
    //  CORETODO: Should be a local node instead of Devin
    pub fn network_id(&self) -> u64 {
        self.network_id.unwrap_or_else(|| Network::Devin.into())
    }

    /// Returns the HTTP endpoint of this instance
    pub fn endpoint(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Returns the Websocket endpoint of this instance
    pub fn ws_endpoint(&self) -> String {
        format!("ws://localhost:{}", self.port)
    }
}

impl Drop for ShuttleInstance {
    fn drop(&mut self) {
        self.pid.kill().expect("could not kill shuttle");
    }
}

/// Builder for launching `shuttle`.
///
/// # Panics
///
/// If `spawn` is called without `shuttle` being available in the user's $PATH
///
/// # Example
///
/// ```no_run
/// use corebc_core::utils::Shuttle;
///
/// let port = 8545u16;
/// let url = format!("http://localhost:{}", port).to_string();
///
/// let shuttle = Shuttle::new()
///     .port(port)
///     .mnemonic("abstract vacuum mammal awkward pudding scene penalty purchase dinner depart evoke puzzle")
///     .spawn();
///
/// drop(shuttle); // this will kill the instance
/// ```
#[derive(Debug, Clone, Default)]
#[must_use = "This Builder struct does nothing unless it is `spawn`ed"]
pub struct Shuttle {
    program: Option<PathBuf>,
    port: Option<u16>,
    block_time: Option<u64>,
    network_id: Option<u64>,
    mnemonic: Option<String>,
    fork: Option<String>,
    fork_block_number: Option<u64>,
    args: Vec<String>,
    timeout: Option<u64>,
}

impl Shuttle {
    /// Creates an empty Shuttle builder.
    /// The default port is 8545. The mnemonic is chosen randomly.
    ///
    /// # Example
    ///
    /// ```
    /// # use corebc_core::utils::Shuttle;
    /// fn a() {
    ///  let shuttle = Shuttle::default().spawn();
    ///
    ///  println!("Shuttle running at `{}`", shuttle.endpoint());
    /// # }
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an Shuttle builder which will execute `shuttle` at the given path.
    ///
    /// # Example
    ///
    /// ```
    /// # use corebc_core::utils::Shuttle;
    /// fn a() {
    ///  let shuttle = Shuttle::at("~/.foundry/bin/shuttle").spawn();
    ///
    ///  println!("Shuttle running at `{}`", shuttle.endpoint());
    /// # }
    /// ```
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self::new().path(path)
    }

    /// Sets the `path` to the `shuttle` cli
    ///
    /// By default, it's expected that `shuttle` is in `$PATH`, see also
    /// [`std::process::Command::new()`]
    pub fn path<T: Into<PathBuf>>(mut self, path: T) -> Self {
        self.program = Some(path.into());
        self
    }

    /// Sets the port which will be used when the `shuttle` instance is launched.
    pub fn port<T: Into<u16>>(mut self, port: T) -> Self {
        self.port = Some(port.into());
        self
    }

    /// Sets the network_id the `shuttle` instance will use.
    pub fn network_id<T: Into<u64>>(mut self, network_id: T) -> Self {
        self.network_id = Some(network_id.into());
        self
    }

    /// Sets the mnemonic which will be used when the `shuttle` instance is launched.
    pub fn mnemonic<T: Into<String>>(mut self, mnemonic: T) -> Self {
        self.mnemonic = Some(mnemonic.into());
        self
    }

    /// Sets the block-time in seconds which will be used when the `shuttle` instance is launched.
    pub fn block_time<T: Into<u64>>(mut self, block_time: T) -> Self {
        self.block_time = Some(block_time.into());
        self
    }

    /// Sets the `fork-block-number` which will be used in addition to [`Self::fork`].
    ///
    /// **Note:** if set, then this requires `fork` to be set as well
    pub fn fork_block_number<T: Into<u64>>(mut self, fork_block_number: T) -> Self {
        self.fork_block_number = Some(fork_block_number.into());
        self
    }

    /// Sets the `fork` argument to fork from another currently running Ethereum client
    /// at a given block. Input should be the HTTP location and port of the other client,
    /// e.g. `http://localhost:8545`. You can optionally specify the block to fork from
    /// using an @ sign: `http://localhost:8545@1599200`
    pub fn fork<T: Into<String>>(mut self, fork: T) -> Self {
        self.fork = Some(fork.into());
        self
    }

    /// Adds an argument to pass to the `shuttle`.
    pub fn arg<T: Into<String>>(mut self, arg: T) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to pass to the `shuttle`.
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

    /// Sets the timeout which will be used when the `shuttle` instance is launched.
    pub fn timeout<T: Into<u64>>(mut self, timeout: T) -> Self {
        self.timeout = Some(timeout.into());
        self
    }

    /// Consumes the builder and spawns `shuttle`.
    ///
    /// # Panics
    ///
    /// If spawning the instance fails at any point.
    #[track_caller]
    pub fn spawn(self) -> ShuttleInstance {
        let mut cmd = if let Some(ref prg) = self.program {
            Command::new(prg)
        } else {
            Command::new("shuttle")
        };
        cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::inherit());
        let port = if let Some(port) = self.port { port } else { unused_ports::<1>()[0] };
        cmd.arg("-p").arg(port.to_string());

        if let Some(mnemonic) = self.mnemonic {
            cmd.arg("-m").arg(mnemonic);
        }
        let network: Network;

        if let Some(network_id) = self.network_id {
            cmd.arg("--network-id").arg(network_id.to_string());

            match network_id {
                1 => network = Network::Mainnet,
                3 => network = Network::Devin,
                n => network = Network::Private(n),
            }
        } else {
            network = Network::Devin;
        }

        if let Some(block_time) = self.block_time {
            cmd.arg("-b").arg(block_time.to_string());
        }

        if let Some(fork) = self.fork {
            cmd.arg("-f").arg(fork);
        }

        if let Some(fork_block_number) = self.fork_block_number {
            cmd.arg("--fork-block-number").arg(fork_block_number.to_string());
        }

        cmd.args(self.args);

        let mut child = cmd.spawn().expect("couldnt start shuttle");

        let stdout = child.stdout.take().expect("Unable to get stdout for shuttle child process");

        let start = Instant::now();
        let mut reader = BufReader::new(stdout);

        let mut private_keys = Vec::new();
        let mut addresses = Vec::new();
        let mut is_private_key = false;
        loop {
            if start + Duration::from_millis(self.timeout.unwrap_or(SHUTTLE_STARTUP_TIMEOUT_MILLIS)) <=
                Instant::now()
            {
                panic!("Timed out waiting for shuttle to start. Is shuttle installed?")
            }

            let mut line = String::new();
            reader.read_line(&mut line).expect("Failed to read line from shuttle process");
            if line.contains("Listening on") {
                break
            }

            if line.starts_with("Private Keys") {
                is_private_key = true;
            }

            if is_private_key && line.starts_with('(') {
                let key_str = &line[6..line.len() - 1];
                let key = SigningKey::from_str(key_str);
                // CORETODO: Attention, must work but take care
                // let key_hex = hex::decode(key_str).expect("could not parse as hex");
                // let key = K256SecretKey::from_bytes(&GenericArray::clone_from_slice(&key_hex))
                //     .expect("did not get private key");
                addresses.push(secret_key_to_address(&key, &network));
                private_keys.push(*key.secret_key());
            }
        }

        ShuttleInstance { pid: child, private_keys, addresses, port, network_id: self.network_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_launch_shuttle() {
        let _ = Shuttle::new().spawn();
    }
}
