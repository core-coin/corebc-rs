// Modified from <https://github.com/tomusdrw/rust-web3/blob/master/src/types/block.rs>

use crate::types::{Address, Bloom, Bytes, Transaction, TxHash, H256, U256, U64};
use chrono::{DateTime, TimeZone, Utc};
use serde::{
    de::{MapAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{fmt, fmt::Formatter, str::FromStr};
use thiserror::Error;

/// The block type returned from RPC calls.
///
/// This is generic over a `TX` type which will be either the hash or the full transaction,
/// i.e. `Block<TxHash>` or `Block<Transaction>`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Block<TX> {
    /// Hash of the block
    pub hash: Option<H256>,
    /// Hash of the parent
    #[serde(default, rename = "parentHash")]
    pub parent_hash: H256,
    /// Hash of the uncles
    #[serde(default, rename = "sha3Uncles")]
    pub uncles_hash: H256,
    /// Miner/author's address. None if pending.
    #[serde(default, rename = "miner")]
    pub author: Option<Address>,
    /// State root hash
    #[serde(default, rename = "stateRoot")]
    pub state_root: H256,
    /// Transactions root hash
    #[serde(default, rename = "transactionsRoot")]
    pub transactions_root: H256,
    /// Transactions receipts root hash
    #[serde(default, rename = "receiptsRoot")]
    pub receipts_root: H256,
    /// Block number. None if pending.
    pub number: Option<U64>,
    /// Gas Used
    #[serde(default, rename = "energyUsed")]
    pub energy_used: U256,
    /// Gas Limit
    #[serde(default, rename = "energyLimit")]
    pub energy_limit: U256,
    /// Extra data
    #[serde(default, rename = "extraData")]
    pub extra_data: Bytes,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Option<Bloom>,
    /// Timestamp
    #[serde(default)]
    pub timestamp: U256,
    /// Difficulty
    #[serde(default)]
    pub difficulty: U256,
    /// Total difficulty
    #[serde(rename = "totalDifficulty")]
    pub total_difficulty: Option<U256>,
    /// Seal fields
    #[serde(default, rename = "sealFields", deserialize_with = "deserialize_null_default")]
    pub seal_fields: Vec<Bytes>,
    /// Uncles' hashes
    #[serde(default)]
    pub uncles: Vec<H256>,
    /// Transactions
    #[serde(bound = "TX: Serialize + serde::de::DeserializeOwned", default)]
    pub transactions: Vec<TX>,
    /// Size in bytes
    pub size: Option<U256>,
    /// Mix Hash
    #[serde(rename = "mixHash")]
    pub mix_hash: Option<H256>,
    /// Nonce
    pub nonce: Option<crate::types::H64>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Error returned by [`Block::time`].
#[derive(Clone, Copy, Debug, Error)]
pub enum TimeError {
    /// Timestamp is zero.
    #[error("timestamp is zero")]
    TimestampZero,

    /// Timestamp is too large for [`DateTime<Utc>`].
    #[error("timestamp is too large")]
    TimestampOverflow,
}

impl<TX> Block<TX> {
    /// Parse [`Self::timestamp`] into a [`DateTime<Utc>`].
    ///
    /// # Errors
    ///
    /// * [`TimeError::TimestampZero`] if the timestamp is zero, or
    /// * [`TimeError::TimestampOverflow`] if the timestamp is too large to be represented as a
    ///   [`DateTime<Utc>`].
    pub fn time(&self) -> Result<DateTime<Utc>, TimeError> {
        if self.timestamp.is_zero() {
            return Err(TimeError::TimestampZero)
        }
        if self.timestamp.bits() > 63 {
            return Err(TimeError::TimestampOverflow)
        }
        // Casting to i64 is safe because the timestamp is guaranteed to be less than 2^63.
        // TODO: It would be nice if there was `TryInto<i64> for U256`.
        let secs = self.timestamp.as_u64() as i64;
        Ok(Utc.timestamp_opt(secs, 0).unwrap())
    }
}

impl Block<TxHash> {
    /// Converts this block that only holds transaction hashes into a full block with `Transaction`
    pub fn into_full_block(self, transactions: Vec<Transaction>) -> Block<Transaction> {
        {
            let Block {
                hash,
                parent_hash,
                uncles_hash,
                author,
                state_root,
                transactions_root,
                receipts_root,
                number,
                energy_used,
                energy_limit,
                extra_data,
                logs_bloom,
                timestamp,
                difficulty,
                total_difficulty,
                seal_fields,
                uncles,
                size,
                mix_hash,
                nonce,
                ..
            } = self;
            Block {
                hash,
                parent_hash,
                uncles_hash,
                author,
                state_root,
                transactions_root,
                receipts_root,
                number,
                energy_used,
                energy_limit,
                extra_data,
                logs_bloom,
                timestamp,
                difficulty,
                total_difficulty,
                seal_fields,
                uncles,
                size,
                mix_hash,
                nonce,
                transactions,
            }
        }
    }
}

impl From<Block<Transaction>> for Block<TxHash> {
    fn from(full: Block<Transaction>) -> Self {
        {
            let Block {
                hash,
                parent_hash,
                uncles_hash,
                author,
                state_root,
                transactions_root,
                receipts_root,
                number,
                energy_used,
                energy_limit,
                extra_data,
                logs_bloom,
                timestamp,
                difficulty,
                total_difficulty,
                seal_fields,
                uncles,
                transactions,
                size,
                mix_hash,
                nonce,
            } = full;
            Block {
                hash,
                parent_hash,
                uncles_hash,
                author,
                state_root,
                transactions_root,
                receipts_root,
                number,
                energy_used,
                energy_limit,
                extra_data,
                logs_bloom,
                timestamp,
                difficulty,
                total_difficulty,
                seal_fields,
                uncles,
                size,
                mix_hash,
                nonce,
                transactions: transactions.iter().map(|tx| tx.hash).collect(),
            }
        }
    }
}

/// A [block hash](H256) or [block number](BlockNumber).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlockId {
    // TODO: May want to expand this to include the requireCanonical field
    // <https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1898.md>
    /// A block hash
    Hash(H256),
    /// A block number
    Number(BlockNumber),
}

impl From<u64> for BlockId {
    fn from(num: u64) -> Self {
        BlockNumber::Number(num.into()).into()
    }
}

impl From<U64> for BlockId {
    fn from(num: U64) -> Self {
        BlockNumber::Number(num).into()
    }
}

impl From<BlockNumber> for BlockId {
    fn from(num: BlockNumber) -> Self {
        BlockId::Number(num)
    }
}

impl From<H256> for BlockId {
    fn from(hash: H256) -> Self {
        BlockId::Hash(hash)
    }
}

impl Serialize for BlockId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            BlockId::Hash(ref x) => {
                let mut s = serializer.serialize_struct("BlockIdEip1898", 1)?;
                s.serialize_field("blockHash", &format!("{x:?}"))?;
                s.end()
            }
            BlockId::Number(ref num) => num.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for BlockId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BlockIdVisitor;

        impl<'de> Visitor<'de> for BlockIdVisitor {
            type Value = BlockId;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("Block identifier following EIP-1898")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BlockId::Number(v.parse().map_err(serde::de::Error::custom)?))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut number = None;
                let mut hash = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "blockNumber" => {
                            if number.is_some() || hash.is_some() {
                                return Err(serde::de::Error::duplicate_field("blockNumber"))
                            }
                            number = Some(BlockId::Number(map.next_value::<BlockNumber>()?))
                        }
                        "blockHash" => {
                            if number.is_some() || hash.is_some() {
                                return Err(serde::de::Error::duplicate_field("blockHash"))
                            }
                            hash = Some(BlockId::Hash(map.next_value::<H256>()?))
                        }
                        key => {
                            return Err(serde::de::Error::unknown_field(
                                key,
                                &["blockNumber", "blockHash"],
                            ))
                        }
                    }
                }

                number.or(hash).ok_or_else(|| {
                    serde::de::Error::custom("Expected `blockNumber` or `blockHash`")
                })
            }
        }

        deserializer.deserialize_any(BlockIdVisitor)
    }
}

