use corebc::{
    core::{types::TransactionRequest, utils::Shuttle},
    providers::{Http, Middleware, Provider},
};
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // fork mainnet
    let shuttle = Shuttle::new().fork("https://eth.llamarpc.com").spawn();
    let from = shuttle.addresses()[0];
    // connect to the network
    let provider = Provider::<Http>::try_from(shuttle.endpoint()).unwrap().with_sender(from);

    // craft the transaction
    let tx = TransactionRequest::new().to("vitalik.eth").value(100_000);

    // send it!
    let receipt = provider
        .send_transaction(tx, None)
        .await?
        .await?
        .ok_or_else(|| eyre::format_err!("tx dropped from mempool"))?;
    let tx = provider.get_transaction(receipt.transaction_hash).await?;

    println!("{}", serde_json::to_string(&tx)?);
    println!("{}", serde_json::to_string(&receipt)?);

    Ok(())
}
