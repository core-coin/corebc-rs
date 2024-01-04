use crate::types::{Address, Transaction, U256, U64};
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use std::{collections::BTreeMap, fmt, str::FromStr};

/// Transaction summary as found in the Txpool Inspection property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxpoolInspectSummary {
    /// Recipient (None when contract creation)
    pub to: Option<Address>,
    /// Transferred value
    pub value: U256,
    /// energy amount
    pub energy: U256,
    /// energy Price
    pub energy_price: U256,
}

/// Visitor struct for TxpoolInspectSummary.
struct TxpoolInspectSummaryVisitor;

/// Walk through the deserializer to parse a txpool inspection summary into the
/// `TxpoolInspectSummary` struct.
impl<'de> Visitor<'de> for TxpoolInspectSummaryVisitor {
    type Value = TxpoolInspectSummary;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("to: value wei + energyLimit energy × energy_price wei")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let addr_split: Vec<&str> = value.split(": ").collect();
        if addr_split.len() != 2 {
            return Err(de::Error::custom("invalid format for TxpoolInspectSummary: to"))
        }
        let value_split: Vec<&str> = addr_split[1].split(" wei + ").collect();
        if value_split.len() != 2 {
            return Err(de::Error::custom("invalid format for TxpoolInspectSummary: energyLimit"))
        }
        let energy_split: Vec<&str> = value_split[1].split(" energy × ").collect();
        if energy_split.len() != 2 {
            return Err(de::Error::custom("invalid format for TxpoolInspectSummary: energy"))
        }
        let energy_price_split: Vec<&str> = energy_split[1].split(" wei").collect();
        if energy_price_split.len() != 2 {
            return Err(de::Error::custom("invalid format for TxpoolInspectSummary: energy_price"))
        }
        let addr = match addr_split[0] {
            "" => None,
            "0x" => None,
            "contract creation" => None,
            addr => {
                Some(Address::from_str(addr.trim_start_matches("0x")).map_err(de::Error::custom)?)
            }
        };
        let value = U256::from_dec_str(value_split[0]).map_err(de::Error::custom)?;
        let energy = U256::from_dec_str(energy_split[0]).map_err(de::Error::custom)?;
        let energy_price = U256::from_dec_str(energy_price_split[0]).map_err(de::Error::custom)?;

        Ok(TxpoolInspectSummary { to: addr, value, energy, energy_price })
    }
}

/// Implement the `Deserialize` trait for `TxpoolInspectSummary` struct.
impl<'de> Deserialize<'de> for TxpoolInspectSummary {
    fn deserialize<D>(deserializer: D) -> Result<TxpoolInspectSummary, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TxpoolInspectSummaryVisitor)
    }
}

/// Implement the `Serialize` trait for `TxpoolInspectSummary` struct so that the
/// format matches the one from geth.
impl Serialize for TxpoolInspectSummary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let formatted_to = if let Some(to) = self.to {
            format!("{to:?}")
        } else {
            "contract creation".to_string()
        };
        let formatted = format!(
            "{}: {} wei + {} energy × {} wei",
            formatted_to, self.value, self.energy, self.energy_price
        );
        serializer.serialize_str(&formatted)
    }
}

/// Transaction Pool Content
///
/// The content inspection property can be queried to list the exact details of all
/// the transactions currently pending for inclusion in the next block(s), as well
/// as the ones that are being scheduled for future execution only.
///
/// See [here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_content) for more details
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxpoolContent {
    /// pending tx
    pub pending: BTreeMap<Address, BTreeMap<String, Transaction>>,
    /// queued tx
    pub queued: BTreeMap<Address, BTreeMap<String, Transaction>>,
}

/// Transaction Pool Inspect
///
/// The inspect inspection property can be queried to list a textual summary
/// of all the transactions currently pending for inclusion in the next block(s),
/// as well as the ones that are being scheduled for future execution only.
/// This is a method specifically tailored to developers to quickly see the
/// transactions in the pool and find any potential issues.
///
/// See [here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_inspect) for more details
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxpoolInspect {
    /// pending tx
    pub pending: BTreeMap<Address, BTreeMap<String, TxpoolInspectSummary>>,
    /// queued tx
    pub queued: BTreeMap<Address, BTreeMap<String, TxpoolInspectSummary>>,
}

