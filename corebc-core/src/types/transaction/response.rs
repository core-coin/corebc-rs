//! Transaction types
use super::{decode_signature, decode_to, eip2718::TypedTransaction, rlp_opt};
use crate::{
    types::{
        Address, Bloom, Bytes, Log, Network, Signature, SignatureError, H1368, H256, U256, U64,
    },
    utils::sha3,
};
use rlp::{Decodable, DecoderError, RlpStream};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Details of a signed transaction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Transaction {
    /// The transaction's hash
    pub hash: H256,

    /// The transaction's nonce
    pub nonce: U256,

    /// Block hash. None when pending.
    #[serde(default, rename = "blockHash")]
    pub block_hash: Option<H256>,

    /// Block number. None when pending.
    #[serde(default, rename = "blockNumber")]
    pub block_number: Option<U64>,

    /// Sender
    #[serde(default = "crate::types::Address::zero")]
    pub from: Address,

    /// Recipient (None when contract creation)
    #[serde(default)]
    pub to: Option<Address>,

    /// Transferred value
    pub value: U256,

    /// energy Price
    #[serde(rename = "energyPrice")]
    pub energy_price: U256,

    /// Energy amount
    pub energy: U256,

    /// Input data
    pub input: Bytes,

    /// Signature
    #[serde(default, rename = "signature")]
    pub sig: H1368,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<U256>,
}

impl Transaction {
    pub fn hash(&self) -> H256 {
        sha3(self.rlp().as_ref()).into()
    }

    pub fn rlp(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_unbounded_list();

        rlp.append(&self.nonce);
        rlp.append(&self.energy_price);
        rlp.append(&self.energy);

        rlp_opt(&mut rlp, &self.network_id);
        rlp_opt(&mut rlp, &self.to);

        rlp.append(&self.value);
        rlp.append(&self.input.as_ref());

        rlp.append(&self.sig);

        rlp.finalize_unbounded_list();

        let rlp_bytes: Bytes = rlp.out().freeze().into();
        rlp_bytes
    }

    /// Decodes a legacy transaction starting at the RLP offset passed.
    /// Increments the offset for each element parsed.
    #[inline]
    fn decode_base_legacy(
        &mut self,
        rlp: &rlp::Rlp,
        offset: &mut usize,
    ) -> Result<(), DecoderError> {
        self.nonce = rlp.val_at(*offset)?;
        *offset += 1;
        self.energy_price = rlp.val_at(*offset)?;
        *offset += 1;
        self.energy = rlp.val_at(*offset)?;
        *offset += 1;

        self.network_id = Some(rlp.val_at(*offset)?);
        *offset += 1;

        self.to = decode_to(rlp, offset)?;
        self.value = rlp.val_at(*offset)?;
        *offset += 1;
        let input = rlp::Rlp::new(rlp.at(*offset)?.as_raw()).data()?;
        self.input = Bytes::from(input.to_vec());
        *offset += 1;
        Ok(())
    }

    /// Recover the sender of the tx from signature
    pub fn recover_from(&self) -> Result<Address, SignatureError> {
        let signature = Signature { sig: self.sig };
        let typed_tx: TypedTransaction = self.into();
        // CORETODO: Please find a way to unwrap it more naturally
        let network = Network::try_from(typed_tx.network_id().unwrap()).unwrap();
        signature.recover(typed_tx.sighash(), &network)
    }

    /// Recover the sender of the tx from signature and set the from field
    pub fn recover_from_mut(&mut self) -> Result<Address, SignatureError> {
        let from = self.recover_from()?;
        self.from = from;
        Ok(from)
    }
}

/// Get a Transaction directly from a rlp encoded byte stream
impl Decodable for Transaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, DecoderError> {
        let mut txn = Self { hash: H256(sha3(rlp.as_raw())), ..Default::default() };
        // we can get the type from the first value
        let mut offset = 0;

        // only untyped legacy transactions are lists
        // Legacy (0x00)
        // use the original rlp
        txn.decode_base_legacy(rlp, &mut offset)?;
        let sig = decode_signature(rlp, &mut offset)?;
        txn.sig = sig.sig;

        Ok(txn)
    }
}

