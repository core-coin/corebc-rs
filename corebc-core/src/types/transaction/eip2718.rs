use super::request::RequestError;
use crate::{
    types::{
        Address, Bytes, NameOrAddress, Signature, Transaction, TransactionRequest, H256, U256, U64,
    },
    utils::sha3,
};
use rlp::Decodable;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypedTransaction {
    Legacy(TransactionRequest),
}

impl Serialize for TypedTransaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Legacy(tx) => tx.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for TypedTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tx = TransactionRequest::deserialize(deserializer)?;
        Ok(TypedTransaction::Legacy(tx))
    }
}

/// An error involving a typed transaction request.
#[derive(Debug, Error)]
pub enum TypedTransactionError {
    /// When decoding a signed legacy transaction
    #[error(transparent)]
    LegacyError(#[from] RequestError),
    /// Error decoding the transaction type from the transaction's RLP encoding
    #[error(transparent)]
    TypeDecodingError(#[from] rlp::DecoderError),
    /// Missing transaction payload when decoding from RLP
    #[error("Missing transaction payload when decoding")]
    MissingTransactionPayload,
}

impl Default for TypedTransaction {
    fn default() -> Self {
        TypedTransaction::Legacy(Default::default())
    }
}

use TypedTransaction::*;

impl TypedTransaction {
    pub fn from(&self) -> Option<&Address> {
        match self {
            Legacy(inner) => inner.from.as_ref(),
        }
    }

    pub fn set_from(&mut self, from: Address) -> &mut Self {
        match self {
            Legacy(inner) => inner.from = Some(from),
        };
        self
    }

    pub fn to(&self) -> Option<&NameOrAddress> {
        match self {
            Legacy(inner) => inner.to.as_ref(),
        }
    }

    pub fn to_addr(&self) -> Option<&Address> {
        self.to().and_then(|t| t.as_address())
    }

    pub fn set_to<T: Into<NameOrAddress>>(&mut self, to: T) -> &mut Self {
        let to = to.into();
        match self {
            Legacy(inner) => inner.to = Some(to),
        };
        self
    }

    pub fn nonce(&self) -> Option<&U256> {
        match self {
            Legacy(inner) => inner.nonce.as_ref(),
        }
    }

    pub fn set_nonce<T: Into<U256>>(&mut self, nonce: T) -> &mut Self {
        let nonce = nonce.into();
        match self {
            Legacy(inner) => inner.nonce = Some(nonce),
        };
        self
    }

    pub fn value(&self) -> Option<&U256> {
        match self {
            Legacy(inner) => inner.value.as_ref(),
        }
    }

    pub fn set_value<T: Into<U256>>(&mut self, value: T) -> &mut Self {
        let value = value.into();
        match self {
            Legacy(inner) => inner.value = Some(value),
        };
        self
    }

    pub fn energy(&self) -> Option<&U256> {
        match self {
            Legacy(inner) => inner.energy.as_ref(),
        }
    }

    pub fn energy_mut(&mut self) -> &mut Option<U256> {
        match self {
            Legacy(inner) => &mut inner.energy,
        }
    }

    pub fn set_energy<T: Into<U256>>(&mut self, energy: T) -> &mut Self {
        let energy = energy.into();
        match self {
            Legacy(inner) => inner.energy = Some(energy),
        };
        self
    }

    pub fn energy_price(&self) -> Option<U256> {
        match self {
            Legacy(inner) => inner.energy_price,
        }
    }

    pub fn set_energy_price<T: Into<U256>>(&mut self, energy_price: T) -> &mut Self {
        let energy_price = energy_price.into();
        match self {
            Legacy(inner) => inner.energy_price = Some(energy_price),
        };
        self
    }

    pub fn network_id(&self) -> Option<U64> {
        match self {
            Legacy(inner) => inner.network_id,
        }
    }

    pub fn set_network_id<T: Into<U64>>(&mut self, network_id: T) -> &mut Self {
        let network_id = network_id.into();
        match self {
            Legacy(inner) => inner.network_id = Some(network_id),
        };
        self
    }

    pub fn data(&self) -> Option<&Bytes> {
        match self {
            Legacy(inner) => inner.data.as_ref(),
        }
    }

    pub fn set_data(&mut self, data: Bytes) -> &mut Self {
        match self {
            Legacy(inner) => inner.data = Some(data),
        };
        self
    }

    pub fn rlp_signed(&self, signature: &Signature) -> Bytes {
        let mut encoded = vec![];
        match self {
            Legacy(ref tx) => {
                encoded.extend_from_slice(tx.rlp_signed(signature).as_ref());
            }
        };
        encoded.into()
    }

    pub fn rlp(&self) -> Bytes {
        let mut encoded = vec![];
        match self {
            Legacy(inner) => {
                encoded.extend_from_slice(inner.rlp().as_ref());
            }
        };

        encoded.into()
    }

    // Calls inner rlp_sighash to get rlp with network_id as the last field
    // CORETODO: is it possible to have Legacy(inner) without network_id?
    pub fn rlp_sighash(&self) -> Bytes {
        let mut encoded = vec![];
        match self {
            Legacy(inner) => {
                encoded.extend_from_slice(inner.rlp_sighash().as_ref());
            }
        };

        encoded.into()
    }

    /// Hashes the transaction's data. Does not double-RLP encode
    pub fn sighash(&self) -> H256 {
        let encoded = self.rlp_sighash();
        sha3(encoded).into()
    }

    /// Max cost of the transaction
    pub fn max_cost(&self) -> Option<U256> {
        let energy_limit = self.energy();
        let energy_price = self.energy_price();
        match (energy_limit, energy_price) {
            (Some(energy_limit), Some(energy_price)) => Some(energy_limit * energy_price),
            _ => None,
        }
    }

    /// Hashes the transaction's data with the included signature.
    pub fn hash(&self, signature: &Signature) -> H256 {
        sha3(self.rlp_signed(signature).as_ref()).into()
    }

    /// Decodes a signed TypedTransaction from a rlp encoded byte stream
    pub fn decode_signed(rlp: &rlp::Rlp) -> Result<(Self, Signature), TypedTransactionError> {
        if rlp.is_list() {
            let decoded_request = TransactionRequest::decode_signed_rlp(rlp)?;
            return Ok((Self::Legacy(decoded_request.0), decoded_request.1))
        }

        Err(rlp::DecoderError::Custom("invalid tx type").into())
    }
}

/// Get a TypedTransaction directly from a rlp encoded byte stream
impl Decodable for TypedTransaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(Self::Legacy(TransactionRequest::decode(rlp)?))
    }
}

