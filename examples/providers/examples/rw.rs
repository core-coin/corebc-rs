//! The RwClient wraps two data transports: the first is used for read operations, and the second
//! one is used for write operations, that consume gas like sending transactions.

use corebc::{prelude::*, utils::Shuttle};
use url::Url;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let shuttle = Shuttle::new().spawn();

    let http_url = Url::parse(&shuttle.endpoint())?;
    let http = Http::new(http_url);

    let ws = Ws::connect(shuttle.ws_endpoint()).await?;

    let _provider = Provider::rw(http, ws);

    Ok(())
}