impl FromStr for BlockId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("0x") && s.len() == 66 {
            let hash = s.parse::<H256>().map_err(|e| e.to_string());
            hash.map(Self::Hash)
        } else {
            s.parse().map(Self::Number)
        }
    }
}

/// A block number or tag.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlockNumber {
    /// Latest block
    #[default]
    Latest,
    /// Finalized block accepted as canonical
    Finalized,
    /// Safe head block
    Safe,
    /// Earliest block (genesis)
    Earliest,
    /// Pending block (not yet part of the blockchain)
    Pending,
    /// Block by number from canon network
    Number(U64),
}

impl BlockNumber {
    /// Returns the numeric block number if explicitly set
    pub fn as_number(&self) -> Option<U64> {
        match *self {
            BlockNumber::Number(num) => Some(num),
            _ => None,
        }
    }

    /// Returns `true` if a numeric block number is set
    pub fn is_number(&self) -> bool {
        matches!(self, BlockNumber::Number(_))
    }

    /// Returns `true` if it's "latest"
    pub fn is_latest(&self) -> bool {
        matches!(self, BlockNumber::Latest)
    }

    /// Returns `true` if it's "finalized"
    pub fn is_finalized(&self) -> bool {
        matches!(self, BlockNumber::Finalized)
    }

    /// Returns `true` if it's "safe"
    pub fn is_safe(&self) -> bool {
        matches!(self, BlockNumber::Safe)
    }

