use crate::{BlockindexError, Client, Result};
use corebc_core::types::H256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub txid: String,
    pub block_hash: String,
    pub block_height: u64,
    pub confirmations: u64,
    pub block_time: u64,
    pub value: String,
    pub fees: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub status: u64,
    #[serde(default)]
    pub nonce: u64,
    #[serde(default)]
    pub energy_limit: u64,
    #[serde(default)]
    pub energy_used: u64,
    #[serde(default)]
    pub energy_price: String,
    #[serde(default)]
    pub data: String,
}

impl Client {
    /// Returns given transaction.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let tx_hash = "0x9a0516515962331000ab0910b969b94cc63e3254ee36664595085af07815fa31".parse()?;
    /// let tx = client.get_transaction(tx_hash).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_transaction(&self, hash: H256) -> Result<Transaction> {
        let hash = format!("{hash:?}");
        let query = self.create_query("tx", hash.as_ref(), HashMap::from([("details", "basic")]));
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        let mut res: Transaction = serde_json::from_value(response.clone())?;
        res.from = response["vin"][0]["addresses"][0].to_string().replace('\"', "");
        res.to = response["vout"][0]["addresses"][0].to_string().replace('\"', "");
        res.status = response["ethereumSpecific"]["status"]
            .as_u64()
            .ok_or_else(|| BlockindexError::Builder("status".to_string()))?;
        res.nonce = response["ethereumSpecific"]["nonce"]
            .as_u64()
            .ok_or_else(|| BlockindexError::Builder("nonce".to_string()))?;
        res.energy_limit = response["ethereumSpecific"]["energyLimit"]
            .as_u64()
            .ok_or_else(|| BlockindexError::Builder("energyLimit".to_string()))?;
        res.energy_used = response["ethereumSpecific"]["energyUsed"]
            .as_u64()
            .ok_or_else(|| BlockindexError::Builder("energyUsed".to_string()))?;
        res.energy_price =
            response["ethereumSpecific"]["energyPrice"].to_string().replace('\"', "");
        res.data = response["ethereumSpecific"]["data"].to_string().replace('\"', "");

        Ok(res)
    }

    /// Sends raw signed transaction.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// 
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let events = client.send_raw_transaction("some signed transaction".to_string()).await?;
    /// # Ok(()) }
    /// ```
    pub async fn send_raw_transaction(&self, tx: String) -> Result<H256> {
        let query = self.create_query("sendtx", tx.as_ref(), HashMap::from([("details", "basic")]));
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        response["result"]
            .to_string()
            .replace('\"', "")
            .parse()
            .map_err(|_| BlockindexError::Builder("result".to_string()))
    }
}
