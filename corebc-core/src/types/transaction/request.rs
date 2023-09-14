//! Transaction types
use super::{decode_to, extract_network_id, rlp_opt, NUM_TX_FIELDS};
use crate::{
    types::{
        Address, Bytes, NameOrAddress, Signature, SignatureError, Transaction, H256, U256, U64,
    },
    utils::sha3,
};

use rlp::{Decodable, RlpStream};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error involving a transaction request.
#[derive(Debug, Error)]
pub enum RequestError {
    /// When decoding a transaction request from RLP
    #[error(transparent)]
    DecodingError(#[from] rlp::DecoderError),
    /// When recovering the address from a signature
    #[error(transparent)]
    RecoveryError(#[from] SignatureError),
}

/// Parameters for sending a transaction
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TransactionRequest {
    /// Sender address or ENS name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,

    /// Recipient address (None for contract creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<NameOrAddress>,

    /// Supplied gas (None for sensible default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<U256>,

    /// Gas price (None for sensible default)
    #[serde(rename = "gasPrice")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<U256>,

    /// Transferred value (None for no transfer)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,

    /// The compiled code of a contract OR the first 4 bytes of the hash of the
    /// invoked method signature and encoded parameters. For details see Ethereum Contract ABI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,

    /// Transaction nonce (None for next available nonce)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U256>,

    /// Network ID (None for mainnet)
    #[serde(skip_serializing)]
    #[serde(default, rename = "networkId")]
    pub network_id: Option<U64>,
}

impl TransactionRequest {
    /// Creates an empty transaction request with all fields left empty
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience function for sending a new payment transaction to the receiver.
    pub fn pay<T: Into<NameOrAddress>, V: Into<U256>>(to: T, value: V) -> Self {
        TransactionRequest { to: Some(to.into()), value: Some(value.into()), ..Default::default() }
    }

    // Builder pattern helpers

    /// Sets the `from` field in the transaction to the provided value
    #[must_use]
    pub fn from<T: Into<Address>>(mut self, from: T) -> Self {
        self.from = Some(from.into());
        self
    }

    /// Sets the `to` field in the transaction to the provided value
    #[must_use]
    pub fn to<T: Into<NameOrAddress>>(mut self, to: T) -> Self {
        self.to = Some(to.into());
        self
    }

    /// Sets the `gas` field in the transaction to the provided value
    #[must_use]
    pub fn gas<T: Into<U256>>(mut self, gas: T) -> Self {
        self.gas = Some(gas.into());
        self
    }

    /// Sets the `gas_price` field in the transaction to the provided value
    #[must_use]
    pub fn gas_price<T: Into<U256>>(mut self, gas_price: T) -> Self {
        self.gas_price = Some(gas_price.into());
        self
    }

    /// Sets the `value` field in the transaction to the provided value
    #[must_use]
    pub fn value<T: Into<U256>>(mut self, value: T) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets the `data` field in the transaction to the provided value
    #[must_use]
    pub fn data<T: Into<Bytes>>(mut self, data: T) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Sets the `nonce` field in the transaction to the provided value
    #[must_use]
    pub fn nonce<T: Into<U256>>(mut self, nonce: T) -> Self {
        self.nonce = Some(nonce.into());
        self
    }

    /// Sets the `network_id` field in the transaction to the provided value
    #[must_use]
    pub fn network_id<T: Into<U64>>(mut self, network_id: T) -> Self {
        self.network_id = Some(network_id.into());
        self
    }

    /// Hashes the transaction's data with the provided network id
    pub fn sighash(&self) -> H256 {
        match self.network_id {
            Some(_) => sha3(self.rlp().as_ref()).into(),
            None => sha3(self.rlp_unsigned().as_ref()).into(),
        }
    }

    /// Gets the transaction's RLP encoding, prepared with the network_id and extra fields for
    /// signing. Assumes the networkid exists.
    pub fn rlp(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        if let Some(network_id) = self.network_id {
            rlp.begin_list(NUM_TX_FIELDS);
            self.rlp_base(&mut rlp);
            rlp.append(&network_id);
            rlp.append(&0u8);
            rlp.append(&0u8);
        } else {
            rlp.begin_list(NUM_TX_FIELDS - 3);
            self.rlp_base(&mut rlp);
        }
        rlp.out().freeze().into()
    }

    /// Gets the unsigned transaction's RLP encoding
    pub fn rlp_unsigned(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_list(NUM_TX_FIELDS - 3);
        self.rlp_base(&mut rlp);
        rlp.out().freeze().into()
    }

    /// Produces the RLP encoding of the transaction with the provided signature
    pub fn rlp_signed(&self, signature: &Signature) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_list(NUM_TX_FIELDS);

        self.rlp_base(&mut rlp);

        // append the signature
        rlp.append(&signature.v);
        rlp.append(&signature.r);
        rlp.append(&signature.s);
        rlp.out().freeze().into()
    }

    pub(crate) fn rlp_base(&self, rlp: &mut RlpStream) {
        rlp_opt(rlp, &self.nonce);
        rlp_opt(rlp, &self.gas_price);
        rlp_opt(rlp, &self.gas);

        rlp_opt(rlp, &self.to.as_ref());
        rlp_opt(rlp, &self.value);
        rlp_opt(rlp, &self.data.as_ref().map(|d| d.as_ref()));
    }

    /// Decodes the unsigned rlp, returning the transaction request and incrementing the counter
    /// passed as we are traversing the rlp list.
    pub(crate) fn decode_unsigned_rlp_base(
        rlp: &rlp::Rlp,
        offset: &mut usize,
    ) -> Result<Self, rlp::DecoderError> {
        let mut txn = TransactionRequest::new();
        txn.nonce = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;
        txn.gas_price = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;
        txn.gas = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;

        txn.to = decode_to(rlp, offset)?.map(NameOrAddress::Address);
        txn.value = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;

        // finally we need to extract the data which will be encoded as another rlp
        let txndata = rlp::Rlp::new(rlp.at(*offset)?.as_raw()).data()?;
        txn.data = match txndata.len() {
            0 => None,
            _ => Some(Bytes::from(txndata.to_vec())),
        };
        *offset += 1;
        Ok(txn)
    }

    /// Decodes RLP into a transaction.
    pub fn decode_unsigned_rlp(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let mut offset = 0;
        let mut txn = Self::decode_unsigned_rlp_base(rlp, &mut offset)?;

        // If the transaction includes more info, like the networkid, as we serialize in `rlp`, this
        // will decode that value.
        if let Ok(networkid) = rlp.val_at(offset) {
            // If a signed transaction is passed to this method, the networkid would be set to the v
            // value of the signature.
            txn.network_id = Some(networkid);
        }

        Ok(txn)
    }

    /// Decodes the given RLP into a transaction, attempting to decode its signature as well.
    pub fn decode_signed_rlp(rlp: &rlp::Rlp) -> Result<(Self, Signature), RequestError> {
        let mut offset = 0;
        let mut txn = Self::decode_unsigned_rlp_base(rlp, &mut offset)?;

        let v = rlp.at(offset)?.as_val()?;
        // populate networkid from v in case the signature follows EIP155
        txn.network_id = extract_network_id(v);
        offset += 1;
        let r = rlp.at(offset)?.as_val()?;
        offset += 1;
        let s = rlp.at(offset)?.as_val()?;

        let sig = Signature { r, s, v };
        txn.from = Some(sig.recover(txn.sighash())?);

        Ok((txn, sig))
    }
}

impl Decodable for TransactionRequest {
    /// Decodes the given RLP into a transaction request, ignoring the signature if populated
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        Self::decode_unsigned_rlp(rlp)
    }
}