/// Transaction Pool Status
///
/// The status inspection property can be queried for the number of transactions
/// currently pending for inclusion in the next block(s), as well as the ones that
/// are being scheduled for future execution only.
///
/// See [here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_status) for more details
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TxpoolStatus {
    /// number of pending tx
    pub pending: U64,
    /// number of queued tx
    pub queued: U64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serde_txpool_content() {
        // Gathered from geth v1.10.23-stable-d901d853/linux-amd64/go1.18.5
        // Addresses used as keys in `pending` and `queued` have been manually lowercased to
        // simplify the test.
        let txpool_content_json = r#"
{
  "pending": {
    "cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839": {
      "3": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb15d3649d846a2bd426c0ceaca24fab50f7cba8f839",
        "energy": "0xc350",
        "energyPrice": "0xa",
        "hash": "0x8b141d69ab3e18bf9775144ddc2e3ca55dfc3e8b5e67dfaea4401b4074da4041",
        "input": "0x1123",
        "nonce": "0x3",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0xa",
        "transactionIndex": null,
        "network_id": "0x1",
        "signature": "0x4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    },
    "cb56ec43cd027a23ccaf585f2a10456e78b64a9eacde": {
      "3": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb56ec43cd027a23ccaf585f2a10456e78b64a9eacde",
        "energy": "0xc350",
        "energyPrice": "0xa",
        "hash": "0x9ae25e5faa89d8db6a515fa858f6d326ebb54d10c99a68485607900651671607",
        "input": "0x27dc297e800332e506f28f49a13c1edf087bdd6482d6cb3abdf2a4c455642aef1e98fc240000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000002d7b22444149223a313439332e37342c2254555344223a313438392e36362c2255534443223a313439322e34387d00000000000000000000000000000000000000",
        "nonce": "0x3",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0xa",
        "transactionIndex": null,
        "network_id": "0x1",
        "signature": "0x0fbac47922e6e0649343400231a15e26f4f5ab1490fa5e243470de6ca26fd3583b7fa03170600a37b29d214fa618a32d6c2a121552f556578097176bf2ccb9dee0f37e8547d8f5981b6b998f99bf24c92e08b61ca5a7da5ab3da43986881356af9ad55e9b9481432cb1194a7c1302bc72500ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    },
    "cb8249fdd4e9115d8d03d9387db1299985af4e4b2b6d": {
      "12": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb8249fdd4e9115d8d03d9387db1299985af4e4b2b6d",
        "energy": "0xc350",
        "energyPrice": "0xa",
        "hash": "0xe2e86517c0aa95989e438cf6f08ca060d755ce03a0aa4775c7a49fdf650e4ad1",
        "input": "0xa9059cbb00000000000000000000000050272a56ef9aff7238e8b40347da62e87c1f69e200000000000000000000000000000000000000000000000000000000428d3dfc",
        "nonce": "0xc",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "transactionIndex": null,
        "value": "0xa",
        "network_id": "0x1",
        "signature": "0xd13e9f21f6e392e645b944913406b415a4fa81d75c0979b0a1e03a940369c7a749dfcf663a2b28c7170747e76d24c675e5ddab3dcd8f562a80b41bc8a37843b73cf133a3b6080811a6279894b1629640b8318f2f989efab5dcd643905a64916db94920ae1970e362920883d340ca39aa3600ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    }
  },
  "queued": {
    "cb8249fdd4e9115d8d03d9387db1299985af4e4b2b6d": {
      "143": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb8249fdd4e9115d8d03d9387db1299985af4e4b2b6d",
        "energy": "0x7a134",
        "energyPrice": "0x65",
        "transactionIndex": null,
        "hash": "0x3183b9de71cce6a7b8cc4e39d5939cecdefb2f8b45013d5fd61363764b4f890a",
        "input": "0x03a9ea6d00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000f2ff840000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000041d0c4694374d7893d63605625687be2f01028a5b49eca00f72901e773ad8ba7906e58d43e114a28353efaf8abd6a2675de83a3a07af579b8b268e6b714376610d1c00000000000000000000000000000000000000000000000000000000000000",
        "nonce": "0x8f",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0x2fbc",
        "network_id": "0x1",
        "signature": "0xb441bdaf2704bcd492519fbc861107af419f4041e952a261fb8d2759fe9e4ff667fd1d9ac65b1919d47a6ce621126d065577e2215bcfad6400f3bfdfdd4016464ac9dc23d1a0f8ad8782d1bdb3a14c5d7db60b120d905b60f773445faad7013a6528217749cca089a6c7e30b3da10ee51600ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    },
    "cb96cce45e424604ccc2ae6940db14ebf77b17706234": {
      "1": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb96cce45e424604ccc2ae6940db14ebf77b17706234",
        "energy": "0x7a134",
        "energyPrice": "0x65",
        "hash": "0xcece43db04c01d9cd4354c0a59d6748324da6c4483aef0eb8af8ec6605d981a3",
        "input": "0x",
        "nonce": "0x1",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0x2fbc",
        "transactionIndex": null,
        "network_id": "0x1",
        "signature": "0x9dc03c7a1e40154fac1a2de81c332999f5683e0028d152f0f764effae4ac5b68ad05419156a45031314869781d0f0dc8a57d24a13a71592800ff90d00ed8c8256d5250c00d4f01b4aca2d19e40c7d2cbaa2c2c6dc8fcbcb153e4eda482f87e0bedb5fb9755ab514738dbc6d8f80ff9931200ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      },
      "4": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb96cce45e424604ccc2ae6940db14ebf77b17706234",
        "energy": "0x7a134",
        "energyPrice": "0x65",
        "hash": "0x7e703246d3ce598de0b94d0fdbbdbddf4ed2c5ed5dacfb8e273da39d9e9b1b09",
        "input": "0x",
        "nonce": "0x4",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0x2fbc",
        "network_id": "0x1",
        "transactionIndex": null,
        "signature": "0x19952ddfd9e866f914c37753821c93c6c215f3fc0db0eabfec688a525df86cdbe7417522315da4e81bb3dd458aaec014c8f55453c3d3001c001e666721f5eacb7e14435671e5cd4886de40e7c4e23c04438b56c8b2f86f15adde0238ad1ca6ad48505c5b86f04f3e71d66c54dd90077e0b00ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    },
    "cb16081eb6e0eba56e6a20e37530840e205d83c2b3ac": {
      "44": {
        "blockHash": null,
        "blockNumber": null,
        "from": "cb16081eb6e0eba56e6a20e37530840e205d83c2b3ac",
        "energy": "0x7a134",
        "energyPrice": "0x65",
        "hash": "0xe52e75d0e49e8ac110c50d15688f6d2dabdb67c1664680f1783d90565af4f97b",
        "input": "0x095ea7b300000000000000000000000029fbd00940df70cfc5dad3f2370686991e2bbf5cffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "nonce": "0x2c",
        "to": "cb08095e7baea6a6c7c4c2dfeb977efac326af552d87",
        "value": "0x2fbc",
        "transactionIndex": null,
        "network_id": "0x1",
        "signature": "0xb2e49c3a8de9edfa01e597bf00140f33b7dbf62450b082ecfe93b8740d99c3fc458a34f439d880438de1ea9b2fab4d02fd41d293987702ca00ca9d17f32c30f64da86bb674c03eaa66e6bf60324356c657b175c4eb6cf1fdcf62af649f589f948c214edd391e9b0e69578168a36b233b2400ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
      }
    }
  }
}"#;
        let deserialized: TxpoolContent = serde_json::from_str(txpool_content_json).unwrap();
        let serialized: String = serde_json::to_string_pretty(&deserialized).unwrap();

        let origin: serde_json::Value = serde_json::from_str(txpool_content_json).unwrap();
        let serialized_value = serde_json::to_value(deserialized.clone()).unwrap();
        assert_eq!(origin, serialized_value);
        assert_eq!(deserialized, serde_json::from_str::<TxpoolContent>(&serialized).unwrap());
    }

    #[test]
    fn serde_txpool_inspect() {
        let txpool_inspect_json = r#"
{
  "pending": {
    "0x00000512261a7486b1e29704ac49a5eb355b6fd86872": {
      "124930": "0x0000000000000000000000000000000000000000007E: 0 wei + 100187 energy × 20000000000 wei"
    },
    "0x0000201354729f8d0f8b64e9a0c353c672c6a66b3857": {
      "252350": "0x0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF: 0 wei + 65792 energy × 2000000000 wei",
      "252351": "0x0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF: 0 wei + 65792 energy × 2000000000 wei",
      "252352": "0x0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF: 0 wei + 65780 energy × 2000000000 wei",
      "252353": "0x0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF: 0 wei + 65780 energy × 2000000000 wei"
    },
    "0x000000000000863B56a3C1f0F1be8BC4F8b7BD78F57a": {
      "40": "contract creation: 0 wei + 612412 energy × 6000000000 wei"
    }
  },
  "queued": {
    "0x00000f87ffcd71859233eb259f42b236c8e9873444e3": {
      "7": "0x00003479BE69e07E838D9738a301Bb0c89e8EA2Bef4a: 1000000000000000 wei + 21000 energy × 10000000000 wei",
      "8": "0x000073Aaf691bc33fe38f86260338EF88f9897eCaa4F: 1000000000000000 wei + 21000 energy × 10000000000 wei"
    },
    "0x0000307e8f249bcccfa5b245449256c5d7e6e079943e": {
      "3": "0x000073Aaf691bc33fe38f86260338EF88f9897eCaa4F: 10000000000000000 wei + 21000 energy × 10000000000 wei"
    }
  }
}"#;
        let deserialized: TxpoolInspect = serde_json::from_str(txpool_inspect_json).unwrap();
        assert_eq!(deserialized, expected_txpool_inspect());

        let serialized = serde_json::to_string(&deserialized).unwrap();
        let deserialized2: TxpoolInspect = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized2, deserialized);
    }

    #[test]
    fn serde_txpool_status() {
        let txpool_status_json = r#"
{
  "pending": "0x23",
  "queued": "0x20"
}"#;
        let deserialized: TxpoolStatus = serde_json::from_str(txpool_status_json).unwrap();
        let serialized: String = serde_json::to_string_pretty(&deserialized).unwrap();
        assert_eq!(txpool_status_json.trim(), serialized);
    }

    fn expected_txpool_inspect() -> TxpoolInspect {
        let mut pending_map = BTreeMap::new();
        let mut pending_map_inner = BTreeMap::new();
        pending_map_inner.insert(
            "124930".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("0000000000000000000000000000000000000000007E").unwrap(),
                ),
                value: U256::from(0u128),
                energy: U256::from(100187u128),
                energy_price: U256::from(20000000000u128),
            },
        );
        pending_map.insert(
            Address::from_str("00000512261a7486b1e29704ac49a5eb355b6fd86872").unwrap(),
            pending_map_inner.clone(),
        );
        pending_map_inner.clear();
        pending_map_inner.insert(
            "252350".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF").unwrap(),
                ),
                value: U256::from(0u128),
                energy: U256::from(65792u128),
                energy_price: U256::from(2000000000u128),
            },
        );
        pending_map_inner.insert(
            "252351".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF").unwrap(),
                ),
                value: U256::from(0u128),
                energy: U256::from(65792u128),
                energy_price: U256::from(2000000000u128),
            },
        );
        pending_map_inner.insert(
            "252352".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF").unwrap(),
                ),
                value: U256::from(0u128),
                energy: U256::from(65780u128),
                energy_price: U256::from(2000000000u128),
            },
        );
        pending_map_inner.insert(
            "252353".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("0000d10e3Be2bc8f959Bc8C41CF65F60dE721cF89ADF").unwrap(),
                ),
                value: U256::from(0u128),
                energy: U256::from(65780u128),
                energy_price: U256::from(2000000000u128),
            },
        );
        pending_map.insert(
            Address::from_str("0000201354729f8d0f8b64e9a0c353c672c6a66b3857").unwrap(),
            pending_map_inner.clone(),
        );
        pending_map_inner.clear();
        pending_map_inner.insert(
            "40".to_string(),
            TxpoolInspectSummary {
                to: None,
                value: U256::from(0u128),
                energy: U256::from(612412u128),
                energy_price: U256::from(6000000000u128),
            },
        );
        pending_map.insert(
            Address::from_str("000000000000863B56a3C1f0F1be8BC4F8b7BD78F57a").unwrap(),
            pending_map_inner,
        );
        let mut queued_map = BTreeMap::new();
        let mut queued_map_inner = BTreeMap::new();
        queued_map_inner.insert(
            "7".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("00003479BE69e07E838D9738a301Bb0c89e8EA2Bef4a").unwrap(),
                ),
                value: U256::from(1000000000000000u128),
                energy: U256::from(21000u128),
                energy_price: U256::from(10000000000u128),
            },
        );
        queued_map_inner.insert(
            "8".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("000073Aaf691bc33fe38f86260338EF88f9897eCaa4F").unwrap(),
                ),
                value: U256::from(1000000000000000u128),
                energy: U256::from(21000u128),
                energy_price: U256::from(10000000000u128),
            },
        );
        queued_map.insert(
            Address::from_str("00000f87ffcd71859233eb259f42b236c8e9873444e3").unwrap(),
            queued_map_inner.clone(),
        );
        queued_map_inner.clear();
        queued_map_inner.insert(
            "3".to_string(),
            TxpoolInspectSummary {
                to: Some(
                    Address::from_str("000073Aaf691bc33fe38f86260338EF88f9897eCaa4F").unwrap(),
                ),
                value: U256::from(10000000000000000u128),
                energy: U256::from(21000u128),
                energy_price: U256::from(10000000000u128),
            },
        );
        queued_map.insert(
            Address::from_str("0000307e8f249bcccfa5b245449256c5d7e6e079943e").unwrap(),
            queued_map_inner,
        );

        TxpoolInspect { pending: pending_map, queued: queued_map }
    }
}