impl From<TransactionRequest> for TypedTransaction {
    fn from(src: TransactionRequest) -> TypedTransaction {
        TypedTransaction::Legacy(src)
    }
}

impl From<&Transaction> for TypedTransaction {
    fn from(tx: &Transaction) -> TypedTransaction {
        let request: TransactionRequest = tx.into();
        request.into()
    }
}

impl TypedTransaction {
    pub fn as_legacy_ref(&self) -> Option<&TransactionRequest> {
        match self {
            Legacy(tx) => Some(tx),
        }
    }

    pub fn as_legacy_mut(&mut self) -> Option<&mut TransactionRequest> {
        match self {
            Legacy(tx) => Some(tx),
        }
    }
}

impl TypedTransaction {
    fn into_legacy(self) -> TransactionRequest {
        match self {
            Legacy(tx) => tx,
        }
    }
}

impl From<TypedTransaction> for TransactionRequest {
    fn from(src: TypedTransaction) -> TransactionRequest {
        src.into_legacy()
    }
}

// CORETODO: Eip 2718 was implemented after the Istanbul hardfork so this is not necessary to test.
// I left it all here in case we will want to use it in the future.
// #[cfg(test)]
// mod tests {
//     use hex::ToHex;
//     use rlp::Decodable;

//     use super::*;
//     use crate::types::{Address, U256};
//     use std::str::FromStr;

//     #[test]
//     fn serde_legacy_tx() {
//         let tx = TransactionRequest::new().to(Address::zero()).value(U256::from(100));
//         let tx: TypedTransaction = tx.into();
//         let serialized = serde_json::to_string(&tx).unwrap();

//         // deserializes to either the envelope type or the inner type
//         let de: TypedTransaction = serde_json::from_str(&serialized).unwrap();
//         assert_eq!(tx, de);

//         let de: TransactionRequest = serde_json::from_str(&serialized).unwrap();
//         assert_eq!(tx, TypedTransaction::Legacy(de));
//     }

//     #[test]
//     fn test_typed_tx_decode() {
//         // this is the same transaction as the above test
//         let typed_tx_hex =
// hex::decode("
// 02f86b8205390284773594008477359400830186a09496216849c49358b10257cb55b28ea603c874b05e865af3107a4000825544f838f7940000000000000000000000000000000000000001e1a00100000000000000000000000000000000000000000000000000000000000000"
// ).unwrap();
//         let tx_rlp = rlp::Rlp::new(typed_tx_hex.as_slice());
//         let actual_tx = TypedTransaction::decode(&tx_rlp).unwrap();

//         let expected =
//             H256::from_str("0x090b19818d9d087a49c3d2ecee4829ee4acea46089c1381ac5e588188627466d")
//                 .unwrap();
//         let actual = actual_tx.sighash();
//         assert_eq!(expected, actual);
//     }