    /// Returns `true` if it's "pending"
    pub fn is_pending(&self) -> bool {
        matches!(self, BlockNumber::Pending)
    }

    /// Returns `true` if it's "earliest"
    pub fn is_earliest(&self) -> bool {
        matches!(self, BlockNumber::Earliest)
    }
}

impl<T: Into<U64>> From<T> for BlockNumber {
    fn from(num: T) -> Self {
        BlockNumber::Number(num.into())
    }
}

impl Serialize for BlockNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            BlockNumber::Number(ref x) => serializer.serialize_str(&format!("0x{x:x}")),
            BlockNumber::Latest => serializer.serialize_str("latest"),
            BlockNumber::Finalized => serializer.serialize_str("finalized"),
            BlockNumber::Safe => serializer.serialize_str("safe"),
            BlockNumber::Earliest => serializer.serialize_str("earliest"),
            BlockNumber::Pending => serializer.serialize_str("pending"),
        }
    }
}

impl<'de> Deserialize<'de> for BlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl FromStr for BlockNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "latest" => Ok(Self::Latest),
            "finalized" => Ok(Self::Finalized),
            "safe" => Ok(Self::Safe),
            "earliest" => Ok(Self::Earliest),
            "pending" => Ok(Self::Pending),
            // hex
            n if n.starts_with("0x") => n.parse().map(Self::Number).map_err(|e| e.to_string()),
            // decimal
            n => n.parse::<u64>().map(|n| Self::Number(n.into())).map_err(|e| e.to_string()),
        }
    }
}

