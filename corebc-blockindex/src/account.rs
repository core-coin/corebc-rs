use crate::{BlockindexError, Client, Result};
use corebc_core::abi::Address;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The raw response from the balance-related API endpoints
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account: Address,
    pub balance: String,
}

/// The raw response from the tokens list API endpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    #[serde(rename = "type")]
    pub token_type: String,
    pub name: String,
    pub contract: Address,
    pub transfers: u64,
    pub symbol: Option<String>,
    pub decimals: u64,
    pub balance: String,
}

/// The raw response from the balance history API endpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceHistory {
    pub time: u64,
    pub txs: u64,
    pub received: String,
    pub sent: String,
    pub sent_to_self: String,
}

/// The list sorting preference
/// Common optional arguments for the transaction or event list API endpoints
#[derive(Clone, Copy, Debug)]
pub struct TxListParams {
    pub page: u64,
    pub page_size: u64,
    pub from: u64,
    pub to: u64,
}

impl TxListParams {
    pub fn new(page: u64, page_size: u64, from: u64, to: u64) -> Self {
        Self { page, page_size, from, to }
    }
}
impl Default for TxListParams {
    fn default() -> Self {
        Self { from: 0, to: 99999999, page: 1, page_size: 1000 }
    }
}

impl From<TxListParams> for HashMap<&str, u64> {
    fn from(tx_params: TxListParams) -> Self {
        let mut params: HashMap<&str, u64> = HashMap::new();
        params.insert("page", tx_params.page);
        params.insert("pageSize", tx_params.page_size);
        params.insert("from", tx_params.from);
        params.insert("to", tx_params.to);
        params
    }
}

impl From<TxListParams> for HashMap<&str, String> {
    fn from(tx_params: TxListParams) -> Self {
        let mut params: HashMap<&str, String> = HashMap::new();
        params.insert("page", tx_params.page.to_string());
        params.insert("pageSize", tx_params.page_size.to_string());
        params.insert("from", tx_params.from.to_string());
        params.insert("to", tx_params.to.to_string());
        params
    }
}

/// Common optional arguments for the transaction or event list API endpoints
#[derive(Clone, Copy, Debug)]
pub struct BalanceHistoryParams {
    pub from: fn() -> u64,
    pub to: fn() -> u64,
    pub group_by: u64,
}
impl Default for BalanceHistoryParams {
    fn default() -> Self {
        let from = || -> u64 {
            let last_year = SystemTime::now() - Duration::from_secs(1) * 60 * 60 * 24 * 365;
            last_year.duration_since(UNIX_EPOCH).unwrap().as_secs()
        };
        let to = || -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() };
        Self { from, to, group_by: 3600 }
    }
}
impl From<BalanceHistoryParams> for HashMap<&str, u64> {
    fn from(balance_params: BalanceHistoryParams) -> Self {
        let mut params: HashMap<&str, u64> = HashMap::new();
        params.insert("from", (balance_params.from)());
        params.insert("to", (balance_params.to)());
        params.insert("groupBy", balance_params.group_by);
        params
    }
}

impl Client {
    /// Returns the Core balance of a given address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = "ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse()?;
    /// let balance = client.get_balance(&address).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_balance(&self, address: &Address) -> Result<AccountBalance> {
        let addr_str = format!("{address:?}");
        let query =
            self.create_query("address", addr_str.as_ref(), HashMap::from([("details", "basic")]));
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        Ok(AccountBalance {
            account: *address,
            balance: response["balance"].to_string().replace('\"', ""),
        })
    }

    /// Returns the list of transactions performed by an address, with optional pagination.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = "ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse()?;
    /// let transactions = client.get_transactions(&address, None).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_transactions(
        &self,
        address: &Address,
        params: Option<TxListParams>,
    ) -> Result<Vec<String>> {
        let addr_str = format!("{address:?}");
        let tx_params: HashMap<&str, u64> = params.unwrap_or_default().into();
        let query = self.create_query("address", addr_str.as_ref(), tx_params);
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        Ok(response["txids"]
            .as_array()
            .ok_or_else(|| BlockindexError::Builder("txids".to_string()))?
            .iter()
            .map(|x| x.to_string().replace('\"', ""))
            .collect())
    }

    /// Returns the list of tokens of an address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// 
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = &"ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse()?;
    /// let events = client.get_tokens(address, None).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_tokens(
        &self,
        address: &Address,
        params: Option<TxListParams>,
    ) -> Result<Vec<Token>> {
        let addr_str = format!("{address:?}");
        let mut tx_params: HashMap<&str, String> = params.unwrap_or_default().into();
        tx_params.insert("details", "tokenBalances".to_string());
        let query = self.create_query("address", addr_str.as_ref(), tx_params);
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        response["tokens"]
            .as_array()
            .ok_or_else(|| BlockindexError::Builder("tokens".to_string()))?
            .iter()
            .map(|x| {
                serde_json::from_value(x.to_owned())
                    .map_err(|_| BlockindexError::Builder("tokens".to_string()))
            })
            .collect()
    }

    /// Returns the balance history of an address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// 
    /// # async fn foo(client: corebc_blockindex::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let address = &"ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse()?;
    /// let history = client.get_balance_history(address, None).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_balance_history(
        &self,
        address: &Address,
        params: Option<BalanceHistoryParams>,
    ) -> Result<Vec<BalanceHistory>> {
        let addr_str = format!("{address:?}");
        let tx_params: HashMap<&str, u64> = params.unwrap_or_default().into();
        let query = self.create_query("balancehistory", addr_str.as_ref(), tx_params);
        let response: Value = self.get_json(&query).await?;
        if response["error"].as_str().is_some() {
            return Err(BlockindexError::ErrorResponse { error: response["error"].to_string() })
        }
        response
            .as_array()
            .ok_or_else(|| BlockindexError::BalanceFailed)?
            .iter()
            .map(|x| {
                serde_json::from_value(x.to_owned()).map_err(|_| BlockindexError::BalanceFailed)
            })
            .collect()
    }
}