//     // CORETODO: This tests a signed transaction decoding. Change the signing to ED448 and fix
//     // this
//     #[test]
//     fn test_signed_tx_decode() {
//         let expected_tx = Eip1559TransactionRequest::new()
//             .from(Address::from_str("0x00001acadd971da208d25122b645b2ef879868a83e21").unwrap())
//             .network_id(1u64)
//             .nonce(0u64)
//             .max_priority_fee_per_energy(413047990155u64)
//             .max_fee_per_energy(768658734568u64)
//             .energy(184156u64)
//             .to(Address::from_str("0x00000aa7420c43b8c1a7b165d216948870c8ecfe1ee1").unwrap())
//             .value(200000000000000000u64)
//             .data(
//                 Bytes::from_str(
//                     "0x6ecd23060000000000000000000000000000000000000000000000000000000000000002",
//                 )
//                 .unwrap(),
//             );

//         let expected_envelope = TypedTransaction::Eip1559(expected_tx);
//         let typed_tx_hex =
//     hex::decode("
//     02f899018085602b94278b85b2f7a17de88302cf5c940aa7420c43b8c1a7b165d216948870c8ecfe1ee18802c68af0bb140000a46ecd23060000000000000000000000000000000000000000000000000000000000000002c080a0c5f35bf1cc6ab13053e33b1af7400c267be17218aeadcdb4ae3eefd4795967e8a04f6871044dd6368aea8deecd1c29f55b5531020f5506502e3f79ad457051bc4a"
//     ).unwrap();

//         let tx_rlp = rlp::Rlp::new(typed_tx_hex.as_slice());
//         let (actual_tx, signature) = TypedTransaction::decode_signed(&tx_rlp).unwrap();
//         assert_eq!(expected_envelope, actual_tx);
//         assert_eq!(
//             expected_envelope.hash(&signature),
//             H256::from_str("0x206e4c71335333f8658e995cc0c4ee54395d239acb08587ab8e5409bfdd94a6f")
//                 .unwrap()
//         );
//     }

//     #[test]
//     fn test_eip155_decode() {
//         let tx = TransactionRequest::new()
//             .nonce(9)
//             .to("35353535353535353535353535353535353535353535".parse::<Address>().unwrap())
//             .value(1000000000000000000u64)
//             .energy_price(20000000000u64)
//             .energy(21000)
//             .network_id(1);

//         let expected_hex = hex::decode(
//             "
// ee098504a817c8008252089635353535353535353535353535353535353535353535880de0b6b3a764000080018080",
//         )
//         .unwrap();
//         let expected_rlp = rlp::Rlp::new(expected_hex.as_slice());
//         let decoded_transaction = TypedTransaction::decode(&expected_rlp).unwrap();
//         assert_eq!(tx.sighash(), decoded_transaction.sighash());
//     }

//     #[test]
//     fn test_eip1559_deploy_tx_decode() {
//         let typed_tx_hex =
//             hex::decode("02dc8205058193849502f90085010c388d00837a120080808411223344c0").unwrap();
//         let tx_rlp = rlp::Rlp::new(typed_tx_hex.as_slice());
//         TypedTransaction::decode(&tx_rlp).unwrap();
//     }

//     #[test]
//     fn test_tx_casts() {
//         // eip1559 tx
//         let typed_tx_hex =
// hex::decode("
// 02f86b8205390284773594008477359400830186a09496216849c49358b10257cb55b28ea603c874b05e865af3107a4000825544f838f7940000000000000000000000000000000000000001e1a00100000000000000000000000000000000000000000000000000000000000000"
// ).unwrap();
//         let tx_rlp = rlp::Rlp::new(typed_tx_hex.as_slice());
//         let tx = TypedTransaction::decode(&tx_rlp).unwrap();

//         {
//             let typed_tx: TypedTransaction = tx.clone();

//             let tx0: TransactionRequest = typed_tx.clone().into();
//             assert!(typed_tx.as_legacy_ref().is_none());

//             let tx1 = typed_tx.into_legacy();

//             assert_eq!(tx0, tx1);
//         }
//         {
//             let typed_tx: TypedTransaction = tx.clone();
//             let tx0: Eip1559TransactionRequest = typed_tx.clone().into();
//             assert_eq!(tx.as_eip1559_ref().unwrap(), &tx0);

//             let tx1 = typed_tx.into_eip1559();

//             assert_eq!(tx0, tx1);
//         }
//         {
//             let typed_tx: TypedTransaction = tx;
//             let tx0: Eip2930TransactionRequest = typed_tx.clone().into();
//             assert!(typed_tx.as_eip2930_ref().is_none());

//             let tx1 = typed_tx.into_eip2930();

//             assert_eq!(tx0, tx1);
//         }
//     }
// }
