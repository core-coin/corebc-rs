//! Spawn an [shuttle](https://github.com/foundry-rs/foundry/tree/master/shuttle) instance in forking mode

use corebc::utils::Shuttle;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // ensure `shuttle` is available in $PATH
    let shuttle = Shuttle::new().fork("https://eth.llamarpc.com").spawn();

    println!("Shuttle running at `{}`", shuttle.endpoint());

    Ok(())
}