impl From<&Transaction> for TransactionRequest {
    fn from(tx: &Transaction) -> TransactionRequest {
        TransactionRequest {
            from: Some(tx.from),
            to: tx.to.map(NameOrAddress::Address),
            gas: Some(tx.gas),
            gas_price: tx.gas_price,
            value: Some(tx.value),
            data: Some(Bytes(tx.input.0.clone())),
            nonce: Some(tx.nonce),
            network_id: tx.network_id.map(|x| U64::from(x.as_u64())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Bytes;
    use rlp::{Decodable, Rlp};
    use std::str::FromStr;

    #[test]
    fn encode_decode_rlp() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .gas_price(1)
            .gas(25000)
            .to("0000b94f5374fce5edbc8e2a8697c15331677e6ebf0b".parse::<Address>().unwrap())
            .value(10)
            .data(vec![0x55, 0x44])
            .network_id(1);

        // turn the rlp bytes encoding into a rlp stream and check that the decoding returns the
        // same struct
        let rlp_bytes = &tx.rlp().to_vec()[..];
        let got_rlp = Rlp::new(rlp_bytes);
        let txn_request = TransactionRequest::decode(&got_rlp).unwrap();

        // We compare the sighash rather than the specific struct
        assert_eq!(tx.sighash(), txn_request.sighash());
    }

    #[test]
    // test data from https://github.com/ethereum/go-ethereum/blob/b1e72f7ea998ad662166bcf23705ca59cf81e925/core/types/transaction_test.go#L40
    fn empty_sighash_check() {
        let tx = TransactionRequest::new()
            .nonce(0)
            .to("0000095e7baea6a6c7c4c2dfeb977efac326af552d87".parse::<Address>().unwrap())
            .value(0)
            .gas(0)
            .gas_price(0);
        let expected_sighash = "ec08902c56d6df8797a282763e4871a2b69dbb210b5390e7babbf1cebe59a23d";
        let got_sighash = hex::encode(tx.sighash().as_bytes());
        assert_eq!(expected_sighash, got_sighash);
    }
    #[test]
    fn decode_unsigned_transaction() {
        let _res: TransactionRequest = serde_json::from_str(
            r#"{
    "gas":"0xc350",
    "gasPrice":"0x4a817c800",
    "hash":"0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "input":"0x68656c6c6f21",
    "nonce":"0x15",
    "to":"0x0000f02c1c8e6114b1dbe8937a39260b5b0a374432bb",
    "transactionIndex":"0x41",
    "value":"0xf3dbb76162000",
    "network_id": "0x1"
  }"#,
        )
        .unwrap();
    }

    // CORETODO: This tests a signed transaction. First implement ED448, then fix the test.
    // #[test]
    // fn decode_known_rlp_devin() {
    //     let tx = TransactionRequest::new()
    //         .nonce(70272)
    //         .from("0000fab2b4b677a4e104759d378ea25504862150256e".parse::<Address>().unwrap())
    //         .to("0000d1f23226fb4d2b7d2f3bcdd99381b038de705a64".parse::<Address>().unwrap())
    //         .value(0)
    //         .gas_price(1940000007)
    //         .gas(21000);

    //     let expected_rlp =
    // hex::decode("
    // f866830112808473a20d0782520894d1f23226fb4d2b7d2f3bcdd99381b038de705a6480801ca04bc89d41c954168afb4cbd01fe2e0f9fe12e3aa4665eefcee8c4a208df044b5da05d410fd85a2e31870ea6d6af53fafc8e3c1ae1859717c863cac5cff40fee8da4"
    // ).unwrap();     let (got_tx, _signature) =
    //         TransactionRequest::decode_signed_rlp(&Rlp::new(&expected_rlp)).unwrap();

    //     // intialization of TransactionRequests using new() uses the Default trait, so we just
    //     // compare the sighash and signed encoding instead.
    //     assert_eq!(got_tx.sighash(), tx.sighash());
    // }

    #[test]
    fn decode_unsigned_rlp_no_networkid() {
        // unlike the corresponding transaction
        // 0x02c563d96acaf8c157d08db2228c84836faaf3dd513fc959a54ed4ca6c72573e, this doesn't have a
        // `from` field because the `from` field is only obtained via signature recovery
        let expected_tx = TransactionRequest::new()
            .to(Address::from_str("0x0000c7696b27830dd8aa4823a1cba8440c27c36adec4").unwrap())
            .gas(3_000_000)
            .gas_price(20_000_000_000u64)
            .value(0)
            .nonce(6306u64)
            .data(
                Bytes::from_str(
                    "0x91b7f5ed0000000000000000000000000000000000000000000000000000000000000372",
                )
                .unwrap(),
            );

        // manually stripped the signature off the end and modified length
        let expected_rlp = hex::decode("f84a8218a28504a817c800832dc6c0960000c7696b27830dd8aa4823a1cba8440c27c36adec480a491b7f5ed0000000000000000000000000000000000000000000000000000000000000372").unwrap();
        let real_tx = TransactionRequest::decode(&Rlp::new(&expected_rlp)).unwrap();

        assert_eq!(real_tx, expected_tx);
    }

    #[test]
    fn test_eip155_encode() {
        let tx = TransactionRequest::new()
            .nonce(9)
            .to("35353535353535353535353535353535353535353535".parse::<Address>().unwrap())
            .value(1000000000000000000u64)
            .gas_price(20000000000u64)
            .gas(21000)
            .network_id(1);

        let expected_rlp = hex::decode("ee098504a817c8008252089635353535353535353535353535353535353535353535880de0b6b3a764000080018080").unwrap();
        assert_eq!(expected_rlp, tx.rlp().to_vec());

        let expected_sighash =
            hex::decode("2e71f6c2963fa9e8ded46264e0be28402944a5f9e78d62e9de18c4df76bb2421")
                .unwrap();

        assert_eq!(expected_sighash, tx.sighash().as_bytes().to_vec());
    }

    #[test]
    fn test_eip155_decode() {
        let tx = TransactionRequest::new()
            .nonce(9)
            .to("35353535353535353535353535353535353535353535".parse::<Address>().unwrap())
            .value(1000000000000000000u64)
            .gas_price(20000000000u64)
            .gas(21000)
            .network_id(1);

        let expected_hex = hex::decode("ee098504a817c8008252089635353535353535353535353535353535353535353535880de0b6b3a764000080018080").unwrap();
        let expected_rlp = rlp::Rlp::new(expected_hex.as_slice());
        let decoded_transaction = TransactionRequest::decode(&expected_rlp).unwrap();
        assert_eq!(tx, decoded_transaction);
    }

    // CORETODO: Implement ED448 then fix this test
    // #[test]
    // fn test_eip155_decode_signed() {
    //     let expected_signed_bytes =
    // hex::decode("
    // f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83"
    // ).unwrap();     let expected_signed_rlp =
    // rlp::Rlp::new(expected_signed_bytes.as_slice());     let (decoded_tx, decoded_sig) =
    //         TransactionRequest::decode_signed_rlp(&expected_signed_rlp).unwrap();

    //     let expected_sig = Signature {
    //         v: 37,
    //         r: U256::from_dec_str(
    //             "18515461264373351373200002665853028612451056578545711640558177340181847433846",
    //         )
    //         .unwrap(),
    //         s: U256::from_dec_str(
    //             "46948507304638947509940763649030358759909902576025900602547168820602576006531",
    //         )
    //         .unwrap(),
    //     };
    //     assert_eq!(expected_sig, decoded_sig);
    //     assert_eq!(decoded_tx.network_id, Some(U64::from(1)));
    // }

    // CORETODO: Implement ED448 then fix this test
    // #[test]
    // fn test_recover_legacy_tx() {
    //     let raw_tx =
    // "f9015482078b8505d21dba0083022ef1947a250d5630b4cf539739df2c5dacb4c659f2488d880c46549a521b13d8b8e47ff36ab50000000000000000000000000000000000000000000066ab5a608bd00a23f2fe000000000000000000000000000000000000000000000000000000000000008000000000000000000000000048c04ed5691981c42154c6167398f95e8f38a7ff00000000000000000000000000000000000000000000000000000000632ceac70000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000006c6ee5e31d828de241282b9606c8e98ea48526e225a0c9077369501641a92ef7399ff81c21639ed4fd8fc69cb793cfa1dbfab342e10aa0615facb2f1bcf3274a354cfe384a38d0cc008a11c2dd23a69111bc6930ba27a8"
    // ;

    //     let data = hex::decode(raw_tx).unwrap();
    //     let rlp = Rlp::new(&data);
    //     let (tx, sig) = TypedTransaction::decode_signed(&rlp).unwrap();
    //     let recovered = sig.recover(tx.sighash()).unwrap();

    //     let expected: Address = "0xa12e1462d0ced572f396f58b6e2d03894cd7c8a4".parse().unwrap();
    //     assert_eq!(expected, recovered);
    // }
}
