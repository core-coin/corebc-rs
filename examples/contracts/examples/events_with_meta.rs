use corebc::{
    contract::abigen,
    core::types::Address,
    providers::{Provider, StreamExt, Ws},
};
use eyre::Result;
use std::sync::Arc;

// Generate the type-safe contract bindings by providing the ABI
// definition in human readable format
abigen!(
    ERC20,
    r#"[
        event  Transfer(address indexed src, address indexed dst, uint wad)
    ]"#,
);

#[tokio::main]
async fn main() -> Result<()> {
    let client =
        Provider::<Ws>::connect("wss://mainnet.infura.io/ws/v3/c60b0bb42f8a4c6481ecd229eddaca27")
            .await?;

    let client = Arc::new(client);

    // WETH Token
    let address = "0x0000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".parse::<Address>()?;
    let weth = ERC20::new(address, Arc::clone(&client));

    // Subscribe Transfer events
    let events = weth.events().from_block(16232698);
    let mut stream = events.stream().await?.with_meta().take(1);
    while let Some(Ok((event, meta))) = stream.next().await {
        println!("src: {:?}, dst: {:?}, wad: {:?}", event.src, event.dst, event.wad);

        println!(
            r#"address: {:?}, 
               block_number: {:?}, 
               block_hash: {:?}, 
               transaction_hash: {:?}, 
               transaction_index: {:?}, 
               log_index: {:?}
            "#,
            meta.address,
            meta.block_number,
            meta.block_hash,
            meta.transaction_hash,
            meta.transaction_index,
            meta.log_index
        );
    }

    Ok(())
}