impl fmt::Display for BlockNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockNumber::Number(ref x) => format!("0x{x:x}").fmt(f),
            BlockNumber::Latest => f.write_str("latest"),
            BlockNumber::Finalized => f.write_str("finalized"),
            BlockNumber::Safe => f.write_str("safe"),
            BlockNumber::Earliest => f.write_str("earliest"),
            BlockNumber::Pending => f.write_str("pending"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transaction, TxHash};

    #[test]
    fn can_parse_eip1898_block_ids() {
        let num = serde_json::json!(
            { "blockNumber": "0x0" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Number(0u64.into())));

        let num = serde_json::json!(
            { "blockNumber": "pending" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Pending));

        let num = serde_json::json!(
            { "blockNumber": "latest" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Latest));

        let num = serde_json::json!(
            { "blockNumber": "finalized" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Finalized));

        let num = serde_json::json!(
            { "blockNumber": "safe" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Safe));

        let num = serde_json::json!(
            { "blockNumber": "earliest" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Earliest));

        let num = serde_json::json!("0x0");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Number(0u64.into())));

        let num = serde_json::json!("pending");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Pending));

        let num = serde_json::json!("latest");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Latest));

        let num = serde_json::json!("finalized");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Finalized));

        let num = serde_json::json!("safe");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Safe));

        let num = serde_json::json!("earliest");
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(id, BlockId::Number(BlockNumber::Earliest));

        let num = serde_json::json!(
            { "blockHash": "0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3" }
        );
        let id = serde_json::from_value::<BlockId>(num).unwrap();
        assert_eq!(
            id,
            BlockId::Hash(
                "0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"
                    .parse()
                    .unwrap()
            )
        );
    }

    #[test]
    fn serde_block_number() {
        for b in &[
            BlockNumber::Latest,
            BlockNumber::Finalized,
            BlockNumber::Safe,
            BlockNumber::Earliest,
            BlockNumber::Pending,
        ] {
            let b_ser = serde_json::to_string(&b).unwrap();
            let b_de: BlockNumber = serde_json::from_str(&b_ser).unwrap();
            assert_eq!(b_de, *b);
        }

        let b = BlockNumber::Number(1042u64.into());
        let b_ser = serde_json::to_string(&b).unwrap();
        let b_de: BlockNumber = serde_json::from_str(&b_ser).unwrap();
        assert_eq!(b_ser, "\"0x412\"");
        assert_eq!(b_de, b);
    }

    #[test]
    fn deserialize_blk_no_txs() {
        let block = r#"{"number":"0x3","hash":"0xda53da08ef6a3cbde84c33e51c04f68c3853b6a3731f10baa2324968eee63972","parentHash":"0x689c70c080ca22bc0e681694fa803c1aba16a69c8b6368fed5311d279eb9de90","mixHash":"0x0000000000000000000000000000000000000000000000000000000000000000","nonce":"0x0000000000000000","sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x7270c1c4440180f2bd5215809ee3d545df042b67329499e1ab97eb759d31610d","stateRoot":"0x29f32984517a7d25607da485b23cefabfd443751422ca7e603395e1de9bc8a4b","receiptsRoot":"0x056b23fbba480696b65fe5a59b8f2148a1299103c4f57df839233af2cf4ca2d2","miner":"0x00000000000000000000000000000000000000000000","difficulty":"0x0","totalDifficulty":"0x0","extraData":"0x","size":"0x3e8","energyLimit":"0x6691b7","energyUsed":"0x5208","timestamp":"0x5ecedbb9","transactions":["0xc3c5f700243de37ae986082fd2af88d2a7c2752a0c0f7b9d6ac47c729d45e067"],"uncles":[]}"#;
        let _block: Block<TxHash> = serde_json::from_str(block).unwrap();
    }

    #[test]
    fn deserialize_blk_with_txs() {
        let block = r#"{"number":"0x3","hash":"0xda53da08ef6a3cbde84c33e51c04f68c3853b6a3731f10baa2324968eee63972","parentHash":"0x689c70c080ca22bc0e681694fa803c1aba16a69c8b6368fed5311d279eb9de90","mixHash":"0x0000000000000000000000000000000000000000000000000000000000000000","nonce":"0x0000000000000000","sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","transactionsRoot":"0x7270c1c4440180f2bd5215809ee3d545df042b67329499e1ab97eb759d31610d","stateRoot":"0x29f32984517a7d25607da485b23cefabfd443751422ca7e603395e1de9bc8a4b","receiptsRoot":"0x056b23fbba480696b65fe5a59b8f2148a1299103c4f57df839233af2cf4ca2d2","miner":"0x00000000000000000000000000000000000000000000","difficulty":"0x0","totalDifficulty":"0x0","extraData":"0x","size":"0x3e8","energyLimit":"0x6691b7","energyUsed":"0x5208","timestamp":"0x5ecedbb9","transactions":[{"hash":"0xc3c5f700243de37ae986082fd2af88d2a7c2752a0c0f7b9d6ac47c729d45e067","nonce":"0x2","blockHash":"0xda53da08ef6a3cbde84c33e51c04f68c3853b6a3731f10baa2324968eee63972","blockNumber":"0x3","transactionIndex":"0x0","from":"0xfdcedc3bfca10ecb0890337fbdd1977aba848000007a","to":"0xdca8ce283150ab773bcbeb8d38289bdb5661d0000e1e","value":"0x0","energy":"0x15f90","energyPrice":"0x4a817c800","input":"0x","v":"0x25","r":"0x19f2694eb9113656dbea0b925e2e7ceb43df83e601c4116aee9c0dd99130be88","s":"0x73e5764b324a4f7679d890a198ba658ba1c8cd36983ff9797e10b1b89dbb448e"}],"uncles":[]}"#;
        let _block: Block<Transaction> = serde_json::from_str(block).unwrap();
    }

    #[test]
    fn pending_block() {
        let json = serde_json::json!(
        {
          "difficulty": "0x329b1f81605a4a",
          "extraData": "0xd983010a0d846765746889676f312e31362e3130856c696e7578",
          "energyLimit": "0x1c950d9",
          "energyUsed": "0x1386f81",
          "hash": null,
          "logsBloom": "0x5a3fc3425505bf83b1ebe6ffead1bbfdfcd6f9cfbd1e5fb7fbc9c96b1bbc2f2f6bfef959b511e4f0c7d3fbc60194fbff8bcff8e7b8f6ba9a9a956fe36473ed4deec3f1bc67f7dabe48f71afb377bdaa47f8f9bb1cd56930c7dfcbfddf283f9697fb1db7f3bedfa3e4dfd9fae4fb59df8ac5d9c369bff14efcee59997df8bb16d47d22f0bfbafb29fbfff6e1e41bca61e37e7bdfde1fe27b9fd3a7adfcb74fe98e6dbcc5f5bb3bd4d4bb6ccd29fd3bd446c7f38dcaf7ff78fb3f3aa668cbffe56291d7fbbebd2549fdfd9f223b3ba61dee9e92ebeb5dc967f711d039ff1cb3c3a8fb3b7cbdb29e6d1e79e6b95c596dfe2be36fd65a4f6fdeebe7efbe6e38037d7",
          "miner": null,
          "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
          "nonce": null,
          "number": "0xe1a6ee",
          "parentHash": "0xff1a940068dfe1f9e3f5514e6b7ff5092098d21d706396a9b19602f0f2b11d44",
          "receiptsRoot": "0xe7df36675953a2d2dd22ec0e44ff59818891170875ebfba5a39f6c85084a6f10",
          "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
          "size": "0xd8d6",
          "stateRoot": "0x2ed13b153d1467deb397a789aabccb287590579cf231bf4f1ff34ac3b4905ce2",
          "timestamp": "0x6282b31e",
          "totalDifficulty": null,
          "transactions": [
            "0xaad0d53ae350dd06481b42cd097e3f3a4a31e0ac980fac4663b7dd913af20d9b"
          ],
          "transactionsRoot": "0xfc5c28cb82d5878172cd1b97429d44980d1cee03c782950ee73e83f1fa8bfb49",
          "uncles": []
        }
        );
        let block: Block<H256> = serde_json::from_value(json).unwrap();
        assert!(block.author.is_none());
    }

    #[test]
    fn can_deserialize_with_sealed_fields() {
        let json = serde_json::json!({
          "number": "0x1b4",
          "hash": "0xdc0818cf78f21a8e70579cb46a43643f78291264dda342ae31049421c82d21ae",
          "parentHash": "0xe99e022112df268087ea7eafaf4790497fd21dbeeb6bd7a1721df161a6657a54",
          "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
          "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
          "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
          "stateRoot": "0xddc8b0234c2e0cad087c8b389aa7ef01f7d79b2570bccb77ce48648aa61c904d",
          "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
          "miner": "0x0000bb7b8287f3f0a933474a79eae42cbca977791171",
          "difficulty": "0x4ea3f27bc",
          "totalDifficulty": "0x78ed983323d",
          "sealFields": null,
          "nonce": "0x689056015818adbe",
          "mixHash": "0x4fffe9ae21f1c9e15207b1f472d5bbdd68c9595d461666602f2be20daf5e7843",
          "extraData": "0x476574682f4c5649562f76312e302e302f6c696e75782f676f312e342e32",
          "size": "0x0",
          "energyLimit": "0x1388",
          "energyUsed": "0x0",
          "timestamp": "0x55ba467c",
          "transactions": [],
          "uncles": []
        }
              );
        let _block: Block<TxHash> = serde_json::from_value(json).unwrap();
    }
}