/// "Receipt" of an executed transaction: details of its execution.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Transaction hash.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: H256,
    /// Index within the block.
    #[serde(rename = "transactionIndex")]
    pub transaction_index: U64,
    /// Hash of the block this transaction was included within.
    #[serde(rename = "blockHash")]
    pub block_hash: Option<H256>,
    /// Number of the block this transaction was included within.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U64>,
    /// address of the sender.
    pub from: Address,
    // address of the receiver. null when its a contract creation transaction.
    pub to: Option<Address>,
    /// Cumulative energy used within the block after this was executed.
    #[serde(rename = "cumulativeEnergyUsed")]
    pub cumulative_energy_used: U256,
    /// Energy used by this transaction alone.
    ///
    /// Energy used is `None` if the the client is running in light client mode.
    #[serde(rename = "energyUsed")]
    pub energy_used: Option<U256>,
    /// Contract address created, or `None` if not a deployment.
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    /// Logs generated within this transaction.
    pub logs: Vec<Log>,
    /// Status: either 1 (success) or 0 (failure). Only present after activation of [EIP-658](https://eips.ethereum.org/EIPS/eip-658)
    pub status: Option<U64>,
    /// State root. Only present before activation of [EIP-658](https://eips.ethereum.org/EIPS/eip-658)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<H256>,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Bloom,
}

impl rlp::Encodable for TransactionReceipt {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        rlp_opt(s, &self.status);
        s.append(&self.cumulative_energy_used);
        s.append(&self.logs_bloom);
        s.append_list(&self.logs);
    }
}

// Compares the transaction receipt against another receipt by checking the blocks first and then
// the transaction index in the block
impl Ord for TransactionReceipt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.block_number, other.block_number) {
            (Some(number), Some(other_number)) => match number.cmp(&other_number) {
                Ordering::Equal => self.transaction_index.cmp(&other.transaction_index),
                ord => ord,
            },
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.transaction_index.cmp(&other.transaction_index),
        }
    }
}

