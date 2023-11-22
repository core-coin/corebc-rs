//! Instantiate `Geth` with Clique enabled.

use corebc::{
    core::{rand::thread_rng, utils::GoCore},
    signers::LocalWallet,
};
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Generate a random clique signer and set it on Geth.
    let data_dir = tempfile::tempdir().expect("should be able to create temp geth datadir");
    let dir_path = data_dir.into_path();
    println!("Using {}", dir_path.display());

    // Create a random signer
    let key = LocalWallet::new(&mut thread_rng(), corebc::types::Network::Mainnet);

    let clique_key = key.signer().clone();
    let _geth = GoCore::new()
        // set the signer
        .set_clique_private_key(clique_key)
        // must always set the network id here
        .network_id(1)
        // set the datadir to a temp dir
        .data_dir(dir_path)
        // spawn it
        .spawn();

    Ok(())
}
