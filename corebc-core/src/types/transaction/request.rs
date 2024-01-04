//! Transaction types
use super::{decode_to, rlp_opt, NUM_TX_FIELDS};
use crate::{
    types::{
        Address, Bytes, NameOrAddress, Network, Signature, SignatureError, Transaction, H256, U256,
        U64,
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

    /// Supplied energy (None for sensible default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy: Option<U256>,

    /// energy price (None for sensible default)
    #[serde(rename = "energyPrice")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_price: Option<U256>,

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

    /// Sets the `energy` field in the transaction to the provided value
    #[must_use]
    pub fn energy<T: Into<U256>>(mut self, energy: T) -> Self {
        self.energy = Some(energy.into());
        self
    }

    /// Sets the `energy_price` field in the transaction to the provided value
    #[must_use]
    pub fn energy_price<T: Into<U256>>(mut self, energy_price: T) -> Self {
        self.energy_price = Some(energy_price.into());
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
    /// CORETODO: set the workflow for None
    pub fn sighash(&self) -> H256 {
        match self.network_id {
            Some(_) => sha3(self.rlp_sighash().as_ref()).into(),
            None => sha3(self.rlp_unsigned().as_ref()).into(),
        }
    }

    /// Gets the transaction's RLP encoding, prepared with the network_id and extra fields for
    /// signing. Assumes the networkid exists.
    pub fn rlp(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_list(NUM_TX_FIELDS - 2);
        self.rlp_base(&mut rlp);
        rlp.out().freeze().into()
    }

    // Encodes rlp without network_id as the last field (for sighash only)
    pub fn rlp_sighash(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        if let Some(network_id) = self.network_id {
            rlp.begin_list(NUM_TX_FIELDS - 2);
            self.rlp_base_sighash(&mut rlp);
            rlp.append(&network_id);
        } else {
            // CORETODO: Doublecheck what to do with this part
            // If it is called from self.sighash this part is unavailable, but it could be called
            // from eip2718 .sighash()
            rlp.begin_list(NUM_TX_FIELDS - 3);
            self.rlp_base_sighash(&mut rlp);
        }
        rlp.out().freeze().into()
    }

    /// Gets the unsigned transaction's RLP encoding
    pub fn rlp_unsigned(&self) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_list(NUM_TX_FIELDS - 2);
        self.rlp_base(&mut rlp);
        rlp.out().freeze().into()
    }

    /// Produces the RLP encoding of the transaction with the provided signature
    pub fn rlp_signed(&self, signature: &Signature) -> Bytes {
        let mut rlp = RlpStream::new();
        rlp.begin_list(NUM_TX_FIELDS - 1);

        self.rlp_base(&mut rlp);

        // append the signature
        rlp.append(&signature.sig);
        rlp.out().freeze().into()
    }

    pub(crate) fn rlp_base(&self, rlp: &mut RlpStream) {
        rlp_opt(rlp, &self.nonce);
        rlp_opt(rlp, &self.energy_price);
        rlp_opt(rlp, &self.energy);

        rlp_opt(rlp, &self.network_id);

        rlp_opt(rlp, &self.to.as_ref());
        rlp_opt(rlp, &self.value);
        rlp_opt(rlp, &self.data.as_ref().map(|d| d.as_ref()));
    }

    // Rlp encoding without network_id should be used only for encoding sighash
    pub(crate) fn rlp_base_sighash(&self, rlp: &mut RlpStream) {
        rlp_opt(rlp, &self.nonce);
        rlp_opt(rlp, &self.energy_price);
        rlp_opt(rlp, &self.energy);

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
        txn.energy_price = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;
        txn.energy = Some(rlp.at(*offset)?.as_val()?);
        *offset += 1;

        txn.network_id = Some(rlp.at(*offset)?.as_val()?);
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

        let sig = rlp.at(offset)?.as_val()?;

        let sig = Signature { sig };

        // CORETODO: Please find a way to unwrap it more naturally
        let network = Network::try_from(txn.network_id.unwrap()).unwrap();
        txn.from = Some(sig.recover(txn.sighash(), &network)?);

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
            energy: Some(tx.energy),
            energy_price: Some(tx.energy_price),
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
    use ethabi::ethereum_types::H1368;
    use rlp::{Decodable, Rlp};

    #[test]
    fn encode_decode_rlp() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .energy_price(1)
            .energy(25000)
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
    // test data from https://github.com/core-coin/go-core/blob/41c152fffa96b6488feb1d7fa878addc30def5b3/core/types/transaction_test.go#L58
    fn empty_sighash_check() {
        let tx = TransactionRequest::new()
            .nonce(0)
            .to("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87".parse::<Address>().unwrap())
            .value(0)
            .energy(0)
            .energy_price(0)
            .network_id(0);
        let expected_sighash = "0064d7a2aa08686b4f36a2188352ba162ff2b5bdce72335f4a0e25a6c5f47af7";
        let got_sighash = hex::encode(tx.sighash().as_bytes());
        assert_eq!(expected_sighash, got_sighash);
    }
    #[test]
    fn decode_unsigned_transaction() {
        let _res: TransactionRequest = serde_json::from_str(
            r#"{
    "energy":"0xc350",
    "energyPrice":"0x4a817c800",
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

    #[test]
    fn decode_known_rlp_mainnet() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .from("cb8238748ee459bc0c1d86eab1d3f6d83bb433cdad9c".parse::<Address>().unwrap())
            .to("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87".parse::<Address>().unwrap())
            .value(10)
            .energy_price(10)
            .energy(50000)
            .data(vec![0x11, 0x23])
            .network_id(1);

        let expected_rlp =
    hex::decode("f8ce030a82c3500196cb08095e7baea6a6c7c4c2dfeb977efac326af552d870a821123b8ab4baaafc44c4cc23a5ba831b9a89eb823bb965f62de3eeccdaac2a516b6ca4f7ab3e728f8b791d02bca9c5c3b8dd9bfa73c550dfcb63fef4400fa4d5aa5f132ba3932b99ceb8c9014640a77ad022ee6379f3299f060feab4e785650ec3878cb46748f8e15a5473c696cf95c5ede5225312800ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
    ).unwrap();
        let (got_tx, _signature) =
            TransactionRequest::decode_signed_rlp(&Rlp::new(&expected_rlp)).unwrap();

        // intialization of TransactionRequests using new() uses the Default trait, so we just
        // compare the sighash and signed encoding instead.
        assert_eq!(got_tx.sighash(), tx.sighash());
    }

    #[test]
    fn decode_known_rlp_devin() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .from("0xab0338748ee459bc0c1d86eab1d3f6d83bb433cdad9c".parse::<Address>().unwrap())
            .to("ab33d3649d846a2bd426c0ceaca24fab50f7cba8f839".parse::<Address>().unwrap())
            .value(10)
            .energy_price(10)
            .energy(50000)
            .data(vec![0x11, 0x23])
            .network_id(3);

        let expected_rlp =
    hex::decode("f8ce030a82c3500396ab33d3649d846a2bd426c0ceaca24fab50f7cba8f8390a821123b8ab9e73edbc2506ab6805794c3bc45509713493f34eab8e62d2e75ebf9af7346fa85ad0d86c5c116366667b853f63e143943195602bd3cce8078008f9e95d6febd1b3909be9e4e2b1d5090a295c7dccfa28a3a8cc242ec8da680a77901a75c3e97e8c4a73552d09504432157a9d18aa08052700ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
    ).unwrap();
        let (got_tx, _signature) =
            TransactionRequest::decode_signed_rlp(&Rlp::new(&expected_rlp)).unwrap();

        // intialization of TransactionRequests using new() uses the Default trait, so we just
        // compare the sighash and signed encoding instead.
        assert_eq!(got_tx.sighash(), tx.sighash());
    }

    #[test]
    fn test_eip155_encode() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .from("cb8238748ee459bc0c1d86eab1d3f6d83bb433cdad9c".parse::<Address>().unwrap())
            .to("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87".parse::<Address>().unwrap())
            .value(10)
            .energy_price(10)
            .energy(50000)
            .data(vec![0x11, 0x23])
            .network_id(1);

        let expected_rlp =
            hex::decode("e1030a82c3500196cb08095e7baea6a6c7c4c2dfeb977efac326af552d870a821123")
                .unwrap();
        assert_eq!(expected_rlp, tx.rlp().to_vec());
    }

    #[test]
    fn test_eip155_decode() {
        let tx = TransactionRequest::new()
            .nonce(3)
            .to("cb08095e7baea6a6c7c4c2dfeb977efac326af552d87".parse::<Address>().unwrap())
            .value(10)
            .energy_price(10)
            .energy(50000)
            .data(vec![0x11, 0x23])
            .network_id(1);

        let expected_hex =
            hex::decode("e1030a82c3500196cb08095e7baea6a6c7c4c2dfeb977efac326af552d870a821123")
                .unwrap();
        let expected_rlp = rlp::Rlp::new(expected_hex.as_slice());
        let decoded_transaction = TransactionRequest::decode(&expected_rlp).unwrap();
        assert_eq!(tx, decoded_transaction);
    }

    #[test]
    fn test_eip155_decode_signed() {
        let expected_signed_bytes =
            hex::decode(
                "f8ce030a82c3500396ab33d3649d846a2bd426c0ceaca24fab50f7cba8f8390a821123b8ab9e73edbc2506ab6805794c3bc45509713493f34eab8e62d2e75ebf9af7346fa85ad0d86c5c116366667b853f63e143943195602bd3cce8078008f9e95d6febd1b3909be9e4e2b1d5090a295c7dccfa28a3a8cc242ec8da680a77901a75c3e97e8c4a73552d09504432157a9d18aa08052700ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
            ).unwrap();
        let expected_signed_rlp = rlp::Rlp::new(expected_signed_bytes.as_slice());
        let (decoded_tx, decoded_sig) =
            TransactionRequest::decode_signed_rlp(&expected_signed_rlp).unwrap();

        let expected_sig = Signature {
            sig: H1368::from_slice(
                hex::decode(
                    "9e73edbc2506ab6805794c3bc45509713493f34eab8e62d2e75ebf9af7346fa85ad0d86c5c116366667b853f63e143943195602bd3cce8078008f9e95d6febd1b3909be9e4e2b1d5090a295c7dccfa28a3a8cc242ec8da680a77901a75c3e97e8c4a73552d09504432157a9d18aa08052700ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580"
                ).unwrap().as_slice()
            ),
        };
        assert_eq!(expected_sig, decoded_sig);
        assert_eq!(decoded_tx.network_id, Some(U64::from(3)));
    }

    #[test]
    fn test_recover_legacy_tx() {
        let raw_tx =
            "f8ce030a82c3500396ab33d3649d846a2bd426c0ceaca24fab50f7cba8f8390a821123b8ab9e73edbc2506ab6805794c3bc45509713493f34eab8e62d2e75ebf9af7346fa85ad0d86c5c116366667b853f63e143943195602bd3cce8078008f9e95d6febd1b3909be9e4e2b1d5090a295c7dccfa28a3a8cc242ec8da680a77901a75c3e97e8c4a73552d09504432157a9d18aa08052700ba277941fcb9ac8063a9b6ed64fbc86c51dd5ae6cf1f01f7bcf533cf0b0cfc5dc3fdc5bc7eaa99366ada5e7127331b862586a46c12a85f9580";

        let data = hex::decode(raw_tx).unwrap();
        let rlp = Rlp::new(&data);
        let (tx, sig) = TransactionRequest::decode_signed_rlp(&rlp).unwrap();
        let network = Network::try_from(tx.network_id.unwrap()).unwrap();
        let recovered = sig.recover(tx.sighash(), &network).unwrap();

        let expected: Address = "0xab0338748ee459bc0c1d86eab1d3f6d83bb433cdad9c".parse().unwrap();
        assert_eq!(expected, recovered);
    }
}