impl PartialOrd<Self> for TransactionReceipt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use rlp::{Encodable, Rlp};

    use super::*;
    use std::str::FromStr;

    #[test]
    fn decode_transaction_response() {
        let _res: Transaction = serde_json::from_str(
            r#"{
    "blockHash":"0x1d59ff54b1eb26b013ce3cb5fc9dab3705b415a67127a003c3e61eb445bb8df2",
    "blockNumber":"0x5daf3b",
    "from":"0x0000a7d9ddbe1f17865597fbd27ec712455208b6b76d",
    "energy":"0xc350",
    "energyPrice":"0x4a817c800",
    "hash":"0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "input":"0x68656c6c6f21",
    "nonce":"0x15",
    "to":"0x0000f02c1c8e6114b1dbe8937a39260b5b0a374432bb",
    "transactionIndex":"0x41",
    "value":"0xf3dbb76162000",
    "v":"0x25",
    "r":"0x1b5e176d927f8e9ab405058b2d2457392da3e20f328b16ddabcebc33eaac5fea",
    "s":"0x4ba69724e8f69de52f0125ad8b3c5c2cef33019bac3249e2c0a2192766d1721c"
  }"#,
        )
        .unwrap();

        let _res: Transaction = serde_json::from_str(
            r#"{
            "hash":"0xdd79ab0f996150aa3c9f135bbb9272cf0dedb830fafcbbf0c06020503565c44f",
            "nonce":"0xe",
            "blockHash":"0xef3fe1f532c3d8783a6257619bc123e9453aa8d6614e4cdb4cc8b9e1ed861404",
            "blockNumber":"0xf",
            "transactionIndex":"0x0",
            "from":"0x00001b67b03cdccfae10a2d80e52d3d026dbe2960ad0",
            "to":"0x0000986ee0c8b91a58e490ee59718cca41056cf55f24",
            "value":"0x2710",
            "energy":"0x5208",
            "energyPrice":"0x186a0",
            "input":"0x",
            "v":"0x25",
            "r":"0x75188beb2f601bb8cf52ef89f92a6ba2bb7edcf8e3ccde90548cc99cbea30b1e",
            "s":"0xc0559a540f16d031f3404d5df2bb258084eee56ed1193d8b534bb6affdb3c2c"
    }"#,
        )
        .unwrap();
    }

    #[test]
    fn tx_roundtrip() {
        let json = serde_json::json!({"blockHash":"0x31ee73b1cf9ae3adc850c84aa22ed9ce9186f8b927ceeba48a04a56085b38664","blockNumber":"0x68cd2","network_id":"0x3","from":"ab59796210a3fe3c24d433197af05ef54c33279ba80d","energy":"0xf4239","energyPrice":"0x3b9aca00","hash":"0x8305e1f16cd0355d3ba79d604a49d2c707fb87c8a4632814bff00a914b1a87fe","input":"0xca725b7e00000000000000000000000000000000000000000000000000277a879f176600","nonce":"0x11d7","signature": "0xd8dd78f3cb29f8cd26596756ee3df34b3cdb65e9273179e2cd9e2cd325b24e2dea7ab7b640e6569bc5a17028d7f440df0da309e3322d708e00a756221e41ae5538e710199dc2ad67477dfceece9153260e0ee72481e35b3da35d85491cd25bc7d875d48ce98141b65a79ee6598862038210072e307abe34426234c4dd7bee1880a46920fadcebcba5d16e440a2621ad211d0da6155d8be811768141cd9b303ed7c1a4a814c1ae79fb1a780","to":"ab4184ac3f29bedfab8a76895b87564289cd5a962542","value":"0x0"});
        let tx: Transaction = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(tx.nonce, 4567u64.into());

        let encoded = serde_json::to_value(tx).unwrap();
        assert_eq!(encoded, json);
    }

    // CORETODO: REMOVE VRS AND FIX TESTS
    #[test]
    fn rlp_legacy_tx() {
        let tx = Transaction {
            block_hash: None,
            block_number: None,
            from: Address::from_str("cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839").unwrap(),
            energy: U256::from_dec_str("50000").unwrap(),
            energy_price: U256::from_dec_str("10").unwrap(),
            hash:
                H256::from_str("8b141d69ab3e18bf9775144ddc2e3ca55dfc3e8b5e67dfaea4401b4074da4041").unwrap(),
            input: Bytes::from(hex::decode("1123").unwrap()),
            nonce: U256::from_dec_str("3").unwrap(),
            to: Some(Address::from_str("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87").unwrap()),
            value: U256::from_dec_str("10").unwrap(),
            sig: H1368::from_str("0x4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580").unwrap(),
            network_id: Some(U256::from(1)),
        };

        assert_eq!(
            tx.rlp(),
            Bytes::from(
                hex::decode("f8ce030a82c3500196cb08095e7baea6a6c7c4c2dfeb977efac326af552d870a821123b8ab4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
                ).unwrap()
            )
        );
    }

    #[test]
    fn decode_rlp_legacy() {
        let tx = Transaction {
            block_hash: None,
            block_number: None,
            from: Address::from_str("cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839").unwrap(),
            energy: U256::from_dec_str("50000").unwrap(),
            energy_price: U256::from_dec_str("10").unwrap(),
            hash:
                H256::from_str("8b141d69ab3e18bf9775144ddc2e3ca55dfc3e8b5e67dfaea4401b4074da4041").unwrap(),
            input: Bytes::from(hex::decode("1123").unwrap()),
            nonce: U256::from_dec_str("3").unwrap(),
            to: Some(Address::from_str("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87").unwrap()),
            value: U256::from_dec_str("10").unwrap(),
            sig: H1368::from_str("0x4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580").unwrap(),
            network_id: Some(U256::from(1)),
        };

        let rlp_bytes =
            hex::decode("f8ce030a82c3500196cb08095e7baea6a6c7c4c2dfeb977efac326af552d870a821123b8ab4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
            ).unwrap();

        let decoded_transaction = Transaction::decode(&rlp::Rlp::new(&rlp_bytes)).unwrap();

        assert_eq!(decoded_transaction.hash(), tx.hash());
    }

    #[test]
    fn recover_from() {
        let tx = Transaction {
            block_hash: Some(
                H256::from_str("fd00e39f9b15db30a53e9768efade8191dafa5dd8b4791a4ea607e02435755fb")
                .unwrap(),
            ),
            block_number: Some(534366.into()),
            nonce: U256::from_str("d9c").unwrap(),
            energy_price: U256::from_str("3b9aca00").unwrap(),
            energy: U256::from_str("f4239").unwrap(),
            to: Some(Address::from_str("ab258a97844448023d9cada0811bade35a7865985739").unwrap()),
            input: Bytes::from(hex::decode("ca725b7e0000000000000000000000000000000000000000000000000027f29a27e63800").unwrap()),
            value: U256::from_str("0").unwrap(),
            network_id: Some(U256::from(3)),
            from: Address::from_str("ab660ef5114ad53a9fd106b72a260ba5b055a9aeca3c").unwrap(),
            hash:
                H256::from_str("8b59298c5c748bf4e2bd84a00aae809f9b6d8c41a5571d47679b5a39041f56ec").unwrap(),
            sig: H1368::from_str("0xf7571bfb2b44b2f1e48c64f75430a22202f6592969655704218ce35f1aeb10bf7228d89871a24ff23ebe6bc66a75bbf0b831a4c57c3dc779005b62713cb0b70c960da8bc81a37f9551b632ce902df309ca4229d7dc4a4179b05800eede1766b8a0ab0d63032d7ba990197374ab786d832f008f3572f16fbefbb5a85f9eed54c77db3d4269b2c64e5d56a5174c19b35d292941d40505063351ce79852053062cdf8d74f3db2d5bebe7b3500").unwrap(),
        };

        assert_eq!(tx.from, tx.recover_from().unwrap());
        assert_eq!(tx.hash, tx.hash());
    }

    #[test]
    fn decode_transaction_receipt() {
        let _res: TransactionReceipt = serde_json::from_str(
            r#"{
        "transactionHash": "0xa3ece39ae137617669c6933b7578b94e705e765683f260fcfe30eaa41932610f",
        "blockHash": "0xf6084155ff2022773b22df3217d16e9df53cbc42689b27ca4789e06b6339beb2",
        "blockNumber": "0x52a975",
        "contractAddress": null,
        "cumulativeEnergyUsed": "0x797db0",
        "from": "0x0000d907941c8b3b966546fc408b8c942eb10a4f98df",
        "energyUsed": "0x1308c",
        "logs": [
            {
                "blockHash": "0xf6084155ff2022773b22df3217d16e9df53cbc42689b27ca4789e06b6339beb2",
                "address": "0x0000d6df5935cd03a768b7b9e92637a01b25e24cb709",
                "logIndex": "0x119",
                "data": "0x0000000000000000000000000000000000000000000000000000008bb2c97000",
                "removed": false,
                "topics": [
                    "0x8940c4b8e215f8822c5c8f0056c12652c746cbc57eedbd2a440b175971d47a77",
                    "0x000000000000000000000000d907941c8b3b966546fc408b8c942eb10a4f98df"
                ],
                "blockNumber": "0x52a975",
                "transactionIndex": "0x29",
                "transactionHash": "0xa3ece39ae137617669c6933b7578b94e705e765683f260fcfe30eaa41932610f"
            },
            {
                "blockHash": "0xf6084155ff2022773b22df3217d16e9df53cbc42689b27ca4789e06b6339beb2",
                "address": "0x0000d6df5935cd03a768b7b9e92637a01b25e24cb709",
                "logIndex": "0x11a",
                "data": "0x0000000000000000000000000000000000000000000000000000008bb2c97000",
                "removed": false,
                "topics": [
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "0x000000000000000000000000d907941c8b3b966546fc408b8c942eb10a4f98df"
                ],
                "blockNumber": "0x52a975",
                "transactionIndex": "0x29",
                "transactionHash": "0xa3ece39ae137617669c6933b7578b94e705e765683f260fcfe30eaa41932610f"
            }
        ],
        "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000020000000000000000000800000000000000004010000010100000000000000000000000000000000000000000000000000040000080000000000000080000000000000000000000000000000000000000000020000000000000000000000002000000000000000000000000000000000000000000000000000020000000010000000000000000000000000000000000000000000000000000000000",
        "root": null,
        "status": "0x1",
        "to": "0x0000d6df5935cd03a768b7b9e92637a01b25e24cb709",
        "transactionIndex": "0x29"
    }"#,
        )
        .unwrap();
    }

    #[test]
    fn serde_create_transaction_receipt() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{
    "transactionHash": "0x611b173b0e0dfda94da7bfb6cb77c9f1c03e2f2149ba060e6bddfaa219942369",
    "blockHash": "0xa11871d61e0e703ae33b358a6a9653c43e4216f277d4a1c7377b76b4d5b4cbf1",
    "blockNumber": "0xe3c1d8",
    "contractAddress": "000008f6db30039218894067023a3593baf27d3f4a2b",
    "cumulativeEnergyUsed": "0x1246047",
    "from": "00000968995a48162a23af60d3ca25cddfa143cd8891",
    "energyUsed": "0x1b9229",
    "logs": [
      {
        "address": "000008f6db30039218894067023a3593baf27d3f4a2b",
        "topics": [
          "0x40c340f65e17194d14ddddb073d3c9f888e3cb52b5aae0c6c7706b4fbc905fac"
        ],
        "data": "0x0000000000000000000000000968995a48162a23af60d3ca25cddfa143cd88910000000000000000000000000000000000000000000000000000000000002616",
        "blockNumber": "0xe3c1d8",
        "transactionHash": "0x611b173b0e0dfda94da7bfb6cb77c9f1c03e2f2149ba060e6bddfaa219942369",
        "transactionIndex": "0xdf",
        "blockHash": "0xa11871d61e0e703ae33b358a6a9653c43e4216f277d4a1c7377b76b4d5b4cbf1",
        "logIndex": "0x196",
        "removed": false
      },
      {
        "address": "000008f6db30039218894067023a3593baf27d3f4a2b",
        "topics": [
          "0x40c340f65e17194d14ddddb073d3c9f888e3cb52b5aae0c6c7706b4fbc905fac"
        ],
        "data": "0x00000000000000000000000059750ac0631f63bfdce0f0867618e468e11ee34700000000000000000000000000000000000000000000000000000000000000fa",
        "blockNumber": "0xe3c1d8",
        "transactionHash": "0x611b173b0e0dfda94da7bfb6cb77c9f1c03e2f2149ba060e6bddfaa219942369",
        "transactionIndex": "0xdf",
        "blockHash": "0xa11871d61e0e703ae33b358a6a9653c43e4216f277d4a1c7377b76b4d5b4cbf1",
        "logIndex": "0x197",
        "removed": false
      }
    ],
    "logsBloom": "0x00000000000000800000000040000000000000000000000000000000000000000000008000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "status": "0x1",
    "to": null,
    "transactionIndex": "0xdf"
}
"#,
        )
        .unwrap();

        let receipt: TransactionReceipt = serde_json::from_value(v.clone()).unwrap();
        assert!(receipt.to.is_none());
        let receipt = serde_json::to_value(receipt).unwrap();
        assert_eq!(v, receipt);
    }

    #[test]
    fn rlp_encode_receipt() {
        let receipt = TransactionReceipt { status: Some(1u64.into()), ..Default::default() };
        let encoded = receipt.rlp_bytes();

        assert_eq!(
            encoded,
            hex::decode("f901060180b9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c0").unwrap(),
        );
    }

    #[test]
    fn can_sort_receipts() {
        let mut a = TransactionReceipt { block_number: Some(0u64.into()), ..Default::default() };
        let b = TransactionReceipt { block_number: Some(1u64.into()), ..Default::default() };
        assert!(a < b);

        a = b.clone();
        assert_eq!(a.cmp(&b), Ordering::Equal);

        a.transaction_index = 1u64.into();
        assert!(a > b);
    }

    #[test]
    fn test_rlp_decoding_create_roundtrip() {
        let tx = Transaction {
            block_hash: None,
            block_number: None,
            from: Address::from_str("0000c26ad91f4e7a0cad84c4b9315f420ca9217e315d").unwrap(),
            energy: U256::from_str_radix("0x10e2b", 16).unwrap(),
            energy_price: U256::from_str_radix("0x12ec276caf", 16).unwrap(),
            hash: H256::from_str("929ff27a5c7833953df23103c4eb55ebdfb698678139d751c51932163877fada").unwrap(),
            input: Bytes::from(
                hex::decode("a9059cbb000000000000000000000000fdae129ecc2c27d166a3131098bc05d143fa258e0000000000000000000000000000000000000000000000000000000002faf080").unwrap()
            ),
            nonce: U256::zero(),
            value: U256::zero(),
            ..Default::default()
        };
        Transaction::decode(&Rlp::new(&tx.rlp())).unwrap();
    }
}
