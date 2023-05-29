use crate::{BlockindexError, Client, Result, H256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    hash: H256,
    previous_block_hash: H256,
    next_block_hash: H256,
    height: u64,
    confirmations: u64,
    size: u64,
    time: u64,
    nonce: String,
    difficulty: String,
    tx_count: u64,
}

/// Options for querying blocks
#[derive(Clone, Debug)]
pub enum BlockQueryOption {
    ByNumber(u64),
    ByHash(String),
}

impl Client {
    /// Returns given block.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use corebc_blockindex::block::BlockQueryOption;
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let block = client.get_block(BlockQueryOption::ByNumber(4483929)).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_block(
        &self,
        block_query_option: BlockQueryOption,
    ) -> Result<Block, BlockindexError> {
        let mut query_param:String;
        match block_query_option {
            BlockQueryOption::ByNumber(number) => query_param = format!("{number}"),
            BlockQueryOption::ByHash(block_hash) => query_param = block_hash,
        }
        let query =
            self.create_query("block", query_param.as_str(), HashMap::from([("details", "basic")]));
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        Ok(serde_json::from_value(response.clone()).unwrap())
    }
}
